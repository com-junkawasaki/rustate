use crate::{Context, Event};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of action execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    /// Action executed when entering a state
    Entry,
    /// Action executed when exiting a state
    Exit,
    /// Action executed during a transition
    Transition,
}

/// An action that can be executed during state transitions
#[derive(Serialize, Deserialize)]
pub struct Action {
    /// The name of this action
    pub name: String,
    /// The type of action execution
    pub action_type: ActionType,
    /// Function pointer to execute the action
    #[serde(skip)]
    pub(crate) executor: Option<Box<dyn Fn(&mut Context, &Event) + Send + Sync>>,
}

impl Clone for Action {
    fn clone(&self) -> Self {
        // Note: We can't actually clone the executor function,
        // so this creates an action with the same name and type but no executor
        Self {
            name: self.name.clone(),
            action_type: self.action_type,
            executor: None,
        }
    }
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Action")
            .field("name", &self.name)
            .field("action_type", &self.action_type)
            .field("executor", &format_args!("{}", if self.executor.is_some() { "Some(Fn)" } else { "None" }))
            .finish()
    }
}

impl Action {
    /// Create a new action with a name and executor function
    pub fn new<F>(
        name: impl Into<String>,
        action_type: ActionType,
        executor: F,
    ) -> Self 
    where
        F: Fn(&mut Context, &Event) + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            action_type,
            executor: Some(Box::new(executor)),
        }
    }

    /// Create a new entry action
    pub fn entry<F>(name: impl Into<String>, executor: F) -> Self 
    where
        F: Fn(&mut Context, &Event) + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Entry, executor)
    }

    /// Create a new exit action
    pub fn exit<F>(name: impl Into<String>, executor: F) -> Self 
    where
        F: Fn(&mut Context, &Event) + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Exit, executor)
    }

    /// Create a new transition action
    pub fn transition<F>(name: impl Into<String>, executor: F) -> Self 
    where
        F: Fn(&mut Context, &Event) + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Transition, executor)
    }

    /// Create a new action with a name only (for serialization)
    pub fn named(name: impl Into<String>, action_type: ActionType) -> Self {
        Self {
            name: name.into(),
            action_type,
            executor: None,
        }
    }

    /// Execute the action with a context and event
    pub fn execute(&self, context: &mut Context, event: &Event) {
        if let Some(executor) = &self.executor {
            executor(context, event);
        } else {
            // Default behavior for serialized actions with no executor
            // In a real implementation, you might look up an executor from a registry
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Action({}, {:?})", self.name, self.action_type)
    }
}

/// Trait for types that can be converted into an action
pub trait IntoAction {
    /// Convert into an action
    fn into_action(self, action_type: ActionType) -> Action;
}

impl IntoAction for Action {
    fn into_action(self, _action_type: ActionType) -> Action {
        self
    }
}

impl<F> IntoAction for (&str, F)
where
    F: Fn(&mut Context, &Event) + Send + Sync + 'static,
{
    fn into_action(self, action_type: ActionType) -> Action {
        Action::new(self.0, action_type, self.1)
    }
} 