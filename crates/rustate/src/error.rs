use thiserror::Error;

/// Errors that can occur in the RuState library
#[derive(Error, Debug, Clone, PartialEq)]
pub enum StateError {
    /// State not found in the machine
    #[error("State not found: {0}")]
    StateNotFound(String),
    /// Event not handled
    #[error("Event not handled: {0}")]
    EventNotHandled(String),
    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    /// Invalid transition
    #[error("Invalid transition: {0}")]
    InvalidTransition(String),
    /// Guard condition failed for event
    #[error("Guard condition failed for event {event} in state {state}")]
    GuardFailed { state: String, event: String },
    /// Action execution failed
    #[error("Action execution failed: {0}")]
    ActionFailed(String),
    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    /// Actor mailbox full for actor
    #[error("Actor mailbox full for actor: {0}")]
    ActorMailboxFull(String),
    /// Actor stopped
    #[error("Actor stopped: {0}")]
    ActorStopped(String),
    /// Actor internal error
    #[error("Actor internal error: {0}")]
    ActorInternalError(String),
    /// Actor already exists
    #[error("Actor already exists: {0}")]
    ActorExists(String),
    /// Actor not found
    #[error("Actor not found: {0}")]
    ActorNotFound(String),
    /// Send error
    #[error("Send error: {0}")]
    SendError(String),
    /// Receive error
    #[error("Receive error: {0}")]
    ReceiveError(String),
    /// Timeout error
    #[error("Timeout error")]
    TimeoutError,
    /// Unknown error
    #[error("Unknown error: {0}")]
    Other(String),
}

/// Result type for operations that can fail
pub type Result<T, E = StateError> = std::result::Result<T, E>;

// Agent Error (ensure missing variants are added here too)
#[derive(Error, Debug, Clone, PartialEq)]
pub enum AgentError {
    /// Policy error
    #[error("Policy error: {0}")]
    PolicyError(String),
    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),
    /// Agent configuration error
    #[error("Agent configuration error: {0}")]
    ConfigurationError(String),
    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),
    /// Integration error
    #[error("Integration error: {0}")]
    IntegrationError(String),
    /// State machine error
    #[error("State machine error: {0}")]
    MachineError(String),
    /// Operation not supported
    #[error("Operation not supported: {0}")]
    NotSupported(String),
    /// Internal agent error
    #[error("Internal agent error: {0}")]
    InternalError(String),
    /// State machine error
    #[error("State machine error: {0}")]
    StateMachineError(String),
    /// Unknown agent error
    #[error("Unknown agent error: {0}")]
    Other(String),
}

pub type AgentResult<T> = std::result::Result<T, AgentError>;
