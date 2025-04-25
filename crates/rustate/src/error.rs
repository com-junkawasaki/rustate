use std::fmt;
use thiserror::Error;
// StateId is likely String or a similar type based on StateTrait
// Remove the direct import if StateId is not a distinct exported type
// use crate::state::StateId;

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
    /// Actor channel send error
    #[error("Actor channel send error: {0}")]
    ActorSendError(String),
    /// Actor channel receive error
    #[error("Actor channel receive error: {0}")]
    ActorReceiveError(String),
    /// Transition not found from state '{state}' for event '{event}'
    #[error("Transition not found from state '{state}' for event '{event}'")]
    TransitionNotFound { state: String, event: String },
    /// Invalid initial state
    #[error("Invalid initial state: {0}")]
    InvalidInitialState(String),
    /// Missing initial state for machine
    #[error("Missing initial state for machine: {0}")]
    MissingInitialState(String),
    /// Context access error
    #[error("Context access error: {0}")]
    ContextError(String),
    /// Action execution error
    #[error("Action execution error: {0}")]
    ActionError(String),
    /// Invalid state definition
    #[error("Invalid state definition: {0}")]
    InvalidStateDefinition(String),
    /// Invalid transition definition
    #[error("Invalid transition definition: {0}")]
    InvalidTransitionDefinition(String),
    /// Operation not supported
    #[error("Operation not supported: {0}")]
    UnsupportedOperation(String),
    /// Concurrency error
    #[error("Concurrency error: {0}")]
    ConcurrencyError(String),
    /// External error
    #[error("External error: {0}")]
    ExternalError(String),
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Invalid state ID type
    #[error("Invalid state ID type")]
    InvalidStateIdType,
    /// Type mismatch
    #[error("Type mismatch: {0}")]
    TypeMismatch(String),
    /// History state not found for state
    #[error("History state not found for state: {0}")]
    HistoryStateNotFound(String),
    /// Failed to spawn actor task
    #[error("Failed to spawn actor task: {0}")]
    SpawnError(String),
    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),
    /// Feature not implemented
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
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

// Optional: Implement From for common error types to simplify error handling
impl From<serde_json::Error> for StateError {
    fn from(err: serde_json::Error) -> Self {
        StateError::SerializationError(err.to_string())
    }
}
