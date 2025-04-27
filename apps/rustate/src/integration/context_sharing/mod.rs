//!
//! # Context Sharing Pattern
//!
//! Implements a pattern for sharing data between multiple state machines using
//! a shared context container.
//!
//! This pattern allows state machines, potentially spanning across crate boundaries,
//! to access and modify the same context data safely.
//!
//! ## Overview
//!
//! The Context Sharing pattern provides a flexible way for multiple state machines
//! to share common data. Key benefits include:
//!
//! - Simplified data synchronization between machines.
//! - Type-safe data sharing across crate boundaries.
//! - A data-centric approach to coordination, contrasting with event forwarding.
//!
//! ## Key Components
//!
//! - [`SharedContext`]: A thread-safe container (`Arc<RwLock<...>>`) holding shared data
//!   as a `serde_json::Value` (typically representing a JSON object).
//!
//! ## Usage Example
//!
//! ```rust
//! # #[cfg(feature = "integration")]
//! # {
//! use rustate::prelude::*;
//! use rustate::integration::SharedContext;
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//!
//! // 1. Create the shared context
//! let shared_context = SharedContext::new();
//!
//! // 2. Clone the context handle for each machine/action that needs access
//! let context_for_writer = shared_context.clone();
//! let context_for_reader = shared_context.clone();
//!
//! // 3. Define actions that interact with the shared context
//! let write_action = Action::from_fn(
//!     move |_ctx: Arc<RwLock<()>>, _evt: &String| { // Machine context type is (), Event is String
//!         let ctx_writer = context_for_writer.clone();
//!         async move {
//!             println!("Writer: Setting shared status to 'active'");
//!             ctx_writer.set("status", "active")?;
//!             ctx_writer.set("timestamp", 12345)?; // Can store different types
//!             Ok(())
//!         }
//!     }
//! );
//!
//! let read_action = Action::from_fn(
//!     move |local_ctx: Arc<RwLock<Context>>, _evt: &String| { // Machine context type is rustate::Context
//!         let ctx_reader = context_for_reader.clone();
//!         async move {
//!             println!("Reader: Reading shared status...");
//!             if let Some(status) = ctx_reader.get::<String>("status")? {
//!                 println!("Reader: Found status '{}', setting in local context.", status);
//!                 // Write to the reading machine's *local* context
//!                 local_ctx.write().await.set("local_status_copy", status)?;
//!             } else {
//!                 println!("Reader: Shared status not found.");
//!             }
//!             Ok(())
//!         }
//!     }
//! );
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // 4. Create machines using the actions
//!     let mut machine_writer = MachineBuilder::<String, String, ()>::new("idle".to_string())
//!         .state("idle".to_string(), |s| s.on("WRITE".to_string(), |t| t.target("done".to_string()).actions([write_action])))
//!         .state("done".to_string(), |_| {})
//!         .build()?;
//!     
//!     // Machine B uses the default `rustate::Context` as its local context
//!     let mut machine_reader = MachineBuilder::<String, String, Context>::new("waiting".to_string())
//!         .state("waiting".to_string(), |s| s.on("READ".to_string(), |t| t.target("finished".to_string()).actions([read_action])))
//!         .state("finished".to_string(), |_| {})
//!         .build()?;
//!
//!     // 5. Run the machines
//!     println!("Shared context before: {:?}", shared_context.dump().await?);
//!     machine_writer.send("WRITE".to_string()).await?;
//!     println!("Shared context after write: {:?}", shared_context.dump().await?);
//!     machine_reader.send("READ".to_string()).await?;
//!     println!("Reader local context after read: {:?}", machine_reader.context().await);
//!
//!     assert_eq!(shared_context.get::<String>("status").await?, Some("active".to_string()));
//!     assert_eq!(machine_reader.context().await.get::<String>("local_status_copy")?, Some(Ok("active".to_string())));
//!
//!     Ok(())
//! }
//! # }
//! ```
//! ## Implementation Details
//!
//! This pattern utilizes an `Arc<RwLock<serde_json::Value>>` to safely share JSON-structured data.
//! The `RwLock` ensures data consistency during concurrent access: multiple readers are allowed
//! simultaneously, but writers require exclusive access.
//!
//! `SharedContext` stores key-value pairs within a JSON object (`serde_json::Value::Object`).
//! This allows flexible storage of various data types while enabling type-safe access
//! through Serde serialization/deserialization (`get`/`set` methods).
//!
//! ## Limitations
//!
//! - **Performance**: Frequent access or large data volumes might incur overhead due to locking and JSON serialization/deserialization.
//! - **Write Contention**: High write frequency can block readers.
//! - **Data Structure**: Relies on a key-value structure within a JSON object.

use crate::integration::error::{
    Error as IntegrationError, LockResultExt, Result as IntegrationResult,
};
use crate::Result as RuStateResult;
use crate::{Context, Event, Machine, MachineBuilder, State, Transition, TransitionType};
use futures::FutureExt;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::{Arc, RwLock};
use tracing::{trace, warn};

/// A thread-safe, shareable context container.
///
/// This struct wraps context data (stored internally as a `serde_json::Value`, typically an Object)
/// within an `Arc<RwLock<...>>`, allowing multiple state machines or threads
/// to safely read and write to the same underlying data store.
#[derive(Clone, Default, Debug)]
pub struct SharedContext {
    /// The underlying shared data, protected by a Read-Write lock.
    data: Arc<RwLock<serde_json::Value>>,
}

impl SharedContext {
    /// Creates a new, empty `SharedContext` initialized with an empty JSON object (`{}`).
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(serde_json::json!({}))),
        }
    }

    /// Retrieves and deserializes a value from the shared context.
    ///
    /// Acquires a read lock on the data.
    ///
    /// # Arguments
    /// * `key` - The key of the value to retrieve.
    ///
    /// # Returns
    /// * `Ok(Some(T))` if the key exists and deserialization into type `T` is successful.
    /// * `Ok(None)` if the key does not exist or the underlying data is not a JSON object.
    /// * `Err(IntegrationError::Serialization)` if deserialization fails.
    /// * `Err(IntegrationError::Lock)` if the read lock is poisoned.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> IntegrationResult<Option<T>> {
        trace!(key = key, "Attempting to get value from shared context");
        let data_guard = self.data.read().lock_err()?;
        match &*data_guard {
            serde_json::Value::Object(map) => match map.get(key) {
                Some(value) => {
                    // Clone the value to attempt deserialization
                    match serde_json::from_value(value.clone()) {
                        Ok(deserialized) => Ok(Some(deserialized)),
                        Err(e) => {
                            warn!(key = key, error = %e, "Deserialization failed for shared context value");
                            Err(IntegrationError::from(e)) // Convert serde error
                        }
                    }
                }
                None => Ok(None), // Key not found in the map
            },
            _ => Ok(None), // Data is not a JSON object, cannot contain the key
        }
    }

    /// Serializes and sets a value in the shared context.
    ///
    /// Acquires a write lock on the data.
    /// If the underlying data is not currently a JSON object, it will be replaced
    /// with a new JSON object containing only the provided key-value pair.
    ///
    /// # Arguments
    /// * `key` - The key to associate with the value.
    /// * `value` - The value to set (must implement `serde::Serialize`).
    ///
    /// # Returns
    /// * `Ok(())` if setting the value is successful.
    /// * `Err(IntegrationError::Serialization)` if serialization fails.
    /// * `Err(IntegrationError::Lock)` if the write lock is poisoned.
    pub fn set<T: Serialize>(&self, key: &str, value: T) -> IntegrationResult<()> {
        trace!(key = key, "Attempting to set value in shared context");
        let mut data_guard = self.data.write().lock_err()?;
        let json_value = serde_json::to_value(value)?; // Handle serialization error

        match &mut *data_guard {
            serde_json::Value::Object(map) => {
                map.insert(key.to_string(), json_value);
            }
            // Handle cases where the RwLock contains Null, Bool, etc.
            // Replace it with an object containing the new key-value.
            _ => {
                warn!("Shared context was not an object, replacing with new object containing key: {}", key);
                *data_guard = serde_json::json!({ key: json_value });
            }
        }
        Ok(())
    }

    /// Checks if a key exists within the shared context (assuming it holds a JSON object).
    ///
    /// Acquires a read lock.
    ///
    /// # Returns
    /// * `Ok(true)` if the key exists.
    /// * `Ok(false)` if the key does not exist or the context is not a JSON object.
    /// * `Err(IntegrationError::Lock)` if the read lock is poisoned.
    pub fn contains_key(&self, key: &str) -> IntegrationResult<bool> {
        trace!(key = key, "Checking if key exists in shared context");
        let data_guard = self.data.read().lock_err()?;
        match &*data_guard {
            serde_json::Value::Object(map) => Ok(map.contains_key(key)),
            _ => Ok(false),
        }
    }

    /// Removes a key and its associated value from the shared context.
    ///
    /// Acquires a write lock.
    ///
    /// # Returns
    /// * `Ok(Some(serde_json::Value))` if the key existed and was removed.
    /// * `Ok(None)` if the key did not exist or the context was not a JSON object.
    /// * `Err(IntegrationError::Lock)` if the write lock is poisoned.
    pub fn remove(&self, key: &str) -> IntegrationResult<Option<serde_json::Value>> {
        trace!(key = key, "Attempting to remove key from shared context");
        let mut data_guard = self.data.write().lock_err()?;
        match &mut *data_guard {
            serde_json::Value::Object(map) => Ok(map.remove(key)),
            _ => Ok(None),
        }
    }

    /// Returns a clone of the underlying `serde_json::Value`.
    /// Useful for inspecting the entire shared state.
    /// Acquires a read lock.
    ///
    /// # Returns
    /// * `Ok(serde_json::Value)` containing the cloned data.
    /// * `Err(IntegrationError::Lock)` if the read lock is poisoned.
    pub async fn dump(&self) -> IntegrationResult<serde_json::Value> {
        let data_guard = self.data.read().lock_err()?;
        Ok(data_guard.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::StateError;
    use crate::serde_json;
    use crate::{Action, Context, Event, MachineBuilder, State, Transition, TransitionType};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // Helper function to create machines for testing
    fn create_machines(
        shared_context: SharedContext,
    ) -> (Machine<(), Event, String>, Machine<Context, Event, String>) {
        // Machine A uses unit type () as its context
        let write_action = Action::from_fn({
            let ctx_writer = shared_context.clone();
            move |_ctx: Arc<RwLock<()>>, _evt: &Event| {
                let ctx_writer_clone = ctx_writer.clone();
                async move {
                    println!("Writer Action: Setting shared context");
                    ctx_writer_clone.set("status", "active")?;
                    ctx_writer_clone.set("counter", 1)?; // Assuming i32 or similar
                    Ok(())
                }
            }
        });

        // Specify the Output type O as () for MachineBuilder
        let mut idle_state_a = State::new("idle".to_string());
        idle_state_a = idle_state_a.add_transition(
            "EVENT_A",
            Transition::new(
                "idle".to_string(),
                Some("done".to_string()),
                Some(Event::from("EVENT_A")),
                None,
                vec![write_action.clone()], // Clone action
                TransitionType::External,
            ),
        );
        let done_state_a = State::new("done".to_string());

        let machine_a =
            MachineBuilder::<(), Event, String, ()>::new("idle".to_string(), "idle".to_string())
                .state(idle_state_a) // Use owned state
                .state(done_state_a)
                .build()
                .now_or_never()
                .expect("Machine A build failed")
                .unwrap();

        // Machine B uses the default `rustate::Context`
        let read_action = Action::from_fn({
            let ctx_reader = shared_context.clone();
            move |ctx: Arc<RwLock<Context>>, _evt: &Event| {
                let ctx_reader_clone = ctx_reader.clone();
                async move {
                    println!("Reader Action: Reading shared context");
                    let status = ctx_reader_clone.get::<String>("status")?;
                    let counter = ctx_reader_clone.get::<i32>("counter")?;
                    println!(
                        "Reader Action: Read status={:?}, counter={:?}",
                        status, counter
                    );

                    let mut ctx_guard = ctx.write().await;
                    if let Some(s) = status {
                        ctx_guard.set("local_status", s)?;
                    }
                    if let Some(c) = counter {
                        ctx_guard.set("local_counter", c)?;
                    }
                    Ok(())
                }
                .boxed() // Box the future
            }
        });

        // Introduce let bindings for states to fix E0716
        let mut waiting_state_b = State::new("waiting".to_string());
        waiting_state_b = waiting_state_b.add_transition(
            "EVENT_B",
            Transition::new(
                "waiting".to_string(),
                Some("processed".to_string()),
                Some(Event::from("EVENT_B")),
                None,
                vec![read_action.clone()], // Clone action
                TransitionType::External,
            ),
        );
        let processed_state_b = State::new("processed".to_string());

        // Use the default context type Context for MachineBuilder
        let machine_b = MachineBuilder::<Context, Event, String, ()>::new(
            "waiting".to_string(),
            "waiting".to_string(),
        )
        .state(waiting_state_b) // Use owned state
        .state(processed_state_b)
        .build()
        .now_or_never()
        .expect("Machine B build failed")
        .unwrap();

        (machine_a, machine_b)
    }

    #[tokio::test]
    async fn test_context_sharing_flow() -> RuStateResult<()> {
        let shared_context = SharedContext::new();
        let (mut machine_a, mut machine_b) = create_machines(shared_context.clone());

        println!("Initial state A: {:?}", machine_a.current_states);
        println!("Initial state B: {:?}", machine_b.current_states);
        println!("Initial shared context: {:?}", shared_context.dump().await?);

        // Machine A writes to shared context
        println!("Sending EVENT_A to Machine A...");
        let changed_a = machine_a.send(Event::from("EVENT_A")).await?;
        assert!(changed_a);
        println!("State A after EVENT_A: {:?}", machine_a.current_states);
        assert!(machine_a.is_in(&"done".to_string()));
        println!(
            "Shared context after A write: {:?}",
            shared_context.dump().await?
        );
        assert_eq!(shared_context.get::<String>("status")?.unwrap(), "active");
        assert_eq!(shared_context.get::<i32>("counter")?.unwrap(), 1);

        // Machine B reads from shared context
        println!("Sending EVENT_B to Machine B...");
        let changed_b = machine_b.send(Event::from("EVENT_B")).await?;
        assert!(changed_b);
        println!("State B after EVENT_B: {:?}", machine_b.current_states);
        assert!(machine_b.is_in(&"processed".to_string()));

        // Check Machine B's local context
        let ctx_b = machine_b.context.read().await;
        println!("Machine B local context: {:?}", ctx_b);
        assert_eq!(
            ctx_b
                .get::<String>("local_status")
                .ok_or_else(|| StateError::Unknown("Missing local_status".into()))?? // Use StateError::Unknown
                .unwrap(),
            "active"
        );
        assert_eq!(
            ctx_b
                .get::<i32>("local_counter")
                .ok_or_else(|| StateError::Unknown("Missing local_counter".into()))?? // Use StateError::Unknown
                .unwrap(),
            1
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_contains_remove() -> RuStateResult<()> {
        let shared_context = SharedContext::new();
        shared_context.set("key1", "value1")?;
        shared_context.set("key2", 123)?;

        assert!(shared_context.contains_key("key1")?);
        assert!(shared_context.contains_key("key2")?);
        assert!(!shared_context.contains_key("key3")?);

        let removed = shared_context.remove("key1")?;
        assert_eq!(removed, Some(serde_json::json!("value1")));
        assert!(!shared_context.contains_key("key1")?);

        let removed_none = shared_context.remove("key3")?;
        assert!(removed_none.is_none());

        Ok(())
    }
}
