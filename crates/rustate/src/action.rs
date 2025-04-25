use crate::{Context, EventTrait, Result};
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Represents an action to be executed
pub type ActionFn<C, E> =
    Box<dyn Fn(&mut C, &E) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Represents an action to be executed
pub struct Action<C = Context, E = crate::Event> {
    // Use crate::Event
    /// Type of the action (e.g., function call, event emission)
    pub action_type: ActionType<C, E>,
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
        let action_fn: ActionFn<C, E> = Box::new(move |ctx, evt| Box::pin(f(ctx, evt)));
        Self {
            action_type: ActionType::Function(action_fn),
        }
    }

    /// Create a new action that sends an event
    pub fn send(event: E) -> Self {
        Self {
            action_type: ActionType::SendEvent(event),
        }
    }

    /// Execute the action
    pub async fn execute(&self, context: &mut C, event: &E) {
        match &self.action_type {
            ActionType::Function(f) => f(context, event).await,
            ActionType::SendEvent(_event_to_send) => {
                // Logic to actually send the event needs context/actor reference
                eprintln!("SendEvent action needs implementation to dispatch event");
            }
        }
    }
}

/// Different types of actions
pub enum ActionType<C, E> {
    Function(ActionFn<C, E>),
    SendEvent(E), // Action to send an event back to the machine/actor
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

// Implement IntoAction for sending an event
impl<C, E> IntoAction<C, E> for E
where
    E: EventTrait + Send + Sync + 'static + Clone,
    C: Send + Sync + 'static,
{
    fn into_action(self) -> Action<C, E> {
        Action::send(self)
    }
}
