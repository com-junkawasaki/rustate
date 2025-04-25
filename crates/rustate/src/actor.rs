use crate::error::StateError;
use crate::event::EventTrait;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
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
pub trait ActorRef<TEvent: EventTrait, TSnapshot>: Send + Sync + fmt::Debug {
    /// Sends an event to the actor.
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

// Minimal EventObject trait (adapt as needed)
// pub trait EventObject: Clone + Send + Sync + fmt::Debug + Any {
//     fn event_type(&self) -> &str;
//     // Potentially add methods for payload access
// }
// Using the existing one from event.rs for now.

// --- Actor Options ---
#[derive(Debug, Default)]
pub struct ActorOptions<TInput> {
    /// Optional input data for the actor logic's initial snapshot.
    pub input: Option<TInput>,
    /// Optional custom ID for the actor.
    pub id: Option<String>,
    // TODO: Add options for parent actor, system, etc.
}

// --- Concrete Actor Reference Implementation ---
#[derive(Debug)]
pub struct ActorRefImpl<TEvent: EventTrait, TSnapshot> {
    id: String,
    // Channel sender to send events *to* the actor task
    event_sender: mpsc::Sender<TEvent>,
    // Shared snapshot state, updated by the actor task
    snapshot: Arc<Mutex<TSnapshot>>,
    // TODO: Add a way to signal stop
    // stop_sender: Option<oneshot::Sender<()>>,
}

// Implement Clone manually if needed (usually Arc::clone is sufficient for refs)
impl<TEvent: EventTrait, TSnapshot> Clone for ActorRefImpl<TEvent, TSnapshot> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            event_sender: self.event_sender.clone(),
            snapshot: Arc::clone(&self.snapshot),
            // stop_sender: self.stop_sender.clone(), // Needs careful handling if used
        }
    }
}

impl<TEvent: EventTrait, TSnapshot: Clone + Send + Sync + 'static> ActorRef<TEvent, TSnapshot>
    for ActorRefImpl<TEvent, TSnapshot>
{
    fn send(&self, event: TEvent) -> Result<(), StateError> {
        // Try to send the event, handle potential channel closed error
        self.event_sender.try_send(event).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => StateError::ActorMailboxFull(self.id.clone()),
            mpsc::error::TrySendError::Closed(_) => StateError::ActorStopped(self.id.clone()),
        })
        // Or use blocking send if acceptable:
        // self.event_sender.blocking_send(event).map_err(|_| StateError::ActorStopped(self.id.clone()))
        // Or make ActorRef::send async
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn get_snapshot(&self) -> TSnapshot {
        // Lock the mutex to access the snapshot.
        // This blocks if the actor task is currently updating it.
        // Consider using RwLock for better read performance if updates are less frequent.
        self.snapshot.lock().unwrap().clone() // Unwraps are generally discouraged in library code
                                              // TODO: Handle potential poison errors gracefully
    }

    // TODO: Implement stop method
    // fn stop(&self) -> Result<(), StateError> { ... }
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
