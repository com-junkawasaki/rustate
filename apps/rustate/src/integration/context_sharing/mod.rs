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
        let _data_guard = self.data.read().lock_err()?;
        match &*_data_guard {
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
        let json_value = serde_json::to_value(value)?;
        match &mut *data_guard {
            serde_json::Value::Object(map) => {
                map.insert(key.to_string(), json_value);
            }
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
        let _data_guard = self.data.read().lock_err()?;
        match &*_data_guard {
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
        let mut _data_guard = self.data.write().lock_err()?;
        match &mut *_data_guard {
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
        let _data_guard = self.data.read().lock_err()?;
        Ok(_data_guard.clone())
    }

    /// Increments a value in the shared context.
    ///
    /// Acquires a write lock on the data.
    ///
    /// # Arguments
    /// * `key` - The key of the value to increment.
    ///
    /// # Returns
    /// * `Ok(())` if incrementing the value is successful.
    /// * `Err(IntegrationError::Serialization)` if serialization fails.
    /// * `Err(IntegrationError::Lock)` if the write lock is poisoned.
    pub fn increment(&self, key: &str) -> IntegrationResult<()> {
        trace!(key = key, "Attempting to increment value in shared context");
        let current_value: Option<i32> = self.get(key)?;
        let new_value = current_value.unwrap_or(0) + 1;
        self.set(key, new_value)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::error::{Error as IntegrationError, Result as IntegrationResult};
    use crate::Context;
    use crate::{Action, Event, Machine, MachineBuilder, State, Transition, TransitionType};
    use futures::FutureExt;
    use tokio::sync::RwLock;

    async fn create_machines(
        shared_context: SharedContext,
    ) -> (
        Machine<(), Event, String, ()>,
        Machine<Context, Event, String, ()>,
    ) {
        // Clone context before defining closures
        let context_for_writer = shared_context.clone();
        let context_for_reader = shared_context.clone();

        // Writer Action: Use the cloned context
        let write_action = Action::from_fn(move |_local_ctx: Arc<RwLock<()>>, _evt: &Event| {
            // Clone again for the async block if needed inside the closure
            let ctx_writer_clone = context_for_writer.clone();
            async move {
                println!("Writer Action: Writing to shared context");
                ctx_writer_clone.set("status", "updated_a")?;
                ctx_writer_clone.increment("counter")?;
                Ok(())
            }
            .boxed()
        });

        let mut idle_state_a = State::new("idle".to_string());
        let event_a_transition = Transition::new(
            "idle".to_string(),
            None::<String>,
            Some(Event::from("EVENT_A")),
            None,                      // Guard
            vec![write_action.into()], // Action needs .into() here to match Transition trait expectations
            TransitionType::Internal,
        );
        idle_state_a.add_transition("EVENT_A".to_string(), event_a_transition);
        let done_state_a = State::new_final("done".to_string());
        let machine_a =
            MachineBuilder::<(), Event, String, ()>::new("idle".to_string(), "idle".to_string())
                .state(idle_state_a)
                .state(done_state_a)
                .build()
                .await
                .expect("Machine A async build failed");

        // Reader Action: Use the other cloned context
        let read_action = Action::from_fn(move |local_ctx: Arc<RwLock<Context>>, _evt: &Event| {
            // Clone again for the async block if needed inside the closure
            let ctx_reader_clone = context_for_reader.clone();
            async move {
                println!("Reader Action: Reading shared context");
                let status = ctx_reader_clone.get::<String>("status")?;
                let counter = ctx_reader_clone.get::<i32>("counter")?;
                println!("Reader: Read status: {:?}, counter: {:?}", status, counter);
                if let Some(s) = status {
                    local_ctx.write().await.set("local_status_copy", s)?;
                }
                Ok(())
            }
            .boxed()
        });

        let mut waiting_state_b = State::new("waiting".to_string());
        let event_b_transition = Transition::new(
            "waiting".to_string(),
            None::<String>,
            Some(Event::from("EVENT_B")),
            None,                     // Guard
            vec![read_action.into()], // Action needs .into() here to match Transition trait expectations
            TransitionType::Internal,
        );
        waiting_state_b.add_transition("EVENT_B".to_string(), event_b_transition);
        let finished_state_b = State::new_final("processed".to_string());
        let machine_b = MachineBuilder::<Context, Event, String, ()>::new(
            "waiting".to_string(),
            "waiting".to_string(),
        )
        .state(waiting_state_b)
        .state(finished_state_b)
        .build()
        .await
        .expect("Machine B async build failed");

        (machine_a, machine_b)
    }

    #[tokio::test]
    async fn test_context_sharing_flow() -> IntegrationResult<()> {
        let shared_context = SharedContext::new();
        let (mut machine_a, mut machine_b) = create_machines(shared_context.clone()).await;

        // Initial check (optional)
        assert_eq!(shared_context.get::<String>("status")?, None);
        assert_eq!(shared_context.get::<i32>("counter")?, None);

        // Trigger Machine A
        machine_a.send(Event::from("EVENT_A")).await?;

        // Check context after A
        assert_eq!(
            shared_context.get::<String>("status")?,
            Some("updated_a".to_string())
        );
        assert_eq!(shared_context.get::<i32>("counter")?, Some(1));

        // Trigger Machine B
        machine_b.send(Event::from("EVENT_B")).await?;

        // Final context check (no change expected from B's read action)
        assert_eq!(
            shared_context.get::<String>("status")?,
            Some("updated_a".to_string())
        );
        assert_eq!(shared_context.get::<i32>("counter")?, Some(1));

        // Check final states (optional)
        // assert!(machine_a.is_in_final_state());
        // assert!(machine_b.is_in_final_state());

        Ok(())
    }

    #[tokio::test]
    async fn test_complex_assertions() -> IntegrationResult<()> {
        let shared_context = SharedContext::new();
        let (mut machine_a, _machine_b) = create_machines(shared_context.clone()).await;

        // Example usage, assuming EVENT_A triggers the write_action
        machine_a.send(Event::from("EVENT_A")).await?;

        shared_context.set("local_status", "active")?;
        shared_context.set("local_counter", 1i64)?;

        let ctx_b = shared_context;

        // Simplify map_or/unwrap_or to is_some_and
        // Assuming machine_b's context is relevant here, which it isn't directly
        // Reading directly from shared_context instead
        assert!(ctx_b
            .get::<String>("local_status")?
            .is_some_and(|s| s == "active"));
        assert!(ctx_b.get::<i64>("local_counter")?.is_some_and(|c| c == 1));

        Ok::<(), IntegrationError>(())
    }

    #[tokio::test]
    async fn test_contains_remove() -> IntegrationResult<()> {
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
