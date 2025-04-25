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
use futures::Future;
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
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + Serialize + DeserializeOwned,
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
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + Serialize + DeserializeOwned,
{
    /// Create a new transition
    pub fn new(
        source: impl Into<S>,
        target: Option<impl Into<S>>,
        event: Option<E>,
        guard: Option<Guard<C, E>>,
        actions: Vec<Action<C, E>>,
        transition_type: TransitionType,
    ) -> Self
    where
        E: EventTrait + Send + Sync + 'static + Clone + Eq + Serialize + DeserializeOwned,
    {
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

    /// Creates a new internal transition.
    pub fn internal_transition(source: impl Into<S>, event: E) -> Self
    where
        S: 'static,
        C: Clone + Send + Sync + 'static,
        E: EventTrait + Send + Sync + 'static + Clone + Eq + Serialize + DeserializeOwned,
    {
        // Use Transition::new for internal transitions as well
        Transition::new(
            source,                   // source
            None,                     // target (None for internal)
            Some(event),              // event
            None,                     // guard
            vec![],                   // actions
            TransitionType::Internal, // transition_type
        )
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
            Some(guard) => guard.check(context, event),
            None => true, // No guard means always true
        }
    }
}

impl<S, C, E> fmt::Debug for Transition<S, C, E>
where
    S: StateTrait + fmt::Debug,
    C: Clone + Send + Sync + 'static + fmt::Debug,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + fmt::Debug + Serialize + DeserializeOwned,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transition")
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
    S: StateTrait + Eq,
    C: Clone + Send + Sync + 'static + PartialEq,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + Serialize + DeserializeOwned,
{
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.target == other.target
            && self.event == other.event
            && self.guard == other.guard
            && self.actions == other.actions
            && self.transition_type == other.transition_type
    }
}

impl<S, C, E> Eq for Transition<S, C, E>
where
    S: StateTrait + Eq,
    C: Clone + Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Eq + Serialize + DeserializeOwned,
{
}

/// Represents the type of transition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionType {
    External, // Exits source state, enters target state
    Internal, // Stays within the source state, only executes actions
}

trait TransitionTrait<C, E> {
    async fn is_enabled(&self, context: &C, event: &E) -> bool;
    async fn execute_actions(&self, context: &mut C, event: &E) -> Result<()>;
}

impl<S, C, E> TransitionTrait<C, E> for Transition<S, C, E>
where
    S: StateTrait + Send + Sync + 'static,
    C: Send + Sync + 'static,
    E: EventTrait + Send + Sync + 'static,
{
    /// Check if the transition guard allows the transition
    async fn is_enabled(&self, context: &C, event: &E) -> bool {
        if let Some(guard) = &self.guard {
            guard.check(context, event).await
        } else {
            true
        }
    }

    /// Execute all actions associated with this transition
    async fn execute_actions(&self, context: &mut C, event: &E) -> Result<()> {
        for action in &self.actions {
            action.execute(context, event).await?;
        }
        Ok(())
    }
}
