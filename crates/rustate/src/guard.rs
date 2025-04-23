use crate::{Context, Event};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// Type alias for the guard predicate function
pub type GuardPredicate =
    Box<dyn Fn(&Context, &Event) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// A guard condition for a transition
#[derive(Serialize, Deserialize)]
pub struct Guard {
    /// The name of this guard
    pub name: String,
    /// Function pointer to evaluate the guard
    #[serde(skip)]
    pub(crate) predicate: Option<GuardPredicate>,
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
    pub fn new<F, Fut>(name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&Context, &Event) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        Self {
            name: name.into(),
            predicate: Some(Box::new(move |ctx, evt| Box::pin(predicate(ctx, evt)))),
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
    pub async fn evaluate(&self, context: &Context, event: &Event) -> bool {
        match &self.predicate {
            Some(predicate) => predicate(context, event).await,
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

impl<F, Fut> IntoGuard for (&str, F)
where
    F: Fn(&Context, &Event) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = bool> + Send + 'static,
{
    fn into_guard(self) -> Guard {
        Guard::new(self.0, self.1)
    }
}
