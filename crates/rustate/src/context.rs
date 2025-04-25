use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

/// Represents the extended state (context) for a state machine
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Context {
    #[serde(flatten)]
    data: HashMap<String, Value>,
}

impl Context {
    /// Create a new empty context
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    /// Creates a new context from a serde_json Value.
    /// Assumes the input Value is an Object.
    pub fn from_value(value: Value) -> Self {
        match value {
            Value::Object(map) => Self { 
                data: map.into_iter().collect() // Convert serde_json::Map to HashMap
            },
            _ => Self::new(), // Return empty context if not an object
        }
    }

    /// Set a value in the context
    pub fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), serde_json::Error> {
        let value = serde_json::to_value(value)?;
        self.data.insert(key.to_string(), value);
        Ok(())
    }

    /// Get a value from the context
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<Result<T, serde_json::Error>> {
        self.data.get(key).map(|value| serde_json::from_value(value.clone()))
    }
    
    /// Get a value from the context
    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Check if a key exists in the context
    pub fn has(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Remove a key from the context
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }
    
    /// Check if the context is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl fmt::Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use Debug formatting for HashMap
        write!(f, "{:?}", self.data)
    }
}
