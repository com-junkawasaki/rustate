use crate::error::Result;
use crate::error::StateError;
use crate::event::EventTrait;
use crate::state::StateTrait;
use crate::{Context, Event, Machine, MachineBuilder, State, StateType};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

// --- Actor Options ---
#[derive(Debug, Clone)]
pub struct ActorOptions<I: Send + Sync + 'static> {
    pub id: Option<String>,
    pub input: Option<I>,
}

// --- Snapshot ---

/// Represents the immutable state of an actor at a specific point in time.
/// Generic over the actor's context type `TContext`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot<TContext, TOutput = ()> {
    /// The current state value (e.g., hierarchical state identifier).
    /// Using serde_json::Value for flexibility, similar to how XState represents state values.
    /// Consider a more specific enum or struct representation based on machine definition later.
    pub value: serde_json::Value,
    /// The current context (extended state) of the actor.
    pub context: TContext,
    /// The output value produced when the actor reaches a final state.
    pub output: Option<TOutput>,
    /// The status of the actor (e.g., Active, Done, Stopped).
    pub status: ActorStatus,
    // Potential future additions: historyValue, error, etc.
}

impl<TContext, TOutput> Snapshot<TContext, TOutput> {
    pub fn new(
        value: serde_json::Value,
        context: TContext,
        output: Option<TOutput>,
        status: ActorStatus,
    ) -> Self {
        Self {
            value,
            context,
            output,
            status,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorStatus {
    Active,
    Done,
    Stopped,
    Error,
}

// --- Actor Logic ---

/// Defines the behavior of an actor: how it computes its initial state
/// and transitions to new states based on received events.
///
/// Generic Parameters:
/// - TSnapshot: The type of the snapshot produced by this logic.
/// - TEvent: The type of event the actor logic accepts.
/// - TInput: The type of input data provided when the actor starts (must be Send + Sync + 'static).
#[async_trait] // Make the trait async
pub trait ActorLogic<TSnapshot, TEvent: EventTrait, TInput = ()>: Send + Sync {
    // Added Send + Sync bounds as async traits often require them

    /// Computes the initial snapshot (state) of the actor logic.
    /// May use input data provided when the actor is started.
    fn get_initial_snapshot(&self, input: Option<TInput>) -> TSnapshot;

    /// Computes the next snapshot based on the current snapshot and a received event.
    async fn transition(&self, snapshot: TSnapshot, event: TEvent)
        -> Result<TSnapshot, StateError>; // Mark transition as async

    /// Handles an incoming query, returning a response without changing state.
    /// This is optional for ActorLogic implementations.
    async fn handle_query(&self, _query: TEvent::Query) -> Result<TEvent::Response, StateError>
    where
        TEvent: QueryableEvent,
    {
        Err(StateError::NotImplemented(
            "Query handling not implemented for this logic".to_string(),
        ))
    }

    // Potential future additions:
    // fn get_persisted_snapshot(&self, snapshot: TSnapshot) -> ...; // For serialization/restoration
    // fn restore_snapshot(&self, persisted_state: ...) -> TSnapshot;

    async fn started(&mut self) {}
    async fn stopped(&mut self) {}
}

// --- Actor Reference ---

/// A reference to a running actor instance. Used to send events and potentially subscribe to snapshots.
///
/// Generic Parameters:
/// - TEvent: The type of event the actor accepts.
/// - TSnapshot: The type of snapshot the actor emits.
#[async_trait::async_trait]
pub trait ActorRef<TEvent, TSnapshot>: Send + Sync + fmt::Debug
where
    TEvent: EventTrait + Send + Sync + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
{
    /// Sends an event to the actor asynchronously.
    fn send(&self, event: TEvent) -> Result<(), StateError>;

    /// Returns the unique identifier of this actor instance.
    fn id(&self) -> &str;

    /// Gets the latest snapshot emitted by the actor.
    /// Note: This might require internal state management (e.g., caching the last snapshot)
    /// or specific implementations for different actor types.
    fn get_snapshot(&self) -> TSnapshot; // Consider returning Option<TSnapshot> or Result

    // Potential future additions:
    // fn subscribe(&self, observer: impl FnMut(TSnapshot)) -> Subscription;
    // fn stop(&self);
    // fn to_json(&self) -> serde_json::Value; // For inspection/serialization

    /// Clones the ActorRef (typically involves cloning an Arc or channel sender).
    fn clone_ref(&self) -> Box<dyn ActorRef<TEvent, TSnapshot>>;

    // Added stop method signature
    fn stop(&self) -> Result<(), StateError>;

    // Added async query method signature
    async fn query(&self, query: TEvent::Query) -> Result<TEvent::Response, StateError>
    where
        TEvent: QueryableEvent,
        TEvent::Query: Send + Sync,
        TEvent::Response: Send + Sync + fmt::Debug;

    // Added actor_ref method signature
    fn actor_ref(&self) -> &dyn ActorRef<TEvent, TSnapshot>;
}

// Enum for commands sent to the actor
#[derive(fmt::Debug)]
pub enum ActorCommand<E: EventTrait, Q = (), Resp = ()>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
{
    Send(E),
    Query(Q, oneshot::Sender<Result<Resp, StateError>>),
    Stop,
}

/// Trait for events that support a query/response pattern.
pub trait QueryableEvent: EventTrait {
    type Query: Send;
    type Response: Send + fmt::Debug;
}

/// Implementation of ActorRef using a Tokio MPSC channel.
pub struct ActorRefImpl<TEvent, TSnapshot, Q, Resp>
where
    TEvent: EventTrait + Send + Sync + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    id: String,
    sender: mpsc::Sender<ActorCommand<TEvent, Q, Resp>>,
    snapshot: Arc<RwLock<TSnapshot>>,
    _query_marker: PhantomData<Q>,
    _response_marker: PhantomData<Resp>,
}

impl<TEvent, TSnapshot, Q, Resp> Clone for ActorRefImpl<TEvent, TSnapshot, Q, Resp>
where
    TEvent: EventTrait + Send + Sync + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + Sync + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            sender: self.sender.clone(),
            snapshot: self.snapshot.clone(),
            _query_marker: PhantomData,
            _response_marker: PhantomData,
        }
    }
}

impl<TEvent, TSnapshot, Q, Resp> fmt::Debug for ActorRefImpl<TEvent, TSnapshot, Q, Resp>
where
    TEvent: EventTrait + Send + Sync + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + Sync + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorRefImpl")
            .field("id", &self.id)
            .field("snapshot", &self.snapshot) // Consider not printing the whole snapshot
            .finish()
    }
}

#[async_trait::async_trait]
impl<TEvent, TSnapshot, Q, Resp> ActorRef<TEvent, TSnapshot>
    for ActorRefImpl<TEvent, TSnapshot, Q, Resp>
where
    TEvent: QueryableEvent<Query = Q, Response = Resp> + Send + Sync + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    fn id(&self) -> &str {
        &self.id
    }

    fn send(&self, event: TEvent) -> Result<(), StateError> {
        self.sender
            .try_send(ActorCommand::Send(event))
            .map_err(|e| StateError::ActorSendError(e.to_string()))
    }

    fn stop(&self) -> Result<(), StateError> {
        self.sender
            .try_send(ActorCommand::Stop)
            .map_err(|e| StateError::ActorSendError(e.to_string()))
    }

    async fn query(&self, query: TEvent::Query) -> Result<TEvent::Response, StateError> {
        let (response_sender, response_receiver) =
            tokio::sync::oneshot::channel::<Result<TEvent::Response, StateError>>();
        let command = ActorCommand::Query(query, response_sender);
        self.sender.send(command).await.map_err(|e| {
            StateError::ActorSendError(format!("Failed to send query command: {}", e))
        })?;
        response_receiver.await.map_err(|e| {
            StateError::ActorReceiveError(format!("Failed to receive query response: {}", e))
        })?
    }

    fn clone_ref(&self) -> Box<dyn ActorRef<TEvent, TSnapshot>> {
        Box::new(self.clone())
    }

    fn get_snapshot(&self) -> TSnapshot {
        self.snapshot.blocking_read().clone()
    }

    fn actor_ref(&self) -> &dyn ActorRef<TEvent, TSnapshot> {
        self
    }
}

// Actor lifecycle management function
pub async fn run_actor<
    A: Actor<TEvent, TSnapshot> + Send + 'static,
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
>(
    mut actor: A,
    mut receiver: mpsc::Receiver<ActorCommand<TEvent, Q, Resp>>,
) where
    A: Actor<TEvent, TSnapshot, Query = Q, Response = Result<Resp, StateError>>,
    TEvent: QueryableEvent<Query = Q, Response = Resp>,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    info!(actor_id = %actor.actor_ref().id(), "Actor started");
    actor.started().await;

    while let Some(command) = receiver.recv().await {
        match command {
            ActorCommand::Send(event) => {
                debug!(actor_id = %actor.actor_ref().id(), event = ?event, "Received event");
                actor.handle_event(event).await;
            }
            ActorCommand::Query(query, responder) => {
                debug!(actor_id = %actor.actor_ref().id(), query = ?query, "Received query");
                match actor.handle_query(query).await {
                    Ok(response_result) => {
                        if let Err(_) = responder.send(response_result) {
                            error!(actor_id = %actor.actor_ref().id(), "Failed to send query response back");
                        }
                    }
                    Err(e) => {
                        error!(actor_id = %actor.actor_ref().id(), error = %e, "Actor query handling error");
                        let _ = responder.send(Err(e));
                    }
                }
            }
            ActorCommand::Stop => {
                info!(actor_id = %actor.actor_ref().id(), "Stopping actor");
                break;
            }
        }
    }

    actor.stopped().await;
    info!(actor_id = %actor.actor_ref().id(), "Actor stopped");
}

// Helper to spawn an actor and return its reference
pub fn spawn_actor<
    A: Actor<TEvent, TSnapshot> + Send + 'static,
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
>(
    actor: A,
    buffer_size: usize,
) -> Box<dyn ActorRef<TEvent, TSnapshot>>
where
    A: Actor<TEvent, TSnapshot, Query = Q, Response = Result<Resp, StateError>> + Send + 'static,
    TEvent: QueryableEvent<Query = Q, Response = Resp> + Send + Sync + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    let (sender, receiver) = mpsc::channel::<ActorCommand<TEvent, Q, Resp>>(buffer_size);
    let snapshot = actor.snapshot();
    let actor_id = Uuid::new_v4().to_string();

    let actor_ref = ActorRefImpl {
        id: actor_id.clone(),
        sender: sender.clone(),
        snapshot: Arc::new(RwLock::new(snapshot)),
        _query_marker: PhantomData,
        _response_marker: PhantomData,
    };

    let run_task = async move {
        run_actor(actor, receiver).await;
    };

    let handle = tokio::spawn(run_task);

    Box::new(actor_ref)
}

// --- create_actor function ---

/// Creates and starts a new actor instance based on the provided logic.
///
/// Returns an `ActorRef` to interact with the spawned actor.
pub fn create_actor<L, S, E, I, Q, R>(
    logic: L,
    options: ActorOptions<I>,
) -> (ActorRefImpl<E, S, Q, R>, JoinHandle<()>)
where
    L: ActorLogic<S, E, I> + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static + fmt::Debug,
    E: QueryableEvent<Query = Q, Response = R> + Send + Sync + fmt::Debug + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    R: Send + Sync + fmt::Debug + 'static,
    L: ActorLogic<S, E, I>,
{
    let actor_id = options.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let (sender, receiver) = mpsc::channel::<ActorCommand<E, Q, R>>(100);

    let initial_snapshot = logic.get_initial_snapshot(options.input);
    let snapshot_arc = Arc::new(RwLock::new(initial_snapshot));

    let actor_ref = ActorRefImpl {
        id: actor_id.clone(),
        sender,
        snapshot: snapshot_arc.clone(),
        _query_marker: PhantomData,
        _response_marker: PhantomData,
    };

    let cloned_ref = actor_ref.clone();
    let handle = tokio::spawn(async move {
        let mut current_snapshot = snapshot_arc.read().await.clone();

        while let Some(command) = receiver.recv().await {
            match command {
                ActorCommand::Send(event) => {
                    match logic.transition(current_snapshot.clone(), event).await {
                        Ok(next_snapshot) => {
                            let mut snapshot_guard = snapshot_arc.write().await;
                            *snapshot_guard = next_snapshot.clone();
                            current_snapshot = next_snapshot;
                        }
                        Err(e) => {
                            error!(actor_id = %cloned_ref.id(), error = %e, "Actor transition error");
                        }
                    }
                }
                ActorCommand::Query(query, responder) => match logic.handle_query(query).await {
                    Ok(response_result) => {
                        if let Err(_) = responder.send(response_result) {
                            error!(actor_id = %cloned_ref.id(), "Failed to send query response back");
                        }
                    }
                    Err(e) => {
                        error!(actor_id = %cloned_ref.id(), error = %e, "Actor query handling error");
                        let _ = responder.send(Err(e));
                    }
                },
                ActorCommand::Stop => {
                    info!(actor_id = %cloned_ref.id(), "Stopping actor");
                    break;
                }
            }
        }
        eprintln!("Actor [{}] task finished.", cloned_ref.id());
    });

    (actor_ref, handle)
}

pub fn spawn<L, S, E, I, Q, R>(logic: L, options: ActorOptions<I>) -> Box<dyn ActorRef<E, S>>
where
    L: ActorLogic<S, E, I> + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static + fmt::Debug,
    E: QueryableEvent<Query = Q, Response = R> + Send + Sync + fmt::Debug + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    R: Send + Sync + fmt::Debug + 'static,
{
    let (actor_ref, _handle) = create_actor(logic, options);
    Box::new(actor_ref)
}

// Moved Actor trait definition outside of mod tests
trait Actor<E, S>
where
    E: EventTrait + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
{
    type Query: Send + Sync;
    type Response: Send + Sync + fmt::Debug;

    /// Handles an incoming event, potentially updating the actor's state.
    async fn handle_event(&mut self, event: E);
    /// Handles an incoming query, returning a response without changing state.
    async fn handle_query(&self, query: Self::Query) -> Self::Response;
    /// Returns the current snapshot of the actor's state.
    fn snapshot(&self) -> S;
    /// Provides access to the actor's own reference.
    fn actor_ref(&self) -> &dyn ActorRef<E, S>;

    /// Called when the actor starts.
    async fn started(&mut self) { /* default impl */
    }
    /// Called when the actor stops.
    async fn stopped(&mut self) { /* default impl */
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::StateError;
    use crate::state::StateType;
    use serde_json::json;
    use std::fmt::{self, Display};
    use tokio::sync::mpsc;

    // --- Test Fixtures ---
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    struct TestState(String);

    impl Display for TestState {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl StateTrait for TestState {
        fn id(&self) -> &str {
            &self.0
        }
        fn state_type(&self) -> &StateType {
            &StateType::Normal
        }
        fn parent(&self) -> Option<&str> {
            None
        }
        fn children(&self) -> &[String] {
            &[]
        }
        fn initial(&self) -> Option<&str> {
            None
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
    struct TestEvent(String);

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            &self.0
        }
        fn payload(&self) -> Option<&Value> {
            None
        }
        fn name(&self) -> &str {
            &self.0
        }
    }

    impl From<&str> for TestEvent {
        fn from(s: &str) -> Self {
            TestEvent(s.to_string())
        }
    }
    impl From<Event> for TestEvent {
        fn from(e: Event) -> Self {
            TestEvent(e.event_type)
        }
    }

    #[derive(Debug, Clone, Send, Sync)]
    struct TestQuery(String);
    #[derive(Debug, Clone, Send, Sync)]
    struct TestResponse(String);

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestSnapshot {
        state: TestState,
        event_count: u32,
    }

    #[derive(Clone)]
    struct TestActor {
        state: TestState,
        event_count: u32,
        actor_ref: Arc<ActorRefImpl<TestEvent, TestSnapshot, TestQuery, TestResponse>>,
    }

    #[async_trait]
    impl Actor<TestEvent, TestSnapshot> for TestActor {
        type Query = TestQuery;
        type Response = TestResponse;

        async fn handle_event(&mut self, event: TestEvent) {
            debug!("Handling event: {:?}", event);
            self.state = TestState(format!("State after {}", event.0));
            self.event_count += 1;
        }

        async fn handle_query(&self, query: Self::Query) -> Self::Response {
            debug!("Handling query: {:?}", query);
            TestResponse(format!(
                "Response to {} from state {}",
                query.0, self.state.0
            ))
        }

        fn snapshot(&self) -> TestSnapshot {
            TestSnapshot {
                state: self.state.clone(),
                event_count: self.event_count,
            }
        }

        fn actor_ref(&self) -> &dyn ActorRef<TestEvent, TestSnapshot> {
            &*self.actor_ref
        }

        async fn started(&mut self) {
            info!(actor_id=%self.actor_ref.id(), "TestActor started");
        }

        async fn stopped(&mut self) {
            info!(actor_id=%self.actor_ref.id(), "TestActor stopped");
        }
    }

    impl QueryableEvent for TestEvent {
        type Query = TestQuery;
        type Response = TestResponse;
    }

    fn create_test_actor(
        initial_state: TestState,
        buffer_size: usize,
    ) -> (
        TestActor,
        mpsc::Receiver<ActorCommand<TestEvent, TestQuery, Result<TestResponse, StateError>>>,
        Arc<ActorRefImpl<TestEvent, TestSnapshot, TestQuery, TestResponse>>,
    ) {
        let (sender, receiver) = mpsc::channel(buffer_size);
        let actor_id = Uuid::new_v4().to_string();
        let initial_snapshot = TestSnapshot {
            state: initial_state.clone(),
            event_count: 0,
        };
        let actor_ref_impl = ActorRefImpl {
            id: actor_id,
            sender,
            snapshot: Arc::new(RwLock::new(initial_snapshot)),
            _query_marker: PhantomData,
            _response_marker: PhantomData,
        };
        let actor_ref_arc = Arc::new(actor_ref_impl);
        let actor = TestActor {
            state: initial_state,
            event_count: 0,
            actor_ref: actor_ref_arc.clone(),
        };
        (actor, receiver, actor_ref_arc)
    }

    #[tokio::test]
    async fn test_actor_spawn_and_stop() {
        let (actor, receiver, actor_ref_arc) =
            create_test_actor(TestState("initial".to_string()), 10);
        let actor_ref = actor_ref_arc.as_ref();
        let handle = tokio::spawn(run_actor(actor, receiver));

        assert_eq!(actor_ref.id().is_empty(), false);

        let stop_result = actor_ref.stop();
        assert!(stop_result.is_ok());

        handle.await.expect("Actor task panicked");
    }

    #[tokio::test]
    async fn test_actor_send_event() {
        let (mut actor, mut receiver, actor_ref_arc) =
            create_test_actor(TestState("start".to_string()), 10);
        let actor_ref = actor_ref_arc.as_ref();
        let handle = tokio::spawn(async move {
            if let Some(ActorCommand::Send(event)) = receiver.recv().await {
                actor.handle_event(event).await;
            }
            actor.state
        });

        let event = TestEvent("test_event".to_string());
        let send_result = actor_ref.send(event);
        assert!(send_result.is_ok());

        drop(actor_ref);

        let final_state_result = handle.await;
        assert!(final_state_result.is_ok());
        let final_state = final_state_result.unwrap();

        assert_eq!(final_state, TestState("state_after_test_event".to_string()));
    }

    #[tokio::test]
    async fn test_actor_query() {
        let (actor, receiver, actor_ref_arc) =
            create_test_actor(TestState("query_state".to_string()), 10);
        let actor_ref = actor_ref_arc.as_ref();
        let handle = tokio::spawn(run_actor(actor, receiver));

        let query = TestQuery("test_query".to_string());
        let result = actor_ref.query(query).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        assert_eq!(response.0, "response_to_test_query");

        let stop_result = actor_ref.stop();
        assert!(stop_result.is_ok());
        handle.await.expect("Actor task panicked");
    }

    #[tokio::test]
    async fn test_actor_snapshot() {
        let (actor, receiver, actor_ref_arc) =
            create_test_actor(TestState("snap_state".to_string()), 10);
        let actor_ref = actor_ref_arc.as_ref();
        let handle = tokio::spawn(run_actor(actor, receiver));

        let snapshot = actor_ref.get_snapshot();

        assert_eq!(snapshot.state, TestState("snap_state".to_string()));
        assert_eq!(snapshot.event_count, 0);

        let stop_result = actor_ref.stop();
        assert!(stop_result.is_ok());
        handle.await.expect("Actor task panicked");
    }
}
