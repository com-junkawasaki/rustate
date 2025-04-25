use crate::guard::{Guard, IntoGuard};
use crate::{
    action::{Action, ActionType, IntoAction},
    context::Context,
    error::Result,
    event::{Event, EventTrait, IntoEvent},
    state::{State, StateTrait},
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::future::try_join_all;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

/// Represents a transition between states
#[derive(Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S: Serialize, E: Serialize",
    deserialize = "S: DeserializeOwned, E: DeserializeOwned"
))]
pub struct Transition<S = String, C = Context, E = Event>
where
    S: StateTrait + Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + From<Event> + Serialize + DeserializeOwned,
{
    /// Source state id
    pub source: S,
    /// Target state id (Optional for internal transitions)
    pub target: Option<S>,
    /// Event type that triggers this transition (Optional for eventless transitions)
    pub event: Option<E>,
    /// Optional guard condition (Logic, not serialized)
    #[serde(skip)]
    pub guard: Option<Guard<C, E>>,
    /// Actions to execute during the transition (Logic, not serialized)
    #[serde(skip)]
    pub actions: Vec<Action<C, E>>,
    /// Internal id for this transition
    #[serde(default = "uuid::Uuid::new_v4")]
    pub(crate) id: Uuid,
    /// Type of transition (External or Internal)
    pub transition_type: TransitionType,
    // PhantomData should always be skipped
    #[serde(skip)]
    _phantom_s: PhantomData<S>,
    #[serde(skip)]
    _phantom_c: PhantomData<C>,
    #[serde(skip)]
    _phantom_e: PhantomData<E>,
}

impl<S, C, E> Transition<S, C, E>
where
    S: StateTrait + Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + From<Event> + Serialize + DeserializeOwned,
{
    /// Create a new transition
    pub fn new(
        source: impl Into<S>,
        target: Option<impl Into<S>>,
        event: Option<E>,
        guard: Option<Guard<C, E>>,
        actions: Vec<Action<C, E>>,
        transition_type: TransitionType,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source: source.into(),
            target: target.map(|t| t.into()),
            event,
            guard,
            actions,
            transition_type,
            _phantom_s: PhantomData,
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
        }
    }

    /// Create a new self-transition (target is the same as source)
    pub fn self_transition(state: S, event: E) -> Self {
        Self {
            source: state.clone(),
            target: Some(state),
            event: Some(event),
            guard: None,
            actions: Vec::new(),
            id: uuid::Uuid::new_v4(),
            transition_type: TransitionType::External,
            _phantom_s: PhantomData,
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
        }
    }

    /// Create a new internal transition (no exit/entry actions, just the transition actions)
    pub fn internal_transition<SId: Into<String>, EIn: IntoEvent>(state_id: SId, event: EIn) -> Self
    where
        S: From<String>,
    {
        Self {
            source: S::from(state_id.into()),
            target: None,
            event: Some(event.into_event().into()),
            guard: None,
            actions: Vec::new(),
            id: uuid::Uuid::new_v4(),
            transition_type: TransitionType::Internal,
            _phantom_s: PhantomData,
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
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
        match &self.event {
            Some(transition_event) => transition_event == event,
            None => true, // Eventless transitions always match
        }
    }

    /// Check if this transition is enabled given the context and event
    pub async fn is_enabled(&self, context: &C, event: &E) -> bool {
        self.matches_event(event) && self.check_guard(context, event).await
    }

    /// Execute this transition's actions
    #[async_recursion]
    pub async fn execute_actions(&self, context: &mut C, event: &E) -> Result<()> {
        for action in &self.actions {
            action.execute(context, event).await?;
        }
        Ok(())
    }

    pub async fn check_guard(&self, context: &C, event: &E) -> bool {
        match &self.guard {
            Some(guard) => guard.evaluate(context, event).await,
            None => true, // No guard means always true
        }
    }
}

impl<S, C, E> fmt::Debug for Transition<S, C, E>
where
    S: StateTrait + Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transition")
            .field("id", &self.id)
            .field("source", &self.source)
            .field("target", &self.target)
            .field("event", &self.event)
            .field("guard", &self.guard)
            .field("actions", &self.actions)
            .field("transition_type", &self.transition_type)
            .finish()
    }
}

impl<S, C, E> PartialEq for Transition<S, C, E>
where
    S: StateTrait + Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + From<Event>,
{
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.event == other.event
            && self.target == other.target
            && self.actions.len() == other.actions.len()
            && self.guard.is_some() == other.guard.is_some()
    }
}

impl<S, C, E> Eq for Transition<S, C, E>
where
    S: StateTrait + Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + From<Event>,
{
}

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
