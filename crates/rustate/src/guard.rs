use crate::{Action, Context, Error, Event, EventTrait, IntoAction, Result, State, StateTrait};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

/// Type alias for the guard predicate function
pub type GuardPredicate =
    Box<dyn Fn(&Context, &Event) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// Represents a guard condition for a transition
#[derive(Clone, Serialize)]
pub struct Guard<C = Context, E = Event>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// The name of this guard
    pub name: String,
    /// Function pointer to evaluate the guard
    #[serde(skip)]
    pub condition: Arc<dyn Fn(&C, &E) -> bool + Send + Sync>,
}

impl<C, E> Guard<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Create a new guard with a name and predicate function
    pub fn new<F>(name: impl Into<String>, condition: F) -> Self
    where
        F: Fn(&C, &E) -> bool + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            condition: Arc::new(condition),
        }
    }

    /// Create a new guard with a name only (for serialization)
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            condition: Arc::new(|_ctx, _evt| false),
        }
    }

    /// Evaluate the guard against a context and event
    pub fn check(&self, context: &C, event: &E) -> bool {
        (self.condition)(context, event)
    }
}

impl<C, E> fmt::Display for Guard<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guard({})", self.name)
    }
}

/// Trait for converting into a guard
pub trait IntoGuard<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Convert into a guard
    fn into_guard(self) -> Guard<C, E>;
}

impl<C, E> IntoGuard<C, E> for Guard<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_guard(self) -> Guard<C, E> {
        self
    }
}

// Commenting out problematic implementation for now
// impl<C, E> IntoGuard<C, E> for (&str, Arc<dyn Fn(&C, &E) -> bool + Send + Sync + 'static>)
// where
//     C: Clone + Send + Sync + 'static,
//     E: EventTrait + Send + Sync + 'static,
// {
//     fn into_guard(self) -> Guard<C, E> {
//         Guard {
//             name: self.0.to_string(),
//             condition: self.1,
//         }
//     }
// }

impl<C, E, F> IntoGuard<C, E> for (&str, F)
where
    F: Fn(&C, &E) -> bool + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_guard(self) -> Guard<C, E> {
        Guard::new(self.0, self.1)
    }
}

// Manually implement Debug
impl<C, E> fmt::Debug for Guard<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Guard")
            .field("name", &self.name)
            .field("condition", &"<Fn>") // Don't print the function itself
            .finish()
    }
}

// Manually implement PartialEq
impl<C, E> PartialEq for Guard<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
        // Cannot compare closures (condition)
    }
}

// If PartialEq is manually implemented, Eq can often be derived or implemented simply.
impl<C, E> Eq for Guard<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
}
