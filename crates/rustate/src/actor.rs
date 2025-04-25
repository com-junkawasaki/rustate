use crate::error::StateError;
use crate::error::Result;
use crate::event::EventTrait;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

// --- Snapshot ---

/// Represents the immutable state of an actor at a specific point in time.
/// Generic over the actor's context type `TContext`.
#[derive(Debug, Clone, PartialEq)] // Add Serialize, Deserialize if needed
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorStatus {
    Active,
    Done,
    Error,
    Stopped,
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
}

// --- Actor Reference ---

/// A reference to a running actor instance. Used to send events and potentially subscribe to snapshots.
///
/// Generic Parameters:
/// - TEvent: The type of event the actor accepts.
/// - TSnapshot: The type of snapshot the actor emits.
pub trait ActorRef<TEvent: EventTrait, TSnapshot>: Send + Sync + fmt::Debug
where
    TEvent: EventTrait + Send + fmt::Debug,       // Added Send + Debug bound
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug, // Added Debug bound
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
pub enum ActorCommand<E: EventTrait, Q, R> {
    Event(E),
    Query { query: Q, responder: oneshot::Sender<R> },
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
        Err(StateError::UnsupportedOperation("Query not supported".to_string()))
    }
}

/// Trait for events that support a query/response pattern.
pub trait QueryableEvent: EventTrait {
    type Query: Send;
    type Response: Send + fmt::Debug;
}

/// Represents a reference to an actor, allowing messages to be sent.
pub trait ActorRef<TEvent, TSnapshot>: Send + Sync + fmt::Debug
where
    TEvent: EventTrait + Send + fmt::Debug,       // Added Send + Debug bound
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug, // Added Debug bound
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
}

/// Implementation of ActorRef using a Tokio MPSC channel.
#[derive(fmt::Debug)] // Use fmt::Debug derive
pub struct ActorRefImpl<TEvent, TSnapshot, Q = (), R = ()>
where
    TEvent: EventTrait + Send + fmt::Debug + 'static,       // Added Send + Debug bound
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug, // Added Debug bound
    Q: Send + fmt::Debug + 'static, // Bound for Query type
    R: Send + fmt::Debug + 'static, // Bound for Response type
{
    sender: mpsc::Sender<ActorCommand<TEvent, Q, R>>, // Use ActorCommand with generics
    _snapshot_marker: std::marker::PhantomData<TSnapshot>,
}

impl<TEvent, TSnapshot, Q, R> Clone for ActorRefImpl<TEvent, TSnapshot, Q, R>
where
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + 'static,
    R: Send + fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _snapshot_marker: std::marker::PhantomData,
        }
    }
}

impl<TEvent, TSnapshot, Q, R> ActorRef<TEvent, TSnapshot> for ActorRefImpl<TEvent, TSnapshot, Q, R>
where
    TEvent: EventTrait + Send + fmt::Debug + 'static,
    TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
    Q: Send + fmt::Debug + 'static,
    R: Send + fmt::Debug + 'static,
{
    fn send(&self, event: TEvent) -> Result<(), StateError> {
        self.sender
            .try_send(ActorCommand::Event(event))
            .map_err(|e| StateError::ActorSendError(format!("Failed to send event: {}", e)))
    }

    fn stop(&self) -> Result<(), StateError> {
        self.sender
            .try_send(ActorCommand::Stop)
            .map_err(|e| StateError::ActorSendError(format!("Failed to send stop command: {}", e)))
    }

    async fn query(&self, query: TEvent::Query) -> Result<TEvent::Response, StateError>
    where
        TEvent: QueryableEvent<Query = Q, Response = R>, // Ensure Query/Response match Q/R
        TEvent::Query: fmt::Debug,
        TEvent::Response: fmt::Debug,
    {
        let (responder, receiver) = oneshot::channel();
        self.sender
            .send(ActorCommand::Query { query, responder })
            .await
            .map_err(|e| StateError::ActorSendError(format!("Failed to send query: {}", e)))?;
        receiver
            .await
            .map_err(|e| StateError::ActorReceiveError(format!("Query responder dropped: {}", e)))?
    }

    fn clone_ref(&self) -> Box<dyn ActorRef<TEvent, TSnapshot>> {
        Box::new(self.clone())
    }
}

// Actor lifecycle management function
pub async fn run_actor<
A: Actor<TEvent, TSnapshot> + Send + 'static,
TEvent: EventTrait + Send + fmt::Debug + 'static,
TSnapshot: Clone + Send + Sync + 'static + fmt::Debug,
Q: Send + fmt::Debug + 'static, // Add Q/R generics if Actor/ActorCommand use them
R: Send + fmt::Debug + 'static,
>(
    mut actor: A,
    mut receiver: mpsc::Receiver<ActorCommand<TEvent, Q, R>>,
) {
    while let Some(command) = receiver.recv().await {
        match command {
            ActorCommand::Event(event) => {
                if let Err(e) = actor.handle_event(event).await {
                    eprintln!("Actor error handling event: {}", e);
                }
            }
            ActorCommand::Query { query, responder } => {
                let result = actor.handle_query(query).await;
                let _ = responder.send(result);
            }
            ActorCommand::Stop => {
                println!("Actor stopping...");
                break;
            }
        }
    }
    println!("Actor finished.");
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
    TEvent: QueryableEvent<Query=Q, Response=R>, // Add bound if query is used
{
    let (sender, receiver) = mpsc::channel::<ActorCommand<TEvent, Q, R>>(buffer_size);
    let actor_ref_impl = ActorRefImpl {
        sender,
        _snapshot_marker: std::marker::PhantomData::<TSnapshot>,
    };

    tokio::spawn(run_actor(actor, receiver));

    Box::new(actor_ref_impl)
}

// --- create_actor function ---

/// Creates and starts a new actor instance based on the provided logic.
///
/// Returns an `ActorRef` to interact with the spawned actor.
pub fn create_actor<L, S, E, I>(logic: L, options: ActorOptions<I>) -> ActorRefImpl<E, S>
where
    L: ActorLogic<S, E, I> + Clone + Send + Sync + 'static, // Logic needs to be Clone + Send + Sync + 'static
    S: Clone + Send + Sync + 'static, // Snapshot needs to be Clone + Send + Sync + 'static
    E: EventTrait + Send + Sync + 'static, // Event needs to be Send + Sync + 'static
    I: Send + Sync + 'static,         // Input needs to be Send + Sync + 'static
{
    let actor_id = options
        .id
        .unwrap_or_else(|| format!("actor-{}", Uuid::new_v4()));
    let initial_snapshot = logic.get_initial_snapshot(options.input);

    // Create a channel for sending events to the actor's task
    // Choose buffer size appropriately (e.g., 100)
    let (event_sender, mut event_receiver) = mpsc::channel::<E>(100);

    // Use Arc<Mutex> to allow the actor task to update the snapshot
    // and the ActorRef to read it.
    let snapshot_arc = Arc::new(Mutex::new(initial_snapshot));
    let snapshot_clone_for_task = Arc::clone(&snapshot_arc);

    // Spawn the actor's event processing loop as a background task
    tokio::spawn(async move {
        let mut current_snapshot = snapshot_clone_for_task.lock().unwrap().clone(); // Get initial state
                                                                                    // TODO: Handle potential poison errors

        while let Some(event) = event_receiver.recv().await {
            match logic.transition(current_snapshot.clone(), event).await {
                // Use clone for transition
                Ok(next_snapshot) => {
                    // Update the shared snapshot state
                    let mut snapshot_guard = snapshot_clone_for_task.lock().unwrap();
                    // TODO: Handle potential poison errors
                    *snapshot_guard = next_snapshot.clone();
                    current_snapshot = next_snapshot; // Update local copy for next transition

                    // TODO: Check snapshot status (Done, Error) and potentially stop the loop
                    // if current_snapshot.status() != ActorStatus::Active { break; }
                }
                Err(e) => {
                    // TODO: Handle transition errors appropriately
                    // - Update snapshot status to Error?
                    // - Log the error?
                    // - Stop the actor?
                    eprintln!("Actor [{}] transition error: {}", actor_id, e);
                    // For now, continue processing, might stop on error:
                    // break;
                }
            }
        }
        // Loop ends when the event_sender is dropped (or explicitly stopped)
        println!("Actor [{}] task finished.", actor_id);
        // TODO: Update snapshot status to Stopped?
    });

    // Return the ActorRef implementation
    ActorRefImpl {
        id: actor_id,
        event_sender,
        snapshot: snapshot_arc,
    }
}

pub fn spawn<A>(mut actor: A, options: ActorOptions) -> ActorRef<A::Event, A::Query, A::Response>
where
    A: Actor + Send + 'static,
    A::Event: Send,
    A::Query: Send,
    A::Response: Send,
{
    let (command_sender, mut command_receiver) = mpsc::channel(options.mailbox_capacity);
    let actor_id = options
        .id
        .unwrap_or_else(|| format!("actor-{}", uuid::Uuid::new_v4()));

    let actor_id_clone = actor_id.clone(); // Clone actor_id here
    tokio::spawn(async move {
        let mut status = ActorStatus::Active;
        println!("Actor [{}] started.", actor_id_clone);

        while let Some(command) = command_receiver.recv().await {
            if status == ActorStatus::Stopped {
                println!(
                    "Actor [{}] received command while stopped, ignoring.",
                    actor_id_clone
                );
                continue;
            }

            match command {
                ActorCommand::Event(event) => {
                    if let Err(e) = actor.handle_event(event).await {
                        eprintln!(
                            "Actor [{}] event handling error: {}",
                            actor_id_clone, // Use cloned id
                            e
                        );
                        // Optionally change status or stop based on error
                    }
                }
                ActorCommand::Query { query, responder } => {
                    match actor.handle_query(query).await {
                        Ok(response) => {
                            if responder.send(Ok(response)).is_err() {
                                eprintln!(
                                    "Actor [{}] failed to send query response.",
                                    actor_id_clone // Use cloned id
                                );
                            }
                        }
                        Err(e) => {
                            if responder.send(Err(e)).is_err() {
                                eprintln!(
                                    "Actor [{}] failed to send query error response.",
                                    actor_id_clone // Use cloned id
                                );
                            }
                        }
                    }
                }
                ActorCommand::Stop => {
                    println!("Actor [{}] stopping...", actor_id_clone);
                    status = ActorStatus::Stopped;
                    actor.stopped().await;
                    break; // Exit the loop
                }
            }
        }

        println!("Actor [{}] terminated.", actor_id_clone);
    });

    ActorRef {
        id: actor_id, // Original actor_id here
        sender: command_sender,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::sync::Mutex;

    #[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
    struct TestEvent { event_type: String }
    impl EventTrait for TestEvent {
        fn event_type(&self) -> &str { &self.event_type }
        fn payload(&self) -> Option<&serde_json::Value> { None }
    }
    // Dummy QueryableEvent impl if needed for spawn_actor bounds
    impl QueryableEvent for TestEvent {
        type Query = ();
        type Response = Result<(), StateError>; // Example response
    }


    #[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
    struct TestSnapshot { value: i32 }

    // Test Actor using Arc<Mutex> for state sharing needed by actor_ref() -> &dyn ActorRef
    struct TestActor {
        actor_ref: Arc<Mutex<Option<Box<dyn ActorRef<TestEvent, TestSnapshot>>>>>,
        state: TestSnapshot,
    }

    #[async_trait]
    impl Actor<TestEvent, TestSnapshot> for TestActor {
        // This setup is complex because the actor needs a ref to itself *before* it's fully spawned.
        // A common pattern is to inject the ActorRef after creation/spawn.
        // For this test, we'll assume the ref is somehow set.
        fn actor_ref(&self) -> &dyn ActorRef<TestEvent, TestSnapshot> {
            self.actor_ref.lock().unwrap().as_ref().unwrap().as_ref()
        }
        async fn handle_event(&mut self, event: TestEvent) -> Result<(), StateError> {
            println!("TestActor handled event: {:?}", event);
            self.state.value += 1;
            Ok(())
        }
        fn get_snapshot(&self) -> TestSnapshot {
            self.state.clone()
        }
        // Add handle_query if testing queries
    }

    #[tokio::test]
    async fn test_actor_spawn_send_stop() {
        let initial_state = TestSnapshot { value: 0 };
        let actor_ref_storage = Arc::new(Mutex::new(None));

        let actor = TestActor {
            actor_ref: actor_ref_storage.clone(), // Clone Arc for the actor
            state: initial_state,
        };

        // Spawn the actor and get the actual ActorRef
        let actor_ref = spawn_actor::<_, _, _, (), Result<(), StateError>>(actor, 10); // Specify Q/R

        // Store the spawned actor_ref so the actor instance can access it
        *actor_ref_storage.lock().unwrap() = Some(actor_ref.clone_ref());

        // Send an event
        let event = TestEvent { event_type: "INCREMENT".to_string() };
        let send_result = actor_ref.send(event);
        assert!(send_result.is_ok(), "Send failed: {:?}", send_result.err());

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Stop the actor using the ActorRef trait method
        let stop_result = actor_ref.stop();
        assert!(stop_result.is_ok(), "Stop failed: {:?}", stop_result.err());

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Try sending after stop
        let event_after_stop = TestEvent { event_type: "INCREMENT_AGAIN".to_string() };
        let send_after_stop_result = actor_ref.send(event_after_stop);
        assert!(send_after_stop_result.is_err());
        match send_after_stop_result.err().unwrap() {
            StateError::ActorSendError(_) => {} // Expected
            e => panic!("Unexpected error type after stop: {:?}", e),
        }
    }
}
