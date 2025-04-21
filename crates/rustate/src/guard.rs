use crate::{Context, Event};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A guard condition for a transition
#[derive(Serialize, Deserialize)]
pub struct Guard {
    /// The name of this guard
    pub name: String,
    /// Function pointer to evaluate the guard
    #[serde(skip)]
    pub(crate) predicate: Option<Box<dyn Fn(&Context, &Event) -> bool + Send + Sync>>,
}

impl Clone for Guard {
    fn clone(&self) -> Self {
        // Note: We can't actually clone the predicate function,
        // so this creates a guard with the same name but no predicate
        Self {
            name: self.name.clone(),
            predicate: None,
        }
    }
}

impl fmt::Debug for Guard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Guard")
            .field("name", &self.name)
            .field(
                "predicate",
                &format_args!(
                    "{}",
                    if self.predicate.is_some() {
                        "Some(Fn)"
                    } else {
                        "None"
                    }
                ),
            )
            .finish()
    }
}

impl Guard {
    /// Create a new guard with a name and predicate function
    pub fn new<F>(name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&Context, &Event) -> bool + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            predicate: Some(Box::new(predicate)),
        }
    }

    /// Create a new guard with a name only (for serialization)
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            predicate: None,
        }
    }

    /// Evaluate the guard against a context and event
    pub fn evaluate(&self, context: &Context, event: &Event) -> bool {
        match &self.predicate {
            Some(predicate) => predicate(context, event),
            None => {
                // Default behavior for serialized guards with no predicate
                // In a real implementation, you might look up a predicate from a registry
                true
            }
        }
    }
}

impl fmt::Display for Guard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guard({})", self.name)
    }
}

/// Trait for types that can be converted into a guard
pub trait IntoGuard {
    /// Convert into a guard
    fn into_guard(self) -> Guard;
}

impl IntoGuard for Guard {
    fn into_guard(self) -> Guard {
        self
    }
}

impl<F> IntoGuard for (&str, F)
where
    F: Fn(&Context, &Event) -> bool + Send + Sync + 'static,
{
    fn into_guard(self) -> Guard {
        Guard::new(self.0, self.1)
    }
}
