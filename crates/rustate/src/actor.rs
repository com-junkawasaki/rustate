use crate::error::Result;
use crate::error::StateError;
use crate::event::EventTrait;
use crate::state::StateTrait;
use crate::ActorOptions;
use crate::{Context, Event, Machine, MachineBuilder, State};
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
/// - TInput: The type of input data provided when the actor starts.
#[async_trait] // Make the trait async
pub trait ActorLogic<TSnapshot, TEvent: EventTrait, TInput = ()>: Send + Sync {
    // Added Send + Sync bounds as async traits often require them

    /// Computes the initial snapshot (state) of the actor logic.
    /// May use input data provided when the actor is started.
    fn get_initial_snapshot(&self, input: Option<TInput>) -> TSnapshot;

    /// Computes the next snapshot based on the current snapshot and a received event.
    async fn transition(&self, snapshot: TSnapshot, event: TEvent)
        -> Result<TSnapshot, StateError>; // Mark transition as async

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
pub trait ActorRef<TEvent, TSnapshot>: Send + Sync + fmt::Debug
where
    TEvent: EventTrait + Send + fmt::Debug,
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
}

// Enum for commands sent to the actor
#[derive(fmt::Debug)] // Use fmt::Debug derive
pub enum ActorCommand<E: EventTrait, Q = (), R = Result<(), StateError>>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    R: Send + 'static,
{
    Send(E),
    Query(Q, oneshot::Sender<R>),
    Stop,
}

/// Trait for events that support a query/response pattern.
pub trait QueryableEvent: EventTrait {
    type Query: Send;
    type Response: Send + fmt::Debug;
}

/// Implementation of ActorRef using a Tokio MPSC channel.
pub struct ActorRefImpl<TEvent, TSnapshot, Q, R>
where
    TEvent: EventTrait + Send + 'static,
    TSnapshot: Clone + Send + Sync + 'static,
    Q: Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    id: String,
    sender: mpsc::Sender<ActorCommand<TEvent, Q, R>>,
    snapshot: Arc<RwLock<TSnapshot>>,
    _query_marker: PhantomData<Q>,
    _response_marker: PhantomData<R>,
}

impl<TEvent, TSnapshot, Q, R> Clone for ActorRefImpl<TEvent, TSnapshot, Q, R>
where
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + Sync + 'static,
    R: Send + Sync + fmt::Debug + 'static,
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

impl<TEvent, TSnapshot, Q, R> fmt::Debug for ActorRefImpl<TEvent, TSnapshot, Q, R>
where
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + Sync + 'static,
    R: Send + Sync + fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorRefImpl")
            .field("id", &self.id)
            .field("snapshot", &self.snapshot) // Consider not printing the whole snapshot
            .finish()
    }
}

#[async_trait::async_trait]
impl<TEvent, TSnapshot, Q, R> ActorRef<TEvent, TSnapshot> for ActorRefImpl<TEvent, TSnapshot, Q, R>
where
    TEvent: EventTrait + Send + fmt::Debug + 'static + QueryableEvent<Query = Q, Response = R>,
    TSnapshot: Clone + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    R: Send + Sync + fmt::Debug + 'static,
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
        self.sender
            .send(command)
            .await
            .map_err(|e| StateError::ActorSendError(e.to_string()))?;
        response_receiver
            .await
            .map_err(|e| StateError::ActorReceiveError(e.to_string()))?
    }

    fn clone_ref(&self) -> Box<dyn ActorRef<TEvent, TSnapshot>> {
        Box::new(self.clone())
    }

    fn get_snapshot(&self) -> TSnapshot {
        self.snapshot.blocking_read().clone()
    }
}

// Actor lifecycle management function
pub async fn run_actor<
    A: Actor<TEvent, TSnapshot> + Send + 'static,
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + 'static,
    R: Send + fmt::Debug + 'static,
>(
    mut actor: A,
    mut receiver: mpsc::Receiver<ActorCommand<TEvent, Q, R>>,
) {
    let actor_id_clone = actor.actor_ref().id().to_string();
    println!("Actor [{}] starting run loop.", actor_id_clone);

    while let Some(command) = receiver.recv().await {
        match command {
            ActorCommand::Event(event) => {
                if let Err(e) = actor.handle_event(event).await {
                    eprintln!("Actor [{}] event handling error: {}", actor_id_clone, e);
                }
            }
            ActorCommand::Query { query, responder } => match actor.handle_query(query).await {
                Ok(response) => {
                    if responder.send(Ok(response)).is_err() {
                        eprintln!("Actor [{}] failed to send query response.", actor_id_clone);
                    }
                }
                Err(e) => {
                    let _ = responder.send(Err(e));
                }
            },
            ActorCommand::Stop => {
                println!("Actor [{}] stopping...", actor_id_clone);
                actor.stopped().await;
                break;
            }
        }
    }
    println!("Actor [{}] task finished.", actor_id_clone);
}

// Helper to spawn an actor and return its reference
pub fn spawn_actor<
    A: Actor<TEvent, TSnapshot> + Send + 'static,
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + 'static,
    R: Send + fmt::Debug + 'static,
>(
    actor: A,
    buffer_size: usize,
) -> Box<dyn ActorRef<TEvent, TSnapshot>>
where
    TEvent: QueryableEvent<Query = Q, Response = R>,
{
    let (sender, receiver) = mpsc::channel::<ActorCommand<TEvent, Q, R>>(buffer_size);
    let actor_id = actor.actor_ref().id().to_string();

    let actor_ref = ActorRefImpl {
        id: actor_id,
        sender: sender.clone(),
        _query_marker: std::marker::PhantomData,
        _response_marker: std::marker::PhantomData,
    };

    tokio::spawn(run_actor(actor, receiver));

    Box::new(actor_ref)
}

// --- create_actor function ---

/// Creates and starts a new actor instance based on the provided logic.
///
/// Returns an `ActorRef` to interact with the spawned actor.
pub fn create_actor<L, S, E, I, Q, R>(
    logic: L,
    options: ActorOptions<I>,
) -> ActorRefImpl<E, S, Q, R>
where
    L: ActorLogic<S, E> + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static + fmt::Debug,
    E: EventTrait + Send + fmt::Debug + 'static,
    I: Clone + Send + Sync + 'static,
    Q: Send + fmt::Debug + Sync + 'static,
    R: Send + Sync + fmt::Debug + 'static,
{
    let (sender, mut receiver) = mpsc::channel::<ActorCommand<E, Q, R>>(100);
    let initial_snapshot = logic.get_initial_snapshot(options.input);
    let snapshot_arc = Arc::new(RwLock::new(initial_snapshot));
    let actor_id = options.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let actor_ref = ActorRefImpl {
        id: actor_id.clone(),
        sender,
        snapshot: snapshot_arc.clone(),
        _query_marker: PhantomData,
        _response_marker: PhantomData,
    };

    let cloned_ref = actor_ref.clone();
    tokio::spawn(async move {
        let mut current_snapshot = snapshot_arc.read().await.clone();
        // logic.started().await; // Need mutable logic

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
                            eprintln!("Actor [{}] transition error: {}", cloned_ref.id(), e);
                        }
                    }
                }
                ActorCommand::Query(query, responder) => {
                    // TODO: Implement query handling if ActorLogic supports it
                    eprintln!(
                        "Actor [{}] query received, but query handling not fully implemented.",
                        cloned_ref.id()
                    );
                    // let _ = responder.send(Err(StateError::NotImplemented("Query handling".into())));
                }
                ActorCommand::Stop => {
                    eprintln!("Actor [{}] stopping.", cloned_ref.id());
                    break;
                }
            }
        }
        // logic.stopped().await; // Need mutable logic
        eprintln!("Actor [{}] task finished.", cloned_ref.id());
    });

    actor_ref
}

pub fn spawn<L, S, E, I, Q, R>(logic: L, options: ActorOptions<I>) -> Box<dyn ActorRef<E, S>>
where
    L: ActorLogic<S, E> + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static + fmt::Debug,
    E: EventTrait + Send + fmt::Debug + 'static,
    I: Clone + Send + Sync + 'static,
    Q: Send + fmt::Debug + Sync + 'static,
    R: Send + Sync + fmt::Debug + 'static,
{
    let actor_ref_impl: ActorRefImpl<E, S, Q, R> = create_actor(logic, options);
    Box::new(actor_ref_impl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::StateError;
    use crate::state::StateType;
    use serde_json::json;
    use std::fmt::Display;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, Hash)]
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
        fn get_type(&self) -> StateType {
            StateType::Atomic
        }
        fn parent(&self) -> Option<String> {
            None
        }
        fn children(&self) -> Vec<String> {
            Vec::new()
        }
        fn initial(&self) -> Option<String> {
            None
        }
        fn history(&self) -> bool {
            false
        }
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq)]
    struct TestEvent(String);

    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            &self.0
        }
        fn payload(&self) -> Option<Value> {
            None
        }
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestQuery(String);

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestResponse(String);

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestSnapshot {
        state: TestState,
        event_count: u32,
    }

    #[derive(Clone, Debug, Default)]
    struct TestActor {
        state: TestState,
        event_count: u32,
    }

    #[async_trait::async_trait]
    trait Actor<E, S>
    where
        E: EventTrait + Send + 'static,
        S: Clone + Send + Sync + 'static,
    {
        type Query;
        type Response;

        async fn handle_event(&mut self, event: E);
        async fn handle_query(&self, query: Self::Query) -> Self::Response;
        fn snapshot(&self) -> S;
    }

    #[async_trait::async_trait]
    impl Actor<TestEvent, TestSnapshot> for TestActor {
        type Query = TestQuery;
        type Response = TestResponse;

        async fn handle_event(&mut self, event: TestEvent) {
            self.state = TestState(format!("Processed: {}", event.0));
            self.event_count += 1;
            debug!(actor_state = ?self.state, event_count = self.event_count, "Handled event");
        }

        async fn handle_query(&self, query: Self::Query) -> Self::Response {
            debug!(query = ?query, "Handling query");
            TestResponse(format!(
                "Responding to: {} with state {:?}",
                query.0, self.state
            ))
        }

        fn snapshot(&self) -> TestSnapshot {
            debug!(state = ?self.state, event_count = self.event_count, "Creating snapshot");
            TestSnapshot {
                state: self.state.clone(),
                event_count: self.event_count,
            }
        }
    }

    #[async_trait::async_trait]
    impl QueryableEvent for TestEvent {
        type Query = TestQuery;
        type Response = TestResponse;

        fn into_query(self) -> Option<(Self::Query, oneshot::Sender<Self::Response>)> {
            if self.0.starts_with("QUERY:") {
                let (_tx, rx) = oneshot::channel();
                Some((TestQuery(self.0.replace("QUERY:", "")), _tx))
            } else {
                None
            }
        }
    }

    #[tokio::test]
    async fn test_actor_spawn_and_stop() {
        let actor = TestActor::default();
        let (handle, command_sender) = spawn_actor(actor, 10);

        drop(command_sender);
        let result = handle.await;
        assert!(result.is_ok(), "Actor task failed to join");
        info!("Actor stopped gracefully.");
    }

    #[tokio::test]
    async fn test_actor_send_event() {
        let actor = TestActor::default();
        let (handle, command_sender) = spawn_actor(actor, 10);

        let event = TestEvent("Test Event 1".to_string());
        let cmd = ActorCommand::Event(event.clone());
        command_sender
            .send(cmd)
            .await
            .expect("Failed to send event");

        // Give the actor some time to process the event
        time::sleep(Duration::from_millis(50)).await;

        // Stop the actor by dropping the sender
        drop(command_sender);
        let _ = handle.await; // Wait for the actor task to finish
        info!("Event sent and actor processed.");
    }

    #[tokio::test]
    async fn test_actor_query() {
        let actor = TestActor::default();
        let actor_ref = spawn_actor(actor, 10);

        let query = TestQuery("MyQuery".to_string());
        let response = actor_ref.query(query).await.unwrap();

        // Adjust expected response based on the simplified handle_query implementation
        assert!(response.0.contains("Responding to: MyQuery"));
        assert!(response.0.contains("Initial State")); // Assuming default state is "Initial State"

        actor_ref.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_snapshot() {
        let mut actor = TestActor::default();
        // Simulate some events
        actor.handle_event(TestEvent("Inc1".to_string())).await;
        actor.handle_event(TestEvent("Inc2".to_string())).await;

        let actor_ref = spawn_actor(actor, 10);

        // Give time for potential initialization if needed, though handle_event was called directly
        time::sleep(Duration::from_millis(50)).await;

        let snapshot = actor_ref.get_snapshot(); // Use get_snapshot which reads the Arc<RwLock>
        assert_eq!(snapshot.state, TestState("Processed: Inc2".to_string())); // Check final state
        assert_eq!(snapshot.event_count, 2); // Check event count

        actor_ref.stop().await.unwrap();
    }
}
