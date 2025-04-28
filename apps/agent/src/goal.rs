// crates/agent/src/goal.rs

use rustate::StateTrait; // Assuming Goal needs StateTrait
use serde::{de::DeserializeOwned, Deserialize, Serialize}; // Add DeserializeOwned
use std::fmt::Debug;

// Placeholder Goal struct definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal<S>
where
    S: StateTrait + Clone + Debug + Send + Sync + 'static + DeserializeOwned,
{
    pub target_state: S,
    // Add other fields as needed (e.g., conditions, priority)
}

impl<S> Goal<S>
where
    S: StateTrait + Clone + Debug + Send + Sync + 'static + DeserializeOwned,
{
    pub fn new(target_state: S) -> Self {
        Self { target_state }
    }
}
