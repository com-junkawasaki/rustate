//!
//! Defines guard conditions used to determine if a state transition should occur.
//!
//! Guards are functions that evaluate based on the current context and triggering event,
//! returning `true` if the associated transition is allowed, and `false` otherwise.

use crate::{
    context::Context,
    error::StateError,
    event::{Event, EventTrait},
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Type alias for an *asynchronous* guard predicate function.
///
/// **Note:** This type alias represents an *asynchronous* guard function signature
/// (returning a `Future<Output = Result<bool, StateError>>`), but the current `Guard` struct
/// implementation below uses a *synchronous* function signature (`Fn(&C, &E) -> bool`).
/// This suggests a potential future refactoring towards async guards, or a mismatch.
/// For now, `Guard` uses the synchronous version.
pub type AsyncGuardPredicate<C, E> = Arc<
    dyn Fn(Arc<RwLock<C>>, &E) -> Pin<Box<dyn Future<Output = Result<bool, StateError>> + Send>>
        + Send
        + Sync,
>;

/// Represents a guard condition that can be attached to a transition.
///
/// A guard is a synchronous predicate function (`condition`) associated with a `name`.
/// The `condition` function takes the current context (`&C`) and the triggering event (`&E`)
/// and returns `true` if the transition should be allowed, `false` otherwise.
///
/// **Serialization Note:** The `condition` function pointer itself is not serialized
/// (due to `serde(skip)`). When deserializing a machine definition, guards must
/// typically be re-associated based on their `name`.
#[derive(Clone, Serialize)]
pub struct Guard<C = Context, E = Event>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    /// A descriptive name for the guard, used for identification and potentially
    /// re-association during deserialization.
    pub name: String,
    /// The synchronous predicate function that evaluates the guard condition.
    /// Takes context and event, returns `true` if the guard passes.
    #[serde(skip)]
    pub condition: Arc<dyn Fn(&C, &E) -> bool + Send + Sync>,
}

impl<C, E> Guard<C, E>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    /// Creates a new `Guard` with a name and a synchronous predicate function.
    ///
    /// # Arguments
    /// * `name` - A name for the guard (converted into `String`).
    /// * `condition` - A closure or function pointer `Fn(&C, &E) -> bool + Send + Sync + 'static`.
    pub fn new<F>(name: impl Into<String>, condition: F) -> Self
    where
        F: Fn(&C, &E) -> bool + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            condition: Arc::new(condition),
        }
    }

    /// Creates a new `Guard` with only a name.
    ///
    /// This is primarily useful for scenarios where guards are identified by name,
    /// such as during deserialization or when defining transitions separate from
    /// the guard logic implementation. The associated condition defaults to always returning `false`.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            // Default condition returns false. The actual logic should be supplied elsewhere
            // when using guards created this way.
            condition: Arc::new(|_ctx, _evt| false),
        }
    }

    /// Evaluates the guard condition against the given context and event.
    ///
    /// Executes the stored synchronous `condition` function.
    ///
    /// # Arguments
    /// * `context` - The current state machine context.
    /// * `event` - The triggering event.
    ///
    /// # Returns
    /// `true` if the guard condition passes, `false` otherwise.
    pub fn check(&self, context: &C, event: &E) -> bool {
        (self.condition)(context, event)
    }
}

// Display implementation shows the guard's name.
impl<C, E> fmt::Display for Guard<C, E>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guard({})", self.name)
    }
}

/// A trait for types that can be conveniently converted into a [`Guard`].
///
/// This is particularly useful for defining transitions with inline guard conditions.
pub trait IntoGuard<C, E>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
    Self: Sized,
{
    /// Performs the conversion into a `Guard`.
    fn into_guard(self) -> Guard<C, E>;
}

// Allow converting an existing Guard into a Guard (identity conversion).
impl<C, E> IntoGuard<C, E> for Guard<C, E>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_guard(self) -> Guard<C, E> {
        self
    }
}

// Allow converting a tuple of (&str, Fn) into a Guard.
impl<C, E, F> IntoGuard<C, E> for (&str, F)
where
    F: Fn(&C, &E) -> bool + Send + Sync + 'static,
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    /// Converts a tuple `(name: &str, condition: Fn(&C, &E) -> bool)` into a `Guard`.
    fn into_guard(self) -> Guard<C, E> {
        Guard::new(self.0, self.1)
    }
}

// Allow converting just a name (&str) into a named Guard (with a default false condition).
impl<C, E> IntoGuard<C, E> for &str
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    /// Converts a string slice representing a name into a `Guard::named()`.
    /// The condition function will default to returning `false`.
    fn into_guard(self) -> Guard<C, E> {
        Guard::named(self)
    }
}

// Allow converting just an owned String into a named Guard.
impl<C, E> IntoGuard<C, E> for String
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    /// Converts an owned `String` representing a name into a `Guard::named()`.
    /// The condition function will default to returning `false`.
    fn into_guard(self) -> Guard<C, E> {
        Guard::named(self)
    }
}

// Manual Debug implementation to avoid printing the function pointer.
impl<C, E> fmt::Debug for Guard<C, E>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Guard")
            .field("name", &self.name)
            .field("condition", &"<Fn(&C, &E) -> bool>")
            .finish()
    }
}

// Manual PartialEq implementation comparing only the name.
// Function pointers/closures cannot be reliably compared for equality.
impl<C, E> PartialEq for Guard<C, E>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
        // Warning: Two guards are considered equal if their names are the same,
        // even if their underlying condition logic differs.
    }
}

// Eq can be derived if PartialEq is implemented and the type meets Eq requirements.
// Since we only compare the name (String), which is Eq, Guard can be Eq.
impl<C, E> Eq for Guard<C, E>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
}

// Default implementation for Guard when C and E have defaults
impl<C, E> Default for Guard<C, E>
where
    C: Send + Sync + 'static + Default + Clone + fmt::Debug,
    E: EventTrait + Send + Sync + 'static,
{
    fn default() -> Self {
        Self {
            name: "default_guard".to_string(),
            // Default condition allows the transition
            condition: Arc::new(|_ctx, _evt| true),
        }
    }
}
