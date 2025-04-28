//!
//! Defines actions (side effects) that can be executed during state transitions
//! or upon entering/exiting states within the RuState framework.

use crate::{context::Context, error::StateError, event::EventTrait};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Type alias for the underlying asynchronous function of an action.
///
/// Actions are represented as functions that receive a reference-counted, read-write locked
/// context (`Arc<RwLock<C>>`) and a reference to the triggering event (`&E`).
/// They return a pinned, boxed future (`Pin<Box<dyn Future<Output = ()> + Send>>`)
/// allowing them to perform asynchronous operations.
/// The function itself must be `Send + Sync` to be shared across threads.
pub type ActionFn<C, E> = Arc<
    dyn Fn(Arc<RwLock<C>>, &E) -> Pin<Box<dyn Future<Output = Result<(), StateError>> + Send>>
        + Send
        + Sync,
>;
// Changed Output to Result<(), StateError> to allow actions to fail

/// Represents an action to be executed within the state machine.
///
/// Actions typically represent side effects, such as performing I/O, logging,
/// modifying the context non-trivially, or sending events to other actors/machines.
///
/// Currently, actions primarily wrap asynchronous functions (`ActionFn`).
///
/// **Serialization Note:** Direct serialization of function pointers is not feasible.
/// The `serde(skip)` attribute is used here. For state persistence involving actions,
/// a mechanism to serialize action *identifiers* (e.g., strings) and look them up
/// during deserialization would be required.
#[derive(Clone, Serialize)]
pub struct Action<C = Context, E = crate::Event> {
    /// The specific type and logic of the action.
    #[serde(skip, default = "default_action_type")]
    pub action_type: ActionType<C, E>,
}

// Default function for serde(default). Provides a no-op async function.
// See serialization note on the Action struct.
fn default_action_type<C, E>() -> ActionType<C, E> {
    ActionType::Function(Arc::new(|_, _| Box::pin(async { Ok(()) })))
}

impl<C, E> Debug for Action<C, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Action")
            // Provide a simplified representation for Debug output
            .field("action_type", &self.action_type)
            .finish()
    }
}

impl<C, E> Action<C, E>
where
    C: Send + Sync + 'static + Default + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
{
    /// Creates a new `Action` from an asynchronous function.
    ///
    /// The provided function `f` must match the signature required by `ActionFn`.
    pub fn from_fn<F, Fut>(f: F) -> Self
    where
        F: Fn(Arc<RwLock<C>>, &E) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), StateError>> + Send + 'static, // Ensure Fut returns Result
    {
        let action_fn: ActionFn<C, E> = Arc::new(move |ctx_arc, evt| Box::pin(f(ctx_arc, evt)));
        Self {
            action_type: ActionType::Function(action_fn),
        }
    }

    /// Executes the action asynchronously.
    ///
    /// Takes the shared context and the triggering event as input.
    /// Returns `Ok(())` if the action completes successfully, or an `Err(StateError)` if it fails.
    pub async fn execute(&self, context_arc: Arc<RwLock<C>>, event: &E) -> Result<(), StateError> {
        match &self.action_type {
            ActionType::Function(f) => f(context_arc, event).await,
            // Potentially handle other ActionTypes here in the future (e.g., EmitEvent)
        }
    }
}

/// Defines the different kinds of actions that can exist.
///
/// Currently, only function-based actions are implemented.
/// Future versions might include actions like emitting specific events.
#[derive(Clone, Serialize)]
pub enum ActionType<C, E> {
    /// An action implemented as an asynchronous function.
    #[serde(skip)] // Skip the function field itself during serialization
    Function(ActionFn<C, E>),
    // Example: Emitting a specific event might be added later
    // EmitEvent(E),
}

// Custom Debug impl for ActionType to avoid trying to print the function pointer directly.
impl<C, E> Debug for ActionType<C, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionType::Function(_) => f.write_str("Function(<async_fn>)"),
            // Add other variants here if they are added
        }
    }
}

/// A trait allowing various types (like closures) to be converted into an [`Action`].
///
/// This simplifies the process of defining actions inline within the machine definition.
pub trait IntoAction<C, E> {
    /// Performs the conversion into an `Action`.
    fn into_action(self) -> Action<C, E>;
}

// Implement IntoAction for asynchronous function types that match the required signature.
impl<C, E, F, Fut> IntoAction<C, E> for F
where
    F: Fn(Arc<RwLock<C>>, &E) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), StateError>> + Send + 'static, // Ensure Fut returns Result
    C: Send + Sync + 'static + Default + Clone + Debug,
    E: EventTrait + Send + Sync + 'static,
{
    /// Converts a compatible asynchronous function into an `Action`.
    fn into_action(self) -> Action<C, E> {
        Action::from_fn(self)
    }
}
