use crate::actor::{Actor, ActorError};
use crate::actor_ref::ActorRef;
use tokio::sync::mpsc;
use uuid::Uuid;

/// The default buffer size for an actor's mailbox (event queue).
///
/// This value determines how many events can be queued before `send` operations
/// start blocking (or yielding in async context).
pub const DEFAULT_BUFFER_SIZE: usize = 32;

/// Spawns an actor, starting its execution in an independent asynchronous task.
///
/// This function creates a communication channel (mailbox) for the actor,
/// generates a unique ID, creates an `ActorRef` handle, and launches a Tokio task
/// that runs the actor's event loop.
///
/// The event loop continuously receives events from the mailbox, calls the actor's
/// `receive` method to process them, and updates the actor's state.
/// The loop terminates when the channel is closed (i.e., all `ActorRef` clones
/// associated with the actor are dropped) or if a critical error occurs.
///
/// # Type Parameters
///
/// * `A`: The type of the actor, which must implement the `Actor` trait.
///   It also requires `A::State: PartialEq` to log state changes effectively.
///
/// # Arguments
///
/// * `actor` - An instance of the actor logic (`A`).
/// * `buffer` - The size of the actor's event queue (mailbox). Must be > 0.
///
/// # Returns
///
/// An `ActorRef<A>` which acts as a handle to the spawned actor, allowing
/// events to be sent to it.
pub fn spawn_actor<A: Actor>(actor: A, buffer: usize) -> ActorRef<A>
where
    // PartialEq bound is added for logging state changes, might be optional
    // depending on logging requirements.
    A::State: PartialEq,
{
    // Generate a unique ID for the actor instance.
    let id = Uuid::new_v4().to_string();
    // Create an MPSC channel for the actor's mailbox.
    let (sender, mut receiver) = mpsc::channel::<A::Event>(buffer);

    // Create the public handle (ActorRef) for interacting with the actor.
    let actor_ref = ActorRef::new(id.clone(), sender);

    // Spawn the actor's main processing loop as a separate Tokio task.
    tokio::spawn(async move {
        // Initialize the actor's state.
        let mut current_state = actor.initial_state();
        println!(
            "Actor {} spawned with initial state: {:?}",
            id, current_state
        );

        // The actor's event loop.
        while let Some(event) = receiver.recv().await {
            // Process the received event using the actor's logic.
            match actor.receive(current_state.clone(), event).await {
                Ok(new_state) => {
                    // Log state changes (optional, requires PartialEq).
                    if new_state != current_state {
                        println!(
                            "Actor {} state changed: {:?} -> {:?}",
                            id, current_state, new_state
                        );
                        current_state = new_state; // Update the state
                    } else {
                        // Log if state remained unchanged (optional).
                        // println!("Actor {} state unchanged: {:?}", id, current_state);
                    }
                }
                Err(err) => {
                    // Log errors encountered during event processing.
                    eprintln!("Actor {} error processing event: {}", id, err);
                    // Optionally, handle specific errors like stopping the actor.
                    if matches!(err, ActorError::Stopped) {
                        eprintln!("Actor {} stopping due to explicit stop request or critical error.", id);
                        break; // Exit the loop
                    }
                    // Decide whether to continue or stop on other errors.
                }
            }
        }
        // Log when the actor task finishes (usually when the channel is closed).
        println!("Actor {} task finished.", id);
    });

    // Return the handle to the spawned actor.
    actor_ref
}

/// Spawns an actor using the `DEFAULT_BUFFER_SIZE` for its mailbox.
///
/// This is a convenience function that calls `spawn_actor` with the default buffer size.
///
/// # Type Parameters
///
/// * `A`: The type of the actor, which must implement `Actor` and `A::State: PartialEq`.
///
/// # Arguments
///
/// * `actor` - An instance of the actor logic (`A`).
///
/// # Returns
///
/// An `ActorRef<A>` handle to the spawned actor.
pub fn spawn<A: Actor>(actor: A) -> ActorRef<A>
where
    A::State: PartialEq,
{
    spawn_actor(actor, DEFAULT_BUFFER_SIZE)
}
