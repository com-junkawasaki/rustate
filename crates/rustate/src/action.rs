use crate::Error::ActionError;
use crate::{Context, Event, EventTrait, Result};
use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Type alias for the action executor function
pub type ActionExecutor =
    Box<dyn Fn(&mut Context, &Event) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

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

/// Define Action as generic over C, E
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Action<C = Context, E = Event>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// The name of this action
    pub name: String,
    /// The type of action execution
    pub action_type: ActionType,
    #[allow(clippy::type_complexity)]
    /// Function pointer to execute the action
    pub(crate) execute_fn: Arc<dyn Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync>,
    _phantom_c: std::marker::PhantomData<C>,
    _phantom_e: std::marker::PhantomData<E>,
}

// Need to manually implement Debug because BoxFuture is not Debug
impl<C, E> fmt::Debug for Action<C, E>
where
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Action")
            .field("name", &self.name)
            .field("exec", &"<Fn>") // Don't print the function itself
            .field("action_type", &self.action_type)
            .finish()
    }
}

impl<C, E> Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Create a new action with an async function
    pub fn new<F>(name: impl Into<String>, action_type: ActionType, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            action_type,
            execute_fn: Arc::new(execute_fn),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Create a new action with a synchronous function
    pub fn new_sync<F>(name: impl Into<String>, action_type: ActionType, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) + Send + Sync + 'static,
    {
        let async_fn = move |ctx: &mut C, evt: &E| {
            execute_fn(ctx, evt);
            Box::pin(async {}) as BoxFuture<'static, ()>
        };
        Self {
            name: name.into(),
            action_type,
            execute_fn: Arc::new(async_fn),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Execute the action
    pub async fn execute(&self, context: &mut C, event: &E) -> Result<(), Error> {
        (self.execute_fn)(context, event).await
    }
}

impl<C, E> Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Create a new entry action
    pub fn entry<F>(name: impl Into<String>, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Entry, execute_fn)
    }

    /// Create a new exit action
    pub fn exit<F>(name: impl Into<String>, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Exit, execute_fn)
    }

    /// Create a new transition action
    pub fn transition<F>(name: impl Into<String>, execute_fn: F) -> Self
    where
        F: Fn(&mut C, &E) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self::new(name, ActionType::Transition, execute_fn)
    }

    /// Create a new action with a name only (for serialization)
    pub fn named(name: impl Into<String>, action_type: ActionType) -> Self {
        Self {
            name: name.into(),
            action_type,
            execute_fn: Arc::new(|_ctx, _evt| Box::pin(async {})),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Clone without the executor
    pub fn without_executor(&self) -> Self {
        Self {
            name: self.name.clone(),
            action_type: self.action_type,
            execute_fn: Arc::new(|_ctx, _evt| Box::pin(async {})),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }
}

impl<C, E> fmt::Display for Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Action({}, {:?})", self.name, self.action_type)
    }
}

/// Trait for types that can be converted into an action.
pub trait IntoAction<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Converts the type into an action.
    fn into_action(self) -> Action<C, E>;
}

// Implement IntoAction for closures
impl<F, Fut, C, E> IntoAction<C, E> for F
where
    F: Fn(&mut C, &E) -> Fut + Send + Sync + 'static + Clone,
    Fut: futures::Future<Output = Result<(), Error>> + Send + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_action(self) -> Action<C, E> {
        Action {
            name: "closure_action".to_string(), // Consider a way to name these?
            action_type: ActionType::Transition, // Default type, might need adjustment
            execute_fn: Arc::new(move |ctx, evt| Box::pin(self(ctx, evt)) as _),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }
}

// Implement IntoAction for Action itself
impl<C, E> IntoAction<C, E> for Action<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_action(self) -> Action<C, E> {
        self
    }
}
