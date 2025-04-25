use crate::{Context, EventTrait, Result};
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Represents an action to be executed
pub type ActionFn<C, E> =
    Arc<dyn Fn(&mut C, &E) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Represents an action to be executed
#[derive(Clone, Serialize, Deserialize)]
pub struct Action<C = Context, E = crate::Event> {
    // Use crate::Event
    /// Type of the action (e.g., function call, event emission)
    #[serde(skip)]
    #[serde(default = "default_action_type")]
    pub action_type: ActionType<C, E>,
}

// Default function needed for serde skip/default
fn default_action_type<C, E>() -> ActionType<C, E> {
    // Provide a default, perhaps indicating it's non-serializable
    // This is tricky because ActionFn needs a concrete function.
    // For now, let's panic or return a placeholder if needed, though ideally, this default
    // is only used if deserialization encounters a missing field.
    // A better approach might involve custom Serialize/Deserialize impls.
    ActionType::Function(Arc::new(|_, _| Box::pin(async {})))
}

impl<C, E> Debug for Action<C, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Action")
            .field("action_type", &"Fn(...) | Event(...)") // Simplified Debug
            .finish()
    }
}

impl<C, E> Action<C, E>
where
    C: Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Create a new action from a function
    pub fn from_fn<F, Fut>(f: F) -> Self
    where
        F: Fn(&mut C, &E) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let action_fn: ActionFn<C, E> = Arc::new(move |ctx, evt| Box::pin(f(ctx, evt)));
        Self {
            action_type: ActionType::Function(action_fn),
        }
    }

    /// Execute the action
    pub async fn execute(&self, context: &mut C, event: &E) {
        match &self.action_type {
            ActionType::Function(f) => f(context, event).await,
        }
    }
}

/// Different types of actions
#[derive(Clone, Serialize, Deserialize)]
pub enum ActionType<C, E> {
    #[serde(skip)]
    Function(ActionFn<C, E>),
}

/// Trait to convert various types into an Action
pub trait IntoAction<C, E> {
    fn into_action(self) -> Action<C, E>;
}

// Implement IntoAction for functions
impl<C, E, F, Fut> IntoAction<C, E> for F
where
    F: Fn(&mut C, &E) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
    C: Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_action(self) -> Action<C, E> {
        Action::from_fn(self)
    }
}
