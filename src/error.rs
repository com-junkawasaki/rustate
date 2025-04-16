use thiserror::Error;

/// Error types for the RState library
#[derive(Error, Debug)]
pub enum Error {
    #[error("State not found: {0}")]
    StateNotFound(String),
    
    #[error("Invalid transition: {0}")]
    InvalidTransition(String),
    
    #[error("Initial state not set")]
    InitialStateNotSet,
    
    #[error("Invalid state machine configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Internal error: {0}")]
    InternalError(String),
} 