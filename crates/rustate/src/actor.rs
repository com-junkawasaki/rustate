#![allow(dead_code)] // Allow dead code for now during refactoring

/// Provides a concrete actor implementation based on Tokio MPSC channels.
///
/// This module defines the traits, structs, and functions necessary to create,
/// run, and interact with actors that manage state and context according to
/// defined logic (`ActorLogic`). It includes features like event handling,
/// state querying, snapshots, and lifecycle management.
///
/// Compare this with `rustate_core` which provides more foundational, abstract
/// actor concepts. This module offers a ready-to-use implementation.
use crate::error::{Result, StateError};
use crate::event::EventTrait;
use crate::state::StateTrait;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn}; // Use tracing macros
use uuid::Uuid;

// --- Actor Options ---

/// Configuration options for creating an actor.
///
/// (Currently unused in the main `create_actor` flow, might be for future extensions).
#[derive(Debug, Clone)]
pub struct ActorOptions<I: Send + Sync + 'static> {
    /// Optional custom ID for the actor. If None, a UUID will be generated.
    pub id: Option<String>,
    /// Optional input data provided to the actor upon creation.
    pub input: Option<I>,
}

// --- Snapshot ---

/// Represents the immutable state of an actor at a specific point in time.
///
/// Snapshots are useful for debugging, persistence, or transferring state.
/// Generic over the actor's context type `TContext` and potential output type `TOutput`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot<TContext, TOutput = ()> {
    /// The current state value (e.g., a simple enum variant name or a more complex structure).
    /// Using `serde_json::Value` provides flexibility for representing potentially hierarchical
    /// or complex state identifiers, similar to XState. For simpler state machines,
    /// this could be directly the state enum variant if it derives Serialize/Deserialize.
    /// Consider defining a more specific representation based on machine definitions for
    /// improved type safety if needed.
    pub value: serde_json::Value,
    /// The current context (extended state) of the actor.
    pub context: TContext,
    /// The output value produced if the actor has reached a final state.
    /// This is `None` if the actor is not in a final state or doesn't produce output.
    pub output: Option<TOutput>,
    /// The current lifecycle status of the actor.
    pub status: ActorStatus,
    // Potential future additions:
    // - historyValue: For restoring state in hierarchical/parallel machines.
    // - error: Information about any error that caused the actor to stop.
    // - tags: Set of active tags based on the current state.
}

impl<TContext, TOutput> Snapshot<TContext, TOutput> {
    /// Creates a new snapshot.
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

/// Represents the lifecycle status of an actor instance.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorStatus {
    /// The actor is running and processing messages.
    Active,
    /// The actor has reached a final state and successfully completed its work.
    Done,
    /// The actor has stopped processing messages (explicitly or implicitly).
    Stopped,
    /// The actor encountered an error during processing.
    Error,
}

// --- Actor Logic Trait (Specific to this implementation) ---

/// Defines the behavior and state transition logic for an actor within this module's implementation.
///
/// This trait separates the core state machine logic from the actor's execution and communication concerns.
#[async_trait]
pub trait ActorLogic<S, C, E, I, Q, Resp>: Send + Sync
where
    S: StateTrait + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static, // Input type for actor initialization (optional)
    Q: Send + Sync + 'static, // Query type
    Resp: Send + Sync + 'static,
{
    /// Returns the initial state and context when the actor starts.
    /// Potentially use the `input: I` here in future versions.
    fn initial(&self) -> (S, C);

    /// Processes an event and attempts to transition the state machine.
    ///
    /// # Arguments
    /// * `state` - The current state.
    /// * `context` - The current context (passed immutably, modifications should happen within the logic if needed, returning a new context).
    /// * `event` - The event to process.
    ///
    /// # Returns
    /// A `Result` containing the new state and context if a transition occurred,
    /// or an `Err(StateError)` if processing failed. If no transition occurs for the event,
    /// it should typically return `Ok((state, context))`.
    async fn transition(&self, state: S, context: C, event: E) -> Result<(S, C), StateError>;

    /// Handles a query request without changing the actor's state.
    ///
    /// # Arguments
    /// * `state` - A reference to the current state.
    /// * `context` - A reference to the current context.
    /// * `query` - The query to process.
    ///
    /// # Returns
    /// The response (`Resp`) to the query.
    async fn handle_query(&self, state: &S, context: &C, query: Q) -> Resp;

    // Potential future additions:
    // - on_entry/on_exit actions specific to this logic trait
    // - Access to actor's own ActorRef for self-messaging
}

// --- Actor Trait (Defines the core actor behavior instance) ---

/// Defines the methods that a running actor instance must provide.
/// This is typically implemented by the internal actor runner struct.
#[async_trait]
pub trait ActorTrait<E, Q, Resp>: Send + Sync
where
    E: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Returns the unique ID of this actor instance.
    fn id(&self) -> Uuid;

    /// Handles an incoming event. Internal state updates happen here.
    async fn handle_event(&mut self, event: E) -> Result<(), StateError>;

    /// Handles an incoming query, sending the response via the provided channel.
    async fn handle_query(&self, query: Q, responder: oneshot::Sender<Resp>);

    /// Called when the actor task starts running.
    async fn started(&mut self);

    /// Called when the actor task is stopping.
    async fn stopped(&mut self);
}

// --- Actor Command Enum ---

/// Internal commands processed by the actor's run loop via its MPSC channel.
#[derive(fmt::Debug)]
pub enum ActorCommand<E, Q, Resp, C, R>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
    C: Send + 'static, // Context type for Snapshot
    R: Send + 'static, // Output type for Snapshot
{
    /// Command to send an external event for processing.
    SendEvent(E),
    /// Command to perform a query on the actor's state/context.
    Query(Q, oneshot::Sender<Resp>),
    /// Command to retrieve a snapshot of the actor's current state and context.
    GetSnapshot(oneshot::Sender<Result<Snapshot<C, R>, StateError>>),
    /// Command to gracefully stop the actor.
    Stop,
}

// --- Queryable Event Trait ---

/// Trait for events that might support a direct query/response interaction.
///
/// Note: The primary query mechanism in this actor implementation is via
/// `ActorCommand::Query` and `ActorRefImpl::query`. This trait might be
/// deprecated or repurposed if not actively used.
pub trait QueryableEvent: EventTrait {
    /// The type of query associated with this event.
    type Query: Send;
    /// The type of response expected for the query.
    type Response: Send + fmt::Debug;
}

// --- ActorRef Implementation STRUCT (The handle) ---

/// A handle (reference) to a running actor instance.
///
/// Provides methods to interact with the actor asynchronously (sending events,
/// querying state, stopping). Cloning `ActorRefImpl` creates another handle
/// pointing to the same actor. The actor stops when all handles are dropped
/// or when explicitly stopped.
///
/// Generic Parameters:
/// * `E`: Event type
/// * `Q`: Query type
/// * `Resp`: Query Response type
/// * `C`: Context type (for Snapshot)
/// * `R`: Output type (for Snapshot)
pub struct ActorRefImpl<E, Q, Resp, C, R>
where
    E: EventTrait + Send + 'static,
    Q: Send + 'static,
    Resp: Send + 'static,
    C: Send + 'static,
    R: Send + 'static,
{
    /// The unique identifier of the actor instance.
    pub id: Uuid,
    /// Sender half of the MPSC channel to the actor's command inbox.
    pub(crate) sender: mpsc::Sender<ActorCommand<E, Q, Resp, C, R>>,
    /// Shared, mutable status of the actor.
    pub status: Arc<RwLock<ActorStatus>>,
    /// Phantom data to satisfy the compiler about unused generic types.
    _phantom: PhantomData<(Q, Resp, C, R)>,
}

// --- ActorRefImpl Send/Query/Stop/Snapshot Methods ---
impl<E, Q, Resp, C, R> ActorRefImpl<E, Q, Resp, C, R>
where
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    Q: Send + Sync + fmt::Debug + 'static,
    Resp: Send + Sync + fmt::Debug + 'static,
    C: Send + Sync + Clone + Debug + 'static, // Context needs Clone + Debug for snapshot
    R: Send + Sync + Clone + Debug + 'static, // Output needs Clone + Debug for snapshot
{
    /// Asynchronously sends an event to the actor for processing.
    ///
    /// # Arguments
    /// * `event` - The event to send.
    ///
    /// # Returns
    /// `Ok(())` if the event was successfully queued, `Err(StateError)` if the actor's
    /// channel is closed (likely because the actor has stopped).
    pub async fn send_event(&self, event: E) -> Result<(), StateError> {
        trace!(actor_id = %self.id, event = ?event, "Sending event");
        if self.sender.is_closed() {
            warn!(actor_id = %self.id, event = ?event, "Attempted to send event to closed actor channel");
            return Err(StateError::SendError(format!(
                "Actor {} channel closed, likely stopped.",
                self.id
            )));
        }
        self.sender
            .send(ActorCommand::SendEvent(event))
            .await
            .map_err(|e| {
                error!(actor_id = %self.id, error = %e, "Failed to send event");
                StateError::SendError(format!("Failed to send event to actor {}: {}", self.id, e))
            })
    }

    /// Asynchronously sends a query to the actor and awaits its response.
    ///
    /// # Arguments
    /// * `query` - The query to send.
    ///
    /// # Returns
    /// A `Result` containing the response (`Resp`) or a `StateError` if the query fails
    /// (e.g., actor stopped, channel closed, timeout).
    pub async fn query(&self, query: Q) -> Result<Resp, StateError> {
        trace!(actor_id = %self.id, query = ?query, "Sending query");
        let (tx, rx) = oneshot::channel();
        if self.sender.is_closed() {
            warn!(actor_id = %self.id, query = ?query, "Attempted to send query to closed actor channel");
            return Err(StateError::SendError(format!(
                "Actor {} channel closed, likely stopped.",
                self.id
            )));
        }
        self.sender
            .send(ActorCommand::Query(query, tx))
            .await
            .map_err(|e| {
                error!(actor_id = %self.id, error = %e, "Failed to send query command");
                StateError::SendError(format!("Failed to send query to actor {}: {}", self.id, e))
            })?;
        rx.await.map_err(|e| {
            warn!(actor_id = %self.id, error = %e, "Failed to receive query response");
            StateError::ReceiveError(format!(
                "Failed to receive query response from actor {}: {}",
                self.id, e
            ))
        })
    }

    /// Asynchronously requests the actor to stop processing.
    ///
    /// This sends a `Stop` command. The actor will finish processing its current
    /// message (if any) before shutting down.
    ///
    /// # Returns
    /// `Ok(())` if the stop command was sent, `Err(StateError)` if the channel was closed.
    pub async fn stop(&self) -> Result<(), StateError> {
        info!(actor_id = %self.id, "Requesting actor stop");
        if self.sender.is_closed() {
            warn!(actor_id = %self.id, "Attempted to send stop command to closed actor channel");
            // If already closed, it's effectively stopped.
            return Ok(());
        }
        self.sender.send(ActorCommand::Stop).await.map_err(|e| {
            error!(actor_id = %self.id, error = %e, "Failed to send stop command");
            StateError::SendError(format!(
                "Failed to send stop command to actor {}: {}",
                self.id, e
            ))
        })
    }

    /// Asynchronously requests a snapshot of the actor's current state and context.
    ///
    /// # Returns
    /// A `Result` containing the `Snapshot<C, R>` or a `StateError` if the request fails.
    pub async fn get_snapshot(&self) -> Result<Snapshot<C, R>, StateError> {
        trace!(actor_id = %self.id, "Requesting snapshot");
        let (tx, rx) = oneshot::channel();
        if self.sender.is_closed() {
            warn!(actor_id = %self.id, "Attempted to get snapshot from closed actor channel");
            return Err(StateError::SendError(format!(
                "Actor {} channel closed, likely stopped.",
                self.id
            )));
        }
        self.sender
            .send(ActorCommand::GetSnapshot(tx))
            .await
            .map_err(|e| {
                error!(actor_id = %self.id, error = %e, "Failed to send GetSnapshot command");
                StateError::SendError(format!(
                    "Failed to send GetSnapshot command to actor {}: {}",
                    self.id, e
                ))
            })?;
        rx.await.map_err(|e| {
            warn!(actor_id = %self.id, error = %e, "Failed to receive snapshot response");
            StateError::ReceiveError(format!(
                "Failed to receive snapshot from actor {}: {}",
                self.id, e
            ))
        })? // Flatten Result<Result<S, E>, E>
    }

    /// Gets the current status of the actor.
    pub async fn get_status(&self) -> ActorStatus {
        *self.status.read().await
    }
}

// --- Internal Actor State ---

/// Holds the internal mutable state of the running actor task.
struct InternalActorState<L, C, E, S, I, Q, R, Resp>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    S: StateTrait + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    R: Send + Sync + 'static + Debug, // Output type
    Resp: Send + Sync + 'static + Debug,
{
    /// The state machine logic implementation.
    logic: Arc<L>,
    /// The current state value.
    state: S,
    /// The current context, wrapped for shared access.
    context: Arc<RwLock<C>>,
    /// Receiver for incoming commands.
    inbox: mpsc::Receiver<ActorCommand<E, Q, Resp, C, R>>,
    /// Shared status indicator.
    status: Arc<RwLock<ActorStatus>>,
    /// Generated output if the machine reaches a final state.
    output: Option<R>, // Store output when done
    /// Phantom data for unused generics.
    _phantom_i: PhantomData<I>,
    // _phantom_r: PhantomData<R>, // R is used in output
}

// --- Actor Implementation (Setup/Runner) ---

/// The main struct representing a concrete actor instance before it's run.
/// It holds the configuration and initial state necessary to spawn the actor task.
/// This struct itself doesn't handle messages directly; it spawns a task that does.
struct ActorImpl<L, C, E, S, I, Q, R, Resp>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    S: StateTrait + Send + Sync + 'static + PartialEq, // Add PartialEq for state change check
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    I: Send + Sync + Debug + 'static + Default,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static + Default + Clone, // Output type
    Resp: Send + Sync + Debug + 'static,
{
    /// Unique identifier for this actor instance.
    id: Uuid,
    /// The state machine logic.
    logic: Arc<L>,
    // Note: initial_state and context are stored here but used to initialize
    // InternalActorState when the task starts. Consider if they are needed here long-term.
    // /// The starting state value.
    // initial_state: S,
    // /// The starting context value.
    // context: C,
    // /// Optional actor ID passed during creation (unused currently).
    // actor_id: Option<Uuid>,
    /// Size of the command buffer.
    buffer_size: usize,
    /// Sender part of the command channel (cloned to create ActorRefImpl).
    sender: mpsc::Sender<ActorCommand<E, Q, Resp, C, R>>,
    /// Receiver part of the command channel (moved into the spawned task).
    inbox: Option<mpsc::Receiver<ActorCommand<E, Q, Resp, C, R>>>, // Made Option to take ownership
    /// Shared status indicator.
    status: Arc<RwLock<ActorStatus>>,
    // /// Stores the latest snapshot (optional).
    // snapshot: Option<Snapshot<C, R>>, // Maybe move snapshot logic elsewhere?
    /// Phantom data for unused generic types.
    _phantom_l: PhantomData<L>,
    _phantom_e: PhantomData<E>,
    _phantom_s: PhantomData<S>,
    _phantom_i: PhantomData<I>,
    _phantom_q: PhantomData<Q>,
    _phantom_r: PhantomData<R>,
    _phantom_resp: PhantomData<Resp>,
    _phantom_c: PhantomData<C>,
}

// --- ActorImpl Methods (Mainly the run loop) ---
impl<L, C, E, S, I, Q, R, Resp> ActorImpl<L, C, E, S, I, Q, R, Resp>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    S: StateTrait + Send + Sync + 'static + PartialEq + Serialize, // State needs Serialize for snapshot value
    E: EventTrait + Send + Sync + fmt::Debug + 'static,
    I: Send + Sync + Debug + 'static + Default,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static + Default + Clone, // Output type
    Resp: Send + Sync + Debug + 'static,
{
    /// Consumes the ActorImpl and runs the actor's main event loop in the current task.
    /// This is intended to be spawned into a separate Tokio task via `tokio::spawn`.
    pub async fn run(mut self) {
        let actor_id = self.id; // Use the ID assigned during creation
        let logic = self.logic.clone(); // Clone Arc for the task

        let (initial_state_val, initial_context_val) = logic.initial();
        let context_arc = Arc::new(RwLock::new(initial_context_val.clone())); // Clone initial context

        let mut internal_state = InternalActorState {
            logic: logic.clone(),
            state: initial_state_val.clone(),
            context: context_arc.clone(),
            inbox: self.inbox.take().expect("Inbox should be present"), // Take ownership
            status: self.status.clone(),
            output: None::<R>, // Initialize output as None
            _phantom_i: PhantomData,
            // _phantom_r: PhantomData,
        };

        info!(actor_id = %actor_id, state = ?initial_state_val, context = ?initial_context_val, "Actor started");
        *internal_state.status.write().await = ActorStatus::Active;

        // --- Actor Event Loop ---
        loop {
            // Check status before receiving next message
            let current_status = *internal_state.status.read().await;
            if current_status != ActorStatus::Active {
                info!(actor_id = %actor_id, status = ?current_status, "Actor loop terminating due to status change.");
                break;
            }

            tokio::select! {
                // Biased select ensures status check happens often
                biased;

                // Check status periodically or on signal (optional, provides faster shutdown)
                // _ = tokio::time::sleep(Duration::from_secs(1)) => { // Example check interval
                //     if *internal_state.status.read().await != ActorStatus::Active {
                //         info!(actor_id = %actor_id, "Actor loop terminating due to status check.");
                //         break;
                //     }
                // }

                // Receive the next command from the inbox
                maybe_command = internal_state.inbox.recv() => {
                    match maybe_command {
                        Some(command) => {
                            trace!(actor_id = %actor_id, command = ?command, "Received command");
                            let mut should_stop = false;
                            match command {
                                ActorCommand::SendEvent(event) => {
                                    let current_s = internal_state.state.clone();
                                    let current_c = internal_state.context.read().await.clone();
                                    debug!(actor_id = %actor_id, event = ?event, state = ?current_s, "Processing event");

                                    match internal_state.logic.transition(current_s.clone(), current_c.clone(), event).await {
                                        Ok((next_s, next_c)) => {
                                            if next_s != current_s {
                                                info!(actor_id = %actor_id, old_state = ?current_s, new_state = ?next_s, "State transitioned");
                                                internal_state.state = next_s;
                                                // Update context only if it changed potentially
                                                // TODO: This needs refinement. If transition guarantees returning the *exact same* Arc<RwLock<C>>
                                                // if no change, we can compare Arcs. If it always returns a new C, we need PartialEq<C>.
                                                // For now, always update the RwLock content.
                                                *internal_state.context.write().await = next_c;

                                                // TODO: Check if next_s is a final state and set status/output
                                                // if internal_state.state.is_final() { ... }

                                            } else {
                                                 trace!(actor_id = %actor_id, state = ?current_s, "State unchanged after event");
                                            }
                                        },
                                        Err(e) => {
                                             error!(actor_id = %actor_id, error = %e, "Error during transition");
                                             *internal_state.status.write().await = ActorStatus::Error;
                                             should_stop = true; // Stop on transition error
                                        }
                                    }
                                }
                                ActorCommand::Query(query, responder) => {
                                    let state_ref = &internal_state.state;
                                    let context_guard = internal_state.context.read().await;
                                    debug!(actor_id = %actor_id, query = ?query, state = ?state_ref, "Handling query");
                                    let response = internal_state.logic.handle_query(state_ref, &context_guard, query).await;
                                    if responder.send(response).is_err() {
                                        warn!(actor_id = %actor_id, "Failed to send query response: receiver dropped");
                                    }
                                }
                                ActorCommand::GetSnapshot(responder) => {
                                     debug!(actor_id = %actor_id, state = ?internal_state.state, "Handling GetSnapshot");
                                     // Serialize state value to JSON
                                     let state_value = match serde_json::to_value(&internal_state.state) {
                                         Ok(v) => v,
                                         Err(e) => {
                                             error!(actor_id = %actor_id, error = %e, "Failed to serialize state for snapshot");
                                             let _ = responder.send(Err(StateError::SerializationError(e.to_string())));
                                             continue; // Skip snapshot creation on serialization error
                                         }
                                     };
                                    let snapshot = Snapshot::new(
                                        state_value,
                                        internal_state.context.read().await.clone(),
                                        internal_state.output.clone(),
                                        *internal_state.status.read().await,
                                    );
                                    if responder.send(Ok(snapshot)).is_err() {
                                         warn!(actor_id = %actor_id, "Failed to send snapshot response: receiver dropped");
                                    }
                                }
                                ActorCommand::Stop => {
                                    info!(actor_id = %actor_id, "Received stop command");
                                    should_stop = true;
                                }
                            }

                            if should_stop {
                                info!(actor_id = %actor_id, "Initiating stop");
                                *internal_state.status.write().await = ActorStatus::Stopped;
                                // Break after setting status, loop will terminate on next iteration's status check
                            }
                        },
                        None => {
                            // Channel closed, means all ActorRefs were dropped.
                            info!(actor_id = %actor_id, "Command channel closed. Actor stopping.");
                            *internal_state.status.write().await = ActorStatus::Stopped;
                            break; // Exit loop naturally
                        }
                    }
                }
            }
        }

        // Actor loop finished
        let final_status = *internal_state.status.read().await;
        info!(actor_id = %actor_id, status = ?final_status, "Actor task finished.");
        // Perform any cleanup if needed
        // logic.stopped() // If ActorLogic had a stopped hook
    }
}

// --- Spawning Function ---

/// Creates and spawns a new actor based on the provided logic.
///
/// This function sets up the necessary communication channels (MPSC) and spawns a Tokio task
/// to run the actor's event loop (`ActorImpl::run`).
///
/// # Type Parameters
/// * `L`: The actor logic type, implementing [`ActorLogic`].
/// * `C`: The context type.
/// * `S`: The state type, implementing [`StateTrait`].
/// * `E`: The event type, implementing [`EventTrait`].
/// * `I`: The input type for the logic (currently unused).
/// * `Q`: The query type.
/// * `R`: The output type produced by the actor (e.g., when reaching a final state).
/// * `Resp`: The response type for queries.
///
/// # Arguments
/// * `logic`: An instance of the actor's logic (`L`).
/// * `initial_context`: The starting context value for the actor.
/// * `actor_id_prefix`: An optional string prefix for the actor's generated UUID.
/// * `buffer_size`: The size of the command channel (mailbox).
///
/// # Returns
/// A tuple containing:
/// * `ActorRefImpl<E, Q, Resp, C, R>`: A handle to interact with the spawned actor.
/// * `JoinHandle<()>`: A handle to the spawned Tokio task, allowing joining/aborting.
pub fn create_actor<L, C, S, E, I, Q, R, Resp>(
    logic: L,
    initial_context: C,
    actor_id_prefix: Option<&str>,
    buffer_size: usize,
) -> (ActorRefImpl<E, Q, Resp, C, R>, JoinHandle<()>)
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    S: StateTrait + Send + Sync + 'static + PartialEq + Serialize, // State needs Serialize for snapshot
    C: Send + Sync + 'static + Default + Clone + Debug,
    E: EventTrait + Send + Sync + 'static + fmt::Debug,
    I: Send + Sync + Debug + 'static + Default,
    Q: Send + Sync + Debug + 'static,
    R: Send + Sync + Debug + 'static + Default + Clone, // Output type
    Resp: Send + Sync + Debug + 'static,
{
    let id = Uuid::new_v4();
    let full_id_str =
        actor_id_prefix.map_or_else(|| id.to_string(), |prefix| format!("{}-{}", prefix, id)); // Use full_id_str for logging if needed, keep UUID for ref ID
    info!(actor_id = %id, prefix = actor_id_prefix, buffer = buffer_size, "Creating actor");

    let (sender, receiver) = mpsc::channel::<ActorCommand<E, Q, Resp, C, R>>(buffer_size);
    let status = Arc::new(RwLock::new(ActorStatus::Stopped)); // Initial status is stopped

    let actor_ref = ActorRefImpl {
        id,
        sender: sender.clone(), // Clone sender for the ref
        status: status.clone(),
        _phantom: PhantomData,
    };

    let actor_impl = ActorImpl {
        id,
        logic: Arc::new(logic),
        // initial_state and context are obtained from logic.initial() inside run()
        // initial_state,
        // context: initial_context,
        // actor_id: Some(id), // Pass the generated ID if needed internally
        buffer_size,
        sender,                // Move original sender
        inbox: Some(receiver), // Pass receiver wrapped in Option
        status,
        // snapshot: None,
        // Phantom data
        _phantom_l: PhantomData,
        _phantom_e: PhantomData,
        _phantom_s: PhantomData,
        _phantom_i: PhantomData,
        _phantom_q: PhantomData,
        _phantom_r: PhantomData,
        _phantom_resp: PhantomData,
        _phantom_c: PhantomData::<C>,
    };

    // Spawn the actor's run loop in a new task
    let join_handle = tokio::spawn(actor_impl.run());

    info!(actor_id = %id, "Actor task spawned");
    (actor_ref, join_handle)
}

// --- Arc<L> Implementation for ActorLogic ---

/// Allows using an `Arc<L>` directly where `L: ActorLogic` is expected.
/// This is useful when the logic needs to be shared.
#[async_trait]
impl<L, S, C, E, I, Q, Resp> ActorLogic<S, C, E, I, Q, Resp> for Arc<L>
where
    L: ActorLogic<S, C, E, I, Q, Resp> + Send + Sync + 'static,
    S: StateTrait + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
    I: Send + Sync + 'static,
    Q: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Delegates to the inner logic's `initial` method.
    fn initial(&self) -> (S, C) {
        self.as_ref().initial()
    }

    /// Delegates to the inner logic's `transition` method.
    async fn transition(&self, state: S, context: C, event: E) -> Result<(S, C), StateError> {
        self.as_ref().transition(state, context, event).await
    }

    /// Delegates to the inner logic's `handle_query` method.
    async fn handle_query(&self, state: &S, context: &C, query: Q) -> Resp {
        self.as_ref().handle_query(state, context, query).await
    }
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context; // Use the crate's Context if applicable
    use std::fmt::Display;
    use tokio::time::{sleep, Duration};

    // --- Test State Definition ---
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)] // Ensure State is comparable and serializable
    pub struct TestState {
        count: i32,
        name: String,
    }

    impl StateTrait for TestState {} // Basic implementation

    impl Display for TestState {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "State(count: {}, name: {})", self.count, self.name)
        }
    }

    // --- Test Event Definition ---
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)] // Default needed for ActorCommand if event is part of it indirectly
    pub enum TestEvent {
        Increment,
        Decrement,
        SetName(String),
        #[default]
        None, // Default variant if needed
    }

    impl EventTrait for TestEvent {
        // Basic implementations
        fn event_type(&self) -> &str {
            match self {
                TestEvent::Increment => "Increment",
                TestEvent::Decrement => "Decrement",
                TestEvent::SetName(_) => "SetName",
                TestEvent::None => "None",
            }
        }
        fn payload(&self) -> Option<&serde_json::Value> {
            None // No payload for these simple events
        }
        fn name(&self) -> &str {
            self.event_type() // Use type as name for simplicity
        }
    }

    // --- Test Query/Response Definition ---
    #[derive(Debug, Clone)]
    enum TestQuery {
        GetCount,
        GetName,
    }

    #[derive(Debug, Clone, PartialEq, Eq)] // PartialEq/Eq for assertions
    struct TestResponse(String); // Simple string response

    // --- Test Actor Logic ---
    #[derive(Default)] // Add Default derive
    struct TestActorLogic;

    #[async_trait]
    impl ActorLogic<TestState, Context, TestEvent, (), TestQuery, TestResponse> for TestActorLogic {
        fn initial(&self) -> (TestState, Context) {
            (
                TestState {
                    count: 0,
                    name: "Initial".to_string(),
                },
                Context::default(), // Assuming Context::default() exists
            )
        }

        async fn transition(
            &self,
            mut state: TestState,
            context: Context, // Context not modified here
            event: TestEvent,
        ) -> Result<(TestState, Context), StateError> {
            match event {
                TestEvent::Increment => state.count += 1,
                TestEvent::Decrement => state.count -= 1,
                TestEvent::SetName(name) => state.name = name,
                TestEvent::None => {} // No-op
            }
            Ok((state, context)) // Return potentially modified state and original context
        }

        async fn handle_query(
            &self,
            state: &TestState,
            _context: &Context, // Context not used here
            query: TestQuery,
        ) -> TestResponse {
            match query {
                TestQuery::GetCount => TestResponse(state.count.to_string()),
                TestQuery::GetName => TestResponse(state.name.clone()),
            }
        }
    }

    // --- Test Cases ---

    #[tokio::test]
    async fn test_actor_creation_and_initial_state() {
        let logic = TestActorLogic::default();
        let initial_context = Context::default();
        let (actor_ref, _join_handle) =
            create_actor(logic, initial_context, Some("test-create"), 32);

        sleep(Duration::from_millis(10)).await; // Allow time for actor to start

        let status = actor_ref.get_status().await;
        assert_eq!(status, ActorStatus::Active);

        let snapshot_res = actor_ref.get_snapshot().await;
        assert!(snapshot_res.is_ok());
        let snapshot = snapshot_res.unwrap();

        // Assert initial state via snapshot
        let expected_state = TestState {
            count: 0,
            name: "Initial".to_string(),
        };
        let expected_state_value = serde_json::to_value(&expected_state).unwrap();

        assert_eq!(snapshot.value, expected_state_value);
        // assert_eq!(snapshot.context, Context::default()); // Assuming Context impls PartialEq
        assert_eq!(snapshot.status, ActorStatus::Active);
        assert!(snapshot.output.is_none()); // Assuming TestOutput is Default = ()

        // Stop the actor
        let stop_res = actor_ref.stop().await;
        assert!(stop_res.is_ok());
        sleep(Duration::from_millis(10)).await; // Allow time for actor to stop
        let final_status = actor_ref.get_status().await;
        assert_eq!(final_status, ActorStatus::Stopped);

        // Optionally join the handle, though it might have already finished
        // let _ = _join_handle.await;
    }

    #[tokio::test]
    async fn test_actor_event_handling() {
        let logic = TestActorLogic::default();
        let initial_context = Context::default();
        let (actor_ref, _join_handle) =
            create_actor(logic, initial_context, Some("test-event"), 32);

        sleep(Duration::from_millis(10)).await; // Allow time for actor to start

        // Send Increment
        let send_res1 = actor_ref.send_event(TestEvent::Increment).await;
        assert!(send_res1.is_ok());
        sleep(Duration::from_millis(10)).await; // Allow processing time

        // Query count
        let query_res1 = actor_ref.query(TestQuery::GetCount).await;
        assert!(query_res1.is_ok());
        assert_eq!(query_res1.unwrap(), TestResponse("1".to_string()));

        // Send SetName
        let send_res2 = actor_ref
            .send_event(TestEvent::SetName("Updated".to_string()))
            .await;
        assert!(send_res2.is_ok());
        sleep(Duration::from_millis(10)).await;

        // Query name
        let query_res2 = actor_ref.query(TestQuery::GetName).await;
        assert!(query_res2.is_ok());
        assert_eq!(query_res2.unwrap(), TestResponse("Updated".to_string()));

        // Query count again (should still be 1)
        let query_res3 = actor_ref.query(TestQuery::GetCount).await;
        assert!(query_res3.is_ok());
        assert_eq!(query_res3.unwrap(), TestResponse("1".to_string()));

        // Stop the actor
        let stop_res = actor_ref.stop().await;
        assert!(stop_res.is_ok());
        sleep(Duration::from_millis(10)).await;
        assert_eq!(actor_ref.get_status().await, ActorStatus::Stopped);
        // let _ = _join_handle.await;
    }

    #[tokio::test]
    async fn test_actor_stop() {
        let logic = TestActorLogic::default();
        let initial_context = Context::default();
        let (actor_ref, join_handle) = create_actor(logic, initial_context, Some("test-stop"), 32);

        sleep(Duration::from_millis(10)).await; // Allow start

        assert_eq!(actor_ref.get_status().await, ActorStatus::Active);

        // Stop the actor
        let stop_res = actor_ref.stop().await;
        assert!(stop_res.is_ok());

        // Allow time for the stop command to be processed and the task to exit
        sleep(Duration::from_millis(20)).await;

        // Check status via ref
        let status_after_stop = actor_ref.get_status().await;
        // Status becomes Stopped *after* the loop finishes, check might race.
        // assert_eq!(status_after_stop, ActorStatus::Stopped);

        // Attempting to send after stop should fail (or indicate closed channel)
        let send_after_stop_res = actor_ref.send_event(TestEvent::Increment).await;
        assert!(send_after_stop_res.is_err());
        if let Err(StateError::SendError(msg)) = send_after_stop_res {
            println!("Send after stop failed as expected: {}", msg);
            assert!(
                msg.contains("channel closed") || msg.contains("actor") && msg.contains("stopped")
            );
        } else {
            panic!("Expected SendError after stop");
        }

        // Join the handle to ensure the task actually finished
        // This might take a moment if the actor was busy
        let join_result = tokio::time::timeout(Duration::from_millis(100), join_handle).await;
        assert!(join_result.is_ok(), "Actor task did not finish after stop");
        if let Ok(task_result) = join_result {
            assert!(task_result.is_ok(), "Actor task panicked");
        }

        // Final status check after join should reliably be Stopped
        assert_eq!(actor_ref.get_status().await, ActorStatus::Stopped);
    }

    // TODO: Add tests for:
    // - Error handling during transition
    // - Querying while actor is busy
    // - Dropping all ActorRefs stops the actor
    // - Buffer limits? (Harder to test reliably)
}
