//!
//! Defines the `Context` struct used to hold arbitrary data (extended state)
//! associated with a RuState state machine.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

/// Represents the extended state (context) for a state machine.
///
/// The `Context` stores arbitrary data associated with the machine's current state.
/// It uses a `HashMap<String, serde_json::Value>` internally, allowing for flexible,
/// dynamic data structures. Values can be any type that implements `Serialize` and
/// `Deserialize`.
///
/// While flexible, using specific, strongly-typed structs for context is often recommended
/// for better compile-time safety and clarity, especially for complex machines.
/// This generic `Context` is useful for simpler cases or when the structure is highly dynamic.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Context {
    /// Internal storage using a HashMap where keys are strings and values
    /// are JSON values, allowing arbitrary data structures.
    #[serde(flatten)] // Flattens the map into the parent JSON object during serialization
    data: HashMap<String, Value>,
}

impl Context {
    /// Creates a new, empty `Context`.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Creates a new `Context` from a `serde_json::Value`.
    ///
    /// Assumes the input `value` is a JSON Object (`Value::Object`).
    /// If the input is not an object, an empty `Context` is returned.
    ///
    /// # Arguments
    /// * `value` - A `serde_json::Value` assumed to be an Object.
    pub fn from_value(value: Value) -> Self {
        match value {
            Value::Object(map) => Self {
                // Convert serde_json::Map to HashMap<String, Value>
                data: map.into_iter().collect(),
            },
            _ => {
                eprintln!("Warning: Context::from_value expected a JSON object, received {:?}. Returning empty context.", value);
                Self::new() // Return empty context if not an object
            }
        }
    }

    /// Sets a value in the context, serializing it to a `serde_json::Value`.
    ///
    /// # Arguments
    /// * `key` - The string key to associate with the value.
    /// * `value` - The value to set. Must implement `serde::Serialize`.
    ///
    /// # Returns
    /// `Ok(())` on success, or a `serde_json::Error` if serialization fails.
    pub fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), serde_json::Error> {
        let json_value = serde_json::to_value(value)?;
        self.data.insert(key.to_string(), json_value);
        Ok(())
    }

    /// Gets a value from the context, attempting to deserialize it into type `T`.
    ///
    /// # Arguments
    /// * `key` - The string key of the value to retrieve.
    ///
    /// # Returns
    /// * `Some(Ok(T))` if the key exists and deserialization succeeds.
    /// * `Some(Err(serde_json::Error))` if the key exists but deserialization fails.
    /// * `None` if the key does not exist.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<Result<T, serde_json::Error>> {
        self.data
            .get(key)
            .map(|value| serde_json::from_value(value.clone())) // Clone value for deserialization
    }

    /// Gets a reference to the raw `serde_json::Value` associated with a key.
    ///
    /// Returns `None` if the key does not exist.
    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Checks if a key exists in the context.
    pub fn has(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Removes a key and its associated value from the context.
    ///
    /// Returns the `serde_json::Value` if the key existed, otherwise `None`.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }

    /// Checks if the context contains no data.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl fmt::Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Attempt to pretty-print the JSON representation for better readability.
        match serde_json::to_string_pretty(&self.data) {
            Ok(pretty_json) => write!(f, "{}", pretty_json),
            Err(_) => {
                // Fallback to Debug formatting if pretty printing fails (should be rare)
                write!(f, "{:?}", self.data)
            }
        }
    }
}
