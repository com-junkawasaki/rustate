use crate::actor::{Actor, ActorError};
use std::fmt::Debug;
use tokio::sync::mpsc;

/// Represents a reference to an actor, providing a way to interact with it,
/// primarily by sending events.
///
/// An `ActorRef` is not a direct pointer to an actor instance but rather a handle
/// managed by the actor system. This abstraction allows the system to manage
/// actor lifecycle, location transparency (local/remote), and communication.
///
/// Cloning an `ActorRef` creates another handle to the same actor instance,
/// increasing the reference count to its communication channel.
#[derive(Clone)]
pub struct ActorRef<A: Actor> {
    /// A unique identifier for the actor, useful for debugging and tracking.
    id: String, // Typically a UUID or a descriptive name.
    /// The sender part of the MPSC channel connected to the actor's mailbox.
    /// Used to send events to the actor.
    sender: mpsc::Sender<A::Event>,
}

impl<A: Actor> Debug for ActorRef<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorRef")
            .field("id", &self.id)
            // Avoid printing the sender details directly for brevity and security.
            .field("sender", &"<mpsc::Sender>")
            .finish()
    }
}

impl<A: Actor> ActorRef<A> {
    /// Asynchronously sends an event to the actor associated with this `ActorRef`.
    ///
    /// This method places the event into the actor's mailbox (queue).
    /// It is generally non-blocking, returning immediately after queueing the event.
    /// However, if the actor's mailbox is full (bounded channel), this method
    /// will wait (`await`) until there is space available.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to send. Must match the `Actor::Event` associated type.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the event was successfully sent (queued).
    /// * `Err(ActorError::Stopped)` - If the receiving actor has already stopped
    ///   (i.e., the `Receiver` end of the channel has been dropped).
    pub async fn send(&self, event: A::Event) -> Result<(), ActorError> {
        self.sender.send(event).await.map_err(|_send_error| {
            // The SendError contains the event, but we don't need it here.
            // The primary reason for a send error in this context is that the receiver
            // has been dropped, indicating the actor is stopped.
            ActorError::Stopped
        })
    }

    /// Retrieves the current state of the actor.
    ///
    /// Note: This functionality often requires a request-response pattern (ask pattern)
    /// involving temporary channels, which is not implemented here by default.
    /// It might be provided by higher-level abstractions or specific actor system implementations.
    // pub async fn ask_state(&self) -> Result<A::State, ActorError> { unimplemented!() }

    /// Attempts to signal the actor to stop processing.
    ///
    /// Note: Graceful shutdown typically involves sending a specific `Stop` event
    /// or using a dedicated signal mechanism managed by the actor system.
    /// A direct `stop` method on the `ActorRef` might not be the standard approach.
    // pub async fn stop(&self) -> Result<(), ActorError> { unimplemented!() }

    /// Internal constructor used by the actor system (or spawning mechanism).
    ///
    /// Creates a new `ActorRef` linked to an actor's mailbox sender.
    ///
    /// # Arguments
    ///
    /// * `id` - The unique identifier for the actor.
    /// * `sender` - The `mpsc::Sender` connected to the actor's mailbox.
    pub(crate) fn new(id: String, sender: mpsc::Sender<A::Event>) -> Self {
        Self { id, sender }
    }

    /// Returns the unique identifier of the actor associated with this reference.
    pub fn id(&self) -> &str {
        &self.id
    }
}

// Note on Send + Sync:
// `ActorRef` is automatically `Send` and `Sync` if `A::Event` is `Send`.
// This is because `mpsc::Sender<T>` is `Send` and `Sync` if `T` is `Send`.
// The `Actor` trait bounds already require `A::Event: Send + Sync`,
// so no `unsafe impl` is necessary.
// unsafe impl<A: Actor> Send for ActorRef<A> {}
// unsafe impl<A: Actor> Sync for ActorRef<A> {}
