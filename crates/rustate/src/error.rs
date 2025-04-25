//!
//! Defines the error types used throughout the RuState library.

// Removed use std::fmt;
use thiserror::Error;
use std::io; // Added for IoError variant
// StateId is likely String or a similar type based on StateTrait
// Remove the direct import if StateId is not a distinct exported type
// use crate::state::StateId;

/// The primary error type for RuState operations.
///
/// This enum covers errors related to machine definition, state transitions,
/// actions, guards, context handling, serialization, and actor communication.
#[derive(Error, Debug, Clone, PartialEq)] // Removed Eq as String doesn't impl Eq
pub enum StateError {
    /// A requested state identifier was not found in the machine definition.
    /// Contains the identifier that was not found.
    #[error("State not found: {0}")]
    StateNotFound(String),

    /// An event was sent that has no defined transition from the current state.
    /// Contains the name or description of the unhandled event.
    #[error("Event not handled: {0}")]
    EventNotHandled(String),

    /// The machine configuration is invalid (e.g., missing initial state, inconsistent definitions).
    /// Contains a description of the configuration issue.
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// A transition definition is invalid (e.g., target state doesn't exist).
    /// Contains a description of the invalid transition.
    #[error("Invalid transition definition: {0}")] // Renamed from InvalidTransition
    InvalidTransitionDefinition(String),

    /// A guard condition for a transition evaluated to false, preventing the transition.
    #[error("Guard condition failed for event '{event}' in state '{state}'")]
    GuardFailed { state: String, event: String },

    /// An action associated with a transition or state entry/exit failed during execution.
    /// Contains a description of the action failure.
    #[error("Action execution failed: {0}")]
    ActionFailed(String),

    /// An error occurred during serialization (e.g., context to JSON).
    /// Contains the underlying serialization error message.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// An error occurred during deserialization (e.g., JSON to context).
    /// Contains the underlying deserialization error message.
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// An actor's command mailbox/channel was full when trying to send a message.
    /// Contains the ID or description of the actor.
    #[error("Actor mailbox full for actor: {0}")]
    ActorMailboxFull(String),

    /// An operation was attempted on an actor that has already stopped.
    /// Contains the ID or description of the stopped actor.
    #[error("Actor stopped: {0}")]
    ActorStopped(String),

    /// An unspecified internal error occurred within an actor's processing loop.
    /// Contains a description of the internal error.
    #[error("Actor internal error: {0}")]
    ActorInternalError(String),

    /// An attempt was made to create an actor with an ID that already exists.
    /// Contains the conflicting actor ID.
    #[error("Actor already exists: {0}")]
    ActorExists(String),

    /// An attempt was made to interact with an actor ID that does not exist.
    /// Contains the actor ID that was not found.
    #[error("Actor not found: {0}")]
    ActorNotFound(String),

    /// An error occurred while sending a message (e.g., event, command) over a channel.
    /// Contains details about the send failure.
    #[error("Send error: {0}")]
    SendError(String),

    /// An error occurred while receiving a message (e.g., query response) over a channel.
    /// Contains details about the receive failure.
    #[error("Receive error: {0}")]
    ReceiveError(String),

    /// An operation timed out.
    #[error("Timeout error")]
    TimeoutError,

    /// An error occurred during an I/O operation (e.g., reading/writing files for codegen).
    #[error("I/O error: {0}")]
    IoError(String),

    /// A transition could not be found from the given state for the specified event.
    #[error("Transition not found from state '{state}' for event '{event}'")]
    TransitionNotFound { state: String, event: String },

    /// The specified initial state identifier is invalid or not found.
    #[error("Invalid initial state: {0}")]
    InvalidInitialState(String),

    /// The machine definition is missing a specified initial state.
    #[error("Missing initial state for machine: {0}")]
    MissingInitialState(String),

    /// An error occurred while accessing or modifying the context.
    #[error("Context access error: {0}")]
    ContextError(String),

    /// A state definition within the machine configuration is invalid.
    #[error("Invalid state definition: {0}")]
    InvalidStateDefinition(String),

    /// The requested operation is not supported by the current configuration or state.
    #[error("Operation not supported: {0}")]
    UnsupportedOperation(String),

    /// An error related to concurrent access or modification.
    #[error("Concurrency error: {0}")]
    ConcurrencyError(String),

    /// An error originating from an external system or library.
    #[error("External error: {0}")]
    ExternalError(String),

    /// The type used for a state identifier is invalid or incompatible.
    #[error("Invalid state ID type")]
    InvalidStateIdType,

    /// A type mismatch occurred during an operation.
    #[error("Type mismatch: {0}")]
    TypeMismatch(String),

    /// A history state could not be found for the specified parent state.
    #[error("History state not found for state: {0}")]
    HistoryStateNotFound(String),

    /// Failed to spawn an actor task (e.g., due to Tokio runtime issues).
    #[error("Failed to spawn actor task: {0}")]
    SpawnError(String),

    /// A requested feature or capability is not yet implemented.
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    /// An error occurred during the execution of a query against the actor's state.
    #[error("Query error: {0}")]
    QueryError(String),

    /// An unspecified error occurred.
    #[error("Unknown error: {0}")]
    Other(String),
}

/// A specialized `Result` type for RuState operations.
/// Defaults to using [`StateError`] as the error type.
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
    /// Unknown agent error
    #[error("Unknown agent error: {0}")]
    Other(String),
}

pub type AgentResult<T> = std::result::Result<T, AgentError>;

// Optional: Implement From for common error types to simplify error handling
impl From<serde_json::Error> for StateError {
    fn from(err: serde_json::Error) -> Self {
        // Distinguish between serialization and deserialization if possible based on context,
        // otherwise, use a general variant.
        StateError::SerializationError(err.to_string())
    }
}

impl From<io::Error> for StateError {
    fn from(err: io::Error) -> Self {
        StateError::IoError(err.to_string())
    }
}
