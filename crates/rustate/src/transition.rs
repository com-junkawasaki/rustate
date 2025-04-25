use crate::{
    action::{Action, ActionType, IntoAction},
    context::Context,
    error::Result,
    event::{Event, EventTrait},
    guard::{Guard, IntoGuard},
    state::{State, StateTrait},
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use std::sync::Arc;
use thiserror::Error;

/// Represents a transition between states
#[derive(Clone, Serialize, Deserialize)]
pub struct Transition<S, C = Context, E = Event>
where
    S: StateTrait + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Source state id
    pub source: S,
    /// Target state id
    pub target: Option<S>,
    /// Event type that triggers this transition
    pub event: E,
    /// Optional guard condition
    pub guard: Option<Guard<C, E>>,
    /// Actions to execute during the transition
    pub actions: Vec<Action<C, E>>,
    /// Internal id for this transition
    #[serde(default = "uuid::Uuid::new_v4")]
    pub(crate) id: uuid::Uuid,
    pub transition_type: TransitionType,
    _phantom_s: std::marker::PhantomData<S>,
    _phantom_c: std::marker::PhantomData<C>,
    _phantom_e: std::marker::PhantomData<E>,
}

impl<S, C, E> Transition<S, C, E>
where
    S: StateTrait + Send + Sync + 'static + Clone,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Create a new transition
    pub fn new(source: S, event: E, target: Option<S>) -> Self {
        Self {
            source,
            event,
            target,
            actions: Vec::new(),
            guard: None,
            id: uuid::Uuid::new_v4(),
            transition_type: TransitionType::External,
            _phantom_s: std::marker::PhantomData,
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Create a new self-transition (target is the same as source)
    pub fn self_transition(state: S, event: E) -> Self {
        Self {
            source: state.clone(),
            target: Some(state),
            event,
            guard: None,
            actions: Vec::new(),
            id: uuid::Uuid::new_v4(),
            transition_type: TransitionType::External,
            _phantom_s: std::marker::PhantomData,
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Create a new internal transition (no exit/entry actions, just the transition actions)
    pub fn internal_transition(state: S, event: E) -> Self {
        Self {
            source: state,
            target: None,
            event,
            guard: None,
            actions: Vec::new(),
            id: uuid::Uuid::new_v4(),
            transition_type: TransitionType::Internal,
            _phantom_s: std::marker::PhantomData,
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Add a guard condition to this transition
    pub fn with_guard(mut self, guard: impl IntoGuard<C, E>) -> Self {
        self.guard = Some(guard.into_guard());
        self
    }

    /// Add an action to this transition
    pub fn with_action(mut self, action: impl IntoAction<C, E>) -> Self {
        self.actions.push(action.into_action());
        self
    }

    /// Check if this transition is triggered by the given event
    pub fn matches_event(&self, event: &E) -> bool {
        self.event == event || self.event == crate::event::WILDCARD_EVENT
    }

    /// Check if this transition is enabled given the context and event
    pub async fn is_enabled(&self, context: &C, event: &E) -> bool {
        if !self.matches_event(event) {
            return false;
        }
        match &self.guard {
            Some(guard) => guard.evaluate(context, event).await,
            None => true,
        }
    }

    /// Execute this transition's actions
    #[async_recursion]
    pub async fn execute_actions(&self, context: &mut C, event: &E) {
        let futures = self
            .actions
            .iter()
            .map(|action| action.execute(context, event));
        let results = try_join_all(futures).await;
        if let Err(e) = results {
            eprintln!("Error executing transition action: {:?}", e);
        }
    }
}

impl<S, C, E> fmt::Debug for Transition<S, C, E>
where
    S: StateTrait + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transition")
            .field("source", &self.source.id())
            .field("event", &self.event.event_type())
            .field("target", &self.target.as_ref().map(|t| t.id()))
            .field("actions_count", &self.actions.len())
            .field("has_guard", &self.guard.is_some())
            .finish()
    }
}

impl<S, C, E> PartialEq for Transition<S, C, E>
where
    S: StateTrait + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source &&
        self.event == other.event &&
        self.target == other.target &&
        self.actions.len() == other.actions.len() &&
        self.guard.is_some() == other.guard.is_some()
    }
}

impl<S, C, E> Eq for Transition<S, C, E>
where
    S: StateTrait + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{}

/// Represents the type of transition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionType {
    External, // Exits source state, enters target state
    Internal, // Stays within the source state, only executes actions
}

/// Represents a guard condition for a transition.
#[derive(Clone)] // Guards need to be Clone
pub struct Guard<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    #[allow(clippy::type_complexity)]
    condition: Arc<dyn Fn(&C, &E) -> futures::future::BoxFuture<'static, bool> + Send + Sync>,
    _phantom_c: std::marker::PhantomData<C>,
    _phantom_e: std::marker::PhantomData<E>,
}

// Need to manually implement Debug because BoxFuture is not Debug
impl<C, E> fmt::Debug for Guard<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Guard").finish_non_exhaustive()
    }
}

impl<C, E> Guard<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Creates a new synchronous guard.
    pub fn new(sync_fn: fn(&C, &E) -> bool) -> Self {
        let condition = Arc::new(move |ctx: &C, evt: &E| {
            let result = sync_fn(ctx, evt);
            Box::pin(async move { result }) as futures::future::BoxFuture<'static, bool>
        });
        Self {
            condition,
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Comment out async guard due to lifetime issues
    /*
    pub fn new_async<F>(async_fn: F) -> Self
    where
        F: for<'a> Fn(&'a C, &'a E) -> futures::future::BoxFuture<'a, bool> + Send + Sync + 'static + Clone,
    {
         let async_fn_arc = Arc::new(async_fn);
         let condition = Arc::new(move |ctx: &C, evt: &E| {
             let async_fn_clone = async_fn_arc.clone();
             let ctx_clone = ctx.clone();
             let evt_clone = evt.clone();
             Box::pin(async move { async_fn_clone(&ctx_clone, &evt_clone).await }) as futures::future::BoxFuture<'static, bool>
         });
        Self {
            condition,
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }
    */

    /// Evaluates the guard condition.
    pub async fn evaluate(&self, context: &C, event: &E) -> bool {
        (self.condition)(context, event).await
    }
}

// Trait for types convertible to Guard
pub trait IntoGuard<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_guard(self) -> Guard<C, E>;
}

// Implement for sync functions
impl<C, E> IntoGuard<C, E> for fn(&C, &E) -> bool
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_guard(self) -> Guard<C, E> {
        Guard::new(self)
    }
}

// Comment out IntoGuard impl for async functions
/*
impl<F, C, E> IntoGuard<C, E> for F
where
    F: for<'a> Fn(&'a C, &'a E) -> futures::future::BoxFuture<'a, bool> + Send + Sync + 'static + Clone,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_guard(self) -> Guard<C, E> {
        Guard::new_async(self)
    }
}
*/

// Implement for Guard itself (identity)
impl<C, E> IntoGuard<C, E> for Guard<C, E>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    fn into_guard(self) -> Guard<C, E> {
        self
    }
}
