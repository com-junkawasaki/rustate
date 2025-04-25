use crate::error::Result;
use crate::error::StateError;
use crate::event::EventTrait;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot, RwLock};
use uuid::Uuid;
use crate::state::StateTrait;
use crate::event::QueryableEvent;
use std::marker::PhantomData;
use crate::{
    ActorRef,
    ActorOptions,
};

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
    pub fn new(value: serde_json::Value, context: TContext, output: Option<TOutput>, status: ActorStatus) -> Self {
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
pub trait ActorRef<TEvent: EventTrait, TSnapshot>: Send + Sync + fmt::Debug
where
    TEvent: EventTrait + Send + fmt::Debug, // Added Send + Debug bound
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

/// Trait defining the behavior of an actor (state machine processor).
pub trait Actor<TEvent: EventTrait, TSnapshot> {
    /// Returns a reference to the actor's mailbox sender.
    fn actor_ref(&self) -> &dyn ActorRef<TEvent, TSnapshot>;

    /// Handles an incoming event, potentially transitioning the state.
    async fn handle_event(&mut self, event: TEvent) -> Result<(), StateError>;

    /// Returns the current state snapshot of the actor.
    fn get_snapshot(&self) -> TSnapshot;

    /// Optional: Handle a query message (can be used for synchronous requests).
    async fn handle_query(&self, query: TEvent::Query) -> Result<TEvent::Response, StateError>
    where
        TEvent: QueryableEvent, // Only if the event type supports queries
    {
        Err(StateError::UnsupportedOperation(
            "Query not supported".to_string(),
        ))
    }

    async fn started(&mut self) {}
    async fn stopped(&mut self) {}
}

/// Trait for events that support a query/response pattern.
pub trait QueryableEvent: EventTrait {
    type Query: Send;
    type Response: Send + fmt::Debug;
}

/// Represents a reference to an actor, allowing messages to be sent.
pub trait ActorRef<TEvent, TSnapshot>: Send + Sync + fmt::Debug
where
    TEvent: EventTrait + Send + fmt::Debug, // Added Send + Debug bound
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
{
    /// Sends an event to the actor asynchronously.
    fn send(&self, event: TEvent) -> Result<(), StateError>;

    /// Stops the actor.
    fn stop(&self) -> Result<(), StateError>;

    /// Optional: Sends a query and waits for a response.
    async fn query(&self, query: TEvent::Query) -> Result<TEvent::Response, StateError>
    where
        TEvent: QueryableEvent,
        TEvent::Query: fmt::Debug,
        TEvent::Response: fmt::Debug;

    // Clones the ActorRef (typically involves cloning an Arc or channel sender).
    fn clone_ref(&self) -> Box<dyn ActorRef<TEvent, TSnapshot>>;

    fn get_snapshot(&self) -> TSnapshot;
}

/// Implementation of ActorRef using a Tokio MPSC channel.
#[derive(fmt::Debug)] // Use fmt::Debug derive
pub struct ActorRefImpl<TEvent, TSnapshot, Q = (), R = Result<(), StateError>>
where
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + Sync + 'static,
    R: Send + Sync + fmt::Debug + 'static, // Added Sync bound
    // TEvent: QueryableEvent<Query = Q, Response = R>, // Removed QueryableEvent bound for now
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

impl<TEvent, TSnapshot, Q, R> ActorRef<TEvent, TSnapshot> for ActorRefImpl<TEvent, TSnapshot, Q, R>
where
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + Sync + 'static,
    R: Send + Sync + fmt::Debug + 'static,
    // TEvent: QueryableEvent<Query = Q, Response = R>, // Removed QueryableEvent bound
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

    async fn query(&self, query: TEvent::Query) -> Result<TEvent::Response, StateError>
    where
        TEvent::Query: fmt::Debug,
        TEvent::Response: fmt::Debug,
    {
        let (responder, response_receiver) = oneshot::channel();
        self.sender
            .send(ActorCommand::Query { query, responder })
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
    options: ActorOptions<I>
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
                    eprintln!("Actor [{}] query received, but query handling not fully implemented.", cloned_ref.id());
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

pub fn spawn<L, S, E, I, Q, R>(
    logic: L, 
    options: ActorOptions<I>
) -> Box<dyn ActorRef<E, S>>
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
    use serde::{Deserialize, Serialize};
    use std::sync::Mutex;

    #[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
    struct TestEvent {
        event_type: String,
    }
    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str {
            &self.event_type
        }
        fn payload(&self) -> Option<&serde_json::Value> {
            None
        }
    }
    impl QueryableEvent for TestEvent {
        type Query = ();
        type Response = Result<(), StateError>;
    }

    #[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
    struct TestSnapshot {
        value: i32,
    }

    struct TestActor {
        actor_ref: Arc<Mutex<Option<Box<dyn ActorRef<TestEvent, TestSnapshot>>>>>,
        state: TestSnapshot,
        id: String,
    }

    #[async_trait]
    impl Actor<TestEvent, TestSnapshot> for TestActor {
        fn actor_ref(&self) -> Box<dyn ActorRef<TestEvent, TestSnapshot>> {
            self.actor_ref
                .lock()
                .unwrap()
                .as_ref()
                .expect("ActorRef not set before access in actor_ref")
                .clone_ref()
        }
        async fn handle_event(&mut self, event: TestEvent) -> Result<(), StateError> {
            println!("TestActor handled event: {:?}", event);
            self.state.value += 1;
            Ok(())
        }
        fn get_snapshot(&self) -> TestSnapshot {
            self.state.clone()
        }
        async fn stopped(&mut self) {
            println!("Actor [{}] stopped handler called.", self.id);
        }
        async fn handle_query(&mut self, query: TestQuery) -> Result<TestResponse, StateError> {
            println!("Actor [{}] handled query: {:?}", self.id, query);
            Ok(Ok(format!("Response to '{}'", query)))
        }
    }

    #[tokio::test]
    async fn test_actor_spawn_send_stop() {
        let initial_state = TestSnapshot { value: 0 };
        let actor_id = "test-actor-1".to_string();
        let actor_ref_storage: Arc<Mutex<Option<Box<dyn ActorRef<TestEvent, TestSnapshot>>>>> =
            Arc::new(Mutex::new(None));

        let (sender, _) = mpsc::channel::<ActorCommand<TestEvent, TestQuery, TestResponse>>(10);
        let initial_actor_ref = ActorRefImpl {
            id: actor_id.clone(),
            sender,
            _snapshot_marker: std::marker::PhantomData,
            _response_marker: std::marker::PhantomData,
        };
        let initial_actor_ref_boxed: Box<dyn ActorRef<TestEvent, TestSnapshot>> =
            Box::new(initial_actor_ref);

        let actor = TestActor {
            actor_ref: Arc::new(Mutex::new(Some(initial_actor_ref_boxed.clone_ref()))),
            state: initial_state,
            id: actor_id.clone(),
        };

        let actor_ref = spawn_actor::<_, _, _, _, _>(actor, 10);

        *actor_ref_storage.lock().unwrap() = Some(actor_ref.clone_ref());

        let event = TestEvent {
            event_type: "INCREMENT".to_string(),
        };
        let send_result = actor_ref.send(event);
        assert!(send_result.is_ok(), "Send failed: {:?}", send_result.err());

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let query_result = actor_ref.query("Hello?".to_string()).await;
        assert!(
            query_result.is_ok(),
            "Query failed: {:?}",
            query_result.err()
        );
        assert_eq!(query_result.unwrap().unwrap(), "Response to 'Hello?'");

        let stop_result = actor_ref.stop();
        assert!(stop_result.is_ok(), "Stop failed: {:?}", stop_result.err());

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let event_after_stop = TestEvent {
            event_type: "INCREMENT_AGAIN".to_string(),
        };
        let send_after_stop_result = actor_ref.send(event_after_stop);
        assert!(send_after_stop_result.is_err());
        match send_after_stop_result.err().unwrap() {
            StateError::ActorSendError(_) => {}
            e => panic!("Unexpected error type after stop: {:?}", e),
        }

        let query_after_stop_result = actor_ref.query("Still there?".to_string()).await;
        assert!(query_after_stop_result.is_err());
        match query_after_stop_result.err().unwrap() {
            StateError::ActorSendError(_) | StateError::ActorReceiveError(_) => {}
            e => panic!("Unexpected error type after stop query: {:?}", e),
        }
    }
}
