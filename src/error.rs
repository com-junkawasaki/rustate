use thiserror::Error;

/// Errors that can occur in the RState library
#[derive(Error, Debug)]
pub enum Error {
    /// State not found in the machine
    #[error("State not found: {0}")]
    StateNotFound(String),
    /// Initial state not set
    #[error("Initial state not set")]
    InitialStateNotSet,
    /// Event could not be handled
    #[error("No transition found for event: {0}")]
    NoTransitionFound(String),
    /// Invalid transition
    #[error("Invalid transition: {0}")]
    InvalidTransition(String),
    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Result type for operations that can fail
pub type Result<T> = std::result::Result<T, Error>; 