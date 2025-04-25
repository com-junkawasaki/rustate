use crate::actor::Actor;
use crate::actor_ref::ActorRef;
use crate::spawn::{spawn_actor, DEFAULT_BUFFER_SIZE}; // Import existing spawn logic and constant

/// Represents a basic actor system, acting as an entry point for creating
/// and potentially managing top-level actors.
///
/// An `ActorSystem` provides a scope or context for actors. While this implementation
/// is minimal, a full-fledged actor system might handle configuration, supervision,
/// actor discovery, logging, and lifecycle management.
///
/// Cloning the `ActorSystem` allows sharing the system context (e.g., for spawning
/// actors from different parts of an application) without transferring ownership.
#[derive(Debug, Clone)] // Clone allows the system handle to be shared.
pub struct ActorSystem {
    /// The name of the actor system, primarily for identification and logging.
    name: String,
    // Future extensions could include:
    // - System-wide configuration (e.g., dispatchers, serializers).
    // - A registry of top-level actors managed by the system.
    // - Supervision strategies for top-level actors.
}

impl ActorSystem {
    /// Creates a new `ActorSystem` with the specified name.
    ///
    /// # Arguments
    ///
    /// * `name` - A string slice representing the name of the system.
    pub fn new(name: &str) -> Self {
        println!("Initializing ActorSystem '{}'...", name);
        ActorSystem {
            name: name.to_string(),
        }
    }

    /// Returns the name of the actor system.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Spawns a new top-level actor within this system.
    ///
    /// This method utilizes the underlying `spawn_actor` function to create
    /// the actor and run it in an independent asynchronous task.
    ///
    /// # Type Parameters
    ///
    /// * `A`: The type of the actor, implementing `Actor` and `A::State: PartialEq`.
    ///
    /// # Arguments
    ///
    /// * `actor` - An instance of the actor logic (`A`).
    /// * `buffer` - The size of the actor's event queue (mailbox).
    ///
    /// # Returns
    ///
    /// An `ActorRef<A>` handle to the newly spawned actor.
    pub fn spawn<A: Actor>(&self, actor: A, buffer: usize) -> ActorRef<A>
    where
        // This bound is required by the underlying spawn_actor function for logging.
        A::State: PartialEq,
    {
        println!(
            "ActorSystem '{}' spawning actor with buffer size {}...",
            self.name, buffer
        );
        // Delegate to the actual spawning logic.
        spawn_actor(actor, buffer)
    }

    /// Spawns a new top-level actor within this system using the default buffer size.
    ///
    /// This is a convenience method that calls `spawn` with `DEFAULT_BUFFER_SIZE`.
    ///
    /// # Type Parameters
    ///
    /// * `A`: The type of the actor, implementing `Actor` and `A::State: PartialEq`.
    ///
    /// # Arguments
    ///
    /// * `actor` - An instance of the actor logic (`A`).
    ///
    /// # Returns
    ///
    /// An `ActorRef<A>` handle to the newly spawned actor.
    pub fn spawn_default<A: Actor>(&self, actor: A) -> ActorRef<A>
    where
        // This bound is required by the underlying spawn_actor function for logging.
        A::State: PartialEq,
    {
        println!(
            "ActorSystem '{}' spawning actor with default buffer size...",
            self.name
        );
        self.spawn(actor, DEFAULT_BUFFER_SIZE)
    }

    // --- Potential Future Enhancements ---
    //
    // /// Initiates a graceful shutdown of the entire actor system.
    // pub async fn shutdown(&self) { /* ... */ }
    //
    // /// Attempts to find an actor by its ID or name within the system.
    // /// (Note: Actor discovery can be complex and might require a registry).
    // pub fn find_actor<A: Actor>(&self, id: &str) -> Option<ActorRef<A>> { /* ... */ }
    //
    // /// Configures system-level monitoring or metrics.
    // pub fn configure_monitoring(&mut self, config: MonitorConfig) { /* ... */ }
}

// Note on Send + Sync for ActorSystem:
// The current implementation only contains `name: String`, which is Send + Sync.
// Therefore, `ActorSystem` itself is implicitly Send + Sync.
// If internal state (like actor registries using `Arc<Mutex<...>>` or channels)
// is added later, care must be taken to ensure the entire system remains Send + Sync
// if required.
