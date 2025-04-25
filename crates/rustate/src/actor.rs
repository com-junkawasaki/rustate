use crate::error::{Result, StateError};
use crate::event::{Event as RustateEvent, EventTrait};
use crate::guard::Guard;
use crate::state::StateTrait;
use crate::Context;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};
use tracing::{debug, error, info};
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

// --- Actor Logic Trait ---
#[async_trait]
pub trait Logic<S, E, I, Q, R>: Send + Sync {
    async fn handle_event(&self, state: &mut S, context: &Context, event: &E) -> Result<(), Error>;
    async fn handle_query(&self, state: &S, context: &Context, query: Q) -> Result<R, Error>;
    async fn decide(&self, state: &S, context: &Context, snapshot: &Option<Snapshot<Context>>) -> Result<Vec<E>, Error>;
}

// --- Actor Trait ---
#[async_trait]
pub trait ActorTrait<E, Q, Resp>: Send + Sync {
    fn id(&self) -> Uuid;
    async fn handle_event(&mut self, event: E);
    async fn handle_query(&self, query: Q) -> Resp;
    async fn started(&mut self);
    async fn stopped(&mut self);
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
    fn get_snapshot(&self) -> TSnapshot;

    /// Clones the ActorRef (typically involves cloning an Arc or channel sender).
    fn clone_ref(&self) -> Box<dyn ActorRef<TEvent, TSnapshot>>;

    fn stop(&self) -> Result<(), StateError>;

    async fn query(&self, query: TEvent::Query) -> Result<TEvent::Response, StateError>
    where
        TEvent: QueryableEvent,
        TEvent::Query: Send + Sync,
        TEvent::Response: Send + Sync + fmt::Debug;

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

// --- ActorRef Implementation ---
pub struct ActorRef<E, Q, Resp>
where
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    pub id: Uuid,
    pub sender: mpsc::Sender<ActorCommand<E, Q, Resp>>,
    _query_marker: PhantomData<Q>,
    _response_marker: PhantomData<Resp>,
}

impl<E, Q, Resp> Clone for ActorRef<E, Q, Resp>
where
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    Q: Send + fmt::Debug + Sync + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            sender: self.sender.clone(),
            _query_marker: PhantomData,
            _response_marker: PhantomData,
        }
    }
}

impl<E, Q, Resp> fmt::Debug for ActorRef<E, Q, Resp>
where
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    Q: Send + fmt::Debug + Sync + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorRef")
            .field("id", &self.id)
            .finish()
    }
}

// --- ActorRef Send/Query Methods ---
impl<E, Q, Resp> ActorRef<E, Q, Resp>
where
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
{
    pub async fn send_event(&self, event: E) -> Result<(), StateError> {
        self.sender
            .send(ActorCommand::Send(event))
            .await
            .map_err(|e| StateError::SendError(format!("Failed to send event: {}", e)))
    }

    pub async fn query(&self, query: Q) -> Result<Resp, StateError>
    where
    {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ActorCommand::Query(query, tx))
            .await
            .map_err(|e| StateError::SendError(format!("Failed to send query: {}", e)))?;
        rx.await
            .map_err(|e| StateError::ReceiveError(format!("Failed to receive query response: {}", e)))
    }

    pub async fn stop(&self) -> Result<(), StateError> {
        self.sender
            .send(ActorCommand::Stop)
            .await
            .map_err(|e| StateError::SendError(format!("Failed to send stop command: {}", e)))
    }
}

// --- Actor Implementation ---
pub struct ActorImpl<L, S, E, I, Q, R>
where
    L: Logic<S, E, I, Q, R> + Send + Sync + 'static,
    S: StateTrait<Context = Context, Event = E> + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + Debug + 'static,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static,
{
    id: Uuid,
    logic: Arc<L>,
    state: S,
    sender: mpsc::Sender<ActorCommand<E, Q, Result<R, StateError>>>,
    context: Arc<tokio::sync::Mutex<Context>>,
    _marker: PhantomData<(I, R)>,
}

// Implementation for ActorImpl using the defined ActorTrait
#[async_trait]
impl<L, S, E, I, Q, R> ActorTrait<E, Q, Result<R, StateError>> for ActorImpl<L, S, E, I, Q, R>
where
    L: Logic<S, E, I, Q, R> + Send + Sync + 'static,
    S: StateTrait<Context = Context, Event = E> + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + Debug + 'static,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static,
{
    fn id(&self) -> Uuid {
        self.id
    }

    async fn handle_event(&mut self, event: E) {
        let mut context_guard = self.context.lock().await;
        if let Err(e) = self
            .logic
            .handle_event(&mut self.state, &mut context_guard, &event)
            .await
        {
            error!(actor_id = %self.id, error = ?e, event = ?event, "Error handling event");
        } else {
            // Event handled successfully
        }
    }

    async fn handle_query(&self, query: Q) -> Result<R, StateError> {
        let context_guard = self.context.lock().await;
        match self.logic.handle_query(&self.state, &context_guard, query).await {
            Ok(response) => Ok(response),
            Err(e) => {
                error!(actor_id = %self.id, error = ?e, "Error handling query");
                Err(StateError::QueryError(format!(
                    "Query handling failed: {:?}",
                    e
                )))
            }
        }
    }

    async fn started(&mut self) {
        info!(actor_id = %self.id, "Actor implementation started hook.");
    }

    async fn stopped(&mut self) {
        info!(actor_id = %self.id, "Actor implementation stopped hook.");
    }
}

// Run the actor main loop
pub async fn run_actor<
    TEvent: Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
>(
    mut actor: Box<dyn ActorTrait<TEvent, Q, Resp> + Send + Sync>,
    mut receiver: mpsc::Receiver<ActorCommand<TEvent, Q, Resp>>,
    actor_ref_id: Uuid,
) {
    info!(actor_id = %actor_ref_id, "Actor started");
    actor.started().await;

    while let Some(command) = receiver.recv().await {
        match command {
            ActorCommand::Send(event) => {
                debug!(actor_id = %actor_ref_id, event = ?event, "Received event");
                actor.handle_event(event).await;
            }
            ActorCommand::Query { query, responder } => {
                debug!(actor_id = %actor_ref.id, query = ?query, "Received query");
                let response_result = actor.handle_query(query).await;
                if let Err(_) = responder.send(response_result) {
                    error!(actor_id = %actor_ref.id, "Failed to send query response: receiver dropped");
                }
            }
            ActorCommand::Stop => {
                info!(actor_id = %actor_ref.id, "Stopping actor");
                break;
            }
        }
    }

    actor.stopped().await;
    info!(actor_id = %actor_ref.id, "Actor stopped");
}

// Helper to spawn an actor and return its reference
pub fn _spawn_actor<
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
        run_actor(actor, receiver, actor_ref).await;
    };

    let _handle = tokio::spawn(run_task);

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
    use crate::state::{State, StateType};
    use crate::Event as CrateEvent;
    use std::fmt::Display;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration as StdDuration;
    use tokio::time::{sleep, timeout};

    // --- Test Fixtures ---
    /// Example state implementation
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TestState(pub String);

    impl Display for TestState {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    /// Example event implementation
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TestEvent(pub String);

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            &self.0
        }
        fn payload(&self) -> Option<&serde_json::Value> {
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
    impl From<CrateEvent> for TestEvent {
        fn from(e: CrateEvent) -> Self {
            TestEvent(e.event_type)
        }
    }

    #[derive(Debug, Clone)]
    struct TestQuery(String);
    #[derive(Debug, Clone, PartialEq)]
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
        type Response = Result<TestResponse, StateError>;

        async fn handle_event(&mut self, event: TestEvent) {
            debug!("Handling event: {:?}", event);
            self.state = TestState(format!("State after {}", event.0));
            self.event_count += 1;
        }

        async fn handle_query(&self, query: Self::Query) -> Self::Response {
            debug!("Handling query: {:?}", query);
            Ok(TestResponse(format!(
                "Response to {} from state {}",
                query.0, self.state.0
            )))
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
        let handle = tokio::spawn(run_actor(actor, receiver, actor_ref.clone()));

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
        let handle = tokio::spawn(run_actor(actor, receiver, actor_ref.clone()));

        let query = TestQuery("test_query".to_string());
        let result = actor_ref.query(query).await;

        assert!(result.is_ok());
        let response_result = result.unwrap();
        assert!(response_result.is_ok());
        let response = response_result.unwrap();

        assert_eq!(
            response,
            TestResponse("Response to test_query from state query_state".to_string())
        );

        let stop_result = actor_ref.stop();
        assert!(stop_result.is_ok());
        handle.await.expect("Actor task panicked");
    }

    #[tokio::test]
    async fn test_actor_snapshot() {
        let (actor, receiver, actor_ref_arc) =
            create_test_actor(TestState("snap_state".to_string()), 10);
        let actor_ref = actor_ref_arc.as_ref();
        let handle = tokio::spawn(run_actor(actor, receiver, actor_ref.clone()));

        let snapshot = actor_ref.get_snapshot();

        assert_eq!(snapshot.state, TestState("snap_state".to_string()));
        assert_eq!(snapshot.event_count, 0);

        let stop_result = actor_ref.stop();
        assert!(stop_result.is_ok());
        handle.await.expect("Actor task panicked");
    }
}

// Implementation for ActorImpl using the defined ActorTrait
#[async_trait]
impl<L, S, E, I, Q, R> ActorTrait<E, Q, Result<R, StateError>> for ActorRefImpl<E, S, Q, R>
where
    L: Logic<S, E, I, Q, R> + Send + Sync + 'static,
    S: StateTrait<Context = Context, Event = E> + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + Debug + 'static,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static,
{
    fn id(&self) -> Uuid {
        self.id
    }

    async fn handle_event(&mut self, event: E) {
        let mut context_guard = self.context.lock().await;
        if let Err(e) = self
            .logic
            .handle_event(&mut self.state, &mut context_guard, &event)
            .await
        {
            error!(actor_id = %self.id, error = ?e, event = ?event, "Error handling event");
        } else {
            // Potentially trigger decide or other logic after successful event handling
        }
    }

    async fn handle_query(&self, query: Q) -> Result<R, StateError> { // Return Result<R, StateError>
        let context_guard = self.context.lock().await;
        match self.logic.handle_query(&self.state, &context_guard, query).await {
            Ok(response) => Ok(response),
            Err(e) => {
                error!(actor_id = %self.id, error = ?e, "Error handling query");
                Err(StateError::QueryError(format!(
                    "Query handling failed: {:?}",
                    e
                )))
            }
        }
    }

    async fn started(&mut self) {
        info!(actor_id = %self.id, "Actor implementation started hook.");
    }

    async fn stopped(&mut self) {
        info!(actor_id = %self.id, "Actor implementation stopped hook.");
    }

    fn as_actor_trait(&self) -> &(dyn ActorTrait<E, Q, Result<R, StateError>> + Send + Sync) {
        self
    }
     fn as_actor_trait_mut(&mut self) -> &mut (dyn ActorTrait<E, Q, Result<R, StateError>> + Send + Sync) {
        self
    }
}
