use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the extended state (context) for a state machine
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Context {
    /// The context data
    pub data: serde_json::Value,
}

impl Context {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            data: serde_json::json!({}),
        }
    }

    /// Create a new context with data
    pub fn with_data(data: impl Into<serde_json::Value>) -> Self {
        Self { data: data.into() }
    }

    /// Set a value in the context
    pub fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), serde_json::Error> {
        match &mut self.data {
            serde_json::Value::Object(map) => {
                map.insert(key.to_string(), serde_json::to_value(value)?);
                Ok(())
            }
            _ => {
                self.data = serde_json::json!({ key: value });
                Ok(())
            }
        }
    }

    /// Get a value from the context
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        match &self.data {
            serde_json::Value::Object(map) => map
                .get(key)
                .and_then(|val| serde_json::from_value(val.clone()).ok()),
            _ => None,
        }
    }

    /// Check if a key exists in the context
    pub fn contains_key(&self, key: &str) -> bool {
        match &self.data {
            serde_json::Value::Object(map) => map.contains_key(key),
            _ => false,
        }
    }

    /// Remove a key from the context
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        match &mut self.data {
            serde_json::Value::Object(map) => map.remove(key),
            _ => None,
        }
    }
}

impl fmt::Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.data)
    }
}
