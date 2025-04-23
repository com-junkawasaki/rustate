use crate::{action::ActionType, Action, Context, Event, Guard, IntoAction, IntoGuard};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a transition between states
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transition {
    /// Source state id
    pub source: String,
    /// Target state id
    pub target: Option<String>,
    /// Event type that triggers this transition
    pub event: String,
    /// Optional guard condition
    pub guard: Option<Guard>,
    /// Actions to execute during the transition
    pub actions: Vec<Action>,
    /// Internal id for this transition
    #[serde(default = "uuid::Uuid::new_v4")]
    pub(crate) id: uuid::Uuid,
}

impl Transition {
    /// Create a new transition
    pub fn new(
        source: impl Into<String>,
        event: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            target: Some(target.into()),
            event: event.into(),
            guard: None,
            actions: Vec::new(),
            id: uuid::Uuid::new_v4(),
        }
    }

    /// Create a new self-transition (target is the same as source)
    pub fn self_transition(state: impl Into<String>, event: impl Into<String>) -> Self {
        let state = state.into();
        Self {
            source: state.clone(),
            target: Some(state),
            event: event.into(),
            guard: None,
            actions: Vec::new(),
            id: uuid::Uuid::new_v4(),
        }
    }

    /// Create a new internal transition (no exit/entry actions, just the transition actions)
    pub fn internal_transition(state: impl Into<String>, event: impl Into<String>) -> Self {
        Self {
            source: state.into(),
            target: None,
            event: event.into(),
            guard: None,
            actions: Vec::new(),
            id: uuid::Uuid::new_v4(),
        }
    }

    /// Add a guard condition to this transition
    pub fn with_guard<G: IntoGuard>(&mut self, guard: G) -> &mut Self {
        self.guard = Some(guard.into_guard());
        self
    }

    /// Add an action to this transition
    pub fn with_action<A: IntoAction>(&mut self, action: A) -> &mut Self {
        self.actions
            .push(action.into_action(ActionType::Transition));
        self
    }

    /// Check if this transition is triggered by the given event
    pub fn matches_event(&self, event: &Event) -> bool {
        self.event == event.event_type || self.event == crate::event::WILDCARD_EVENT
    }

    /// Check if this transition is enabled given the context and event
    pub async fn is_enabled(&self, context: &Context, event: &Event) -> bool {
        if !self.matches_event(event) {
            return false;
        }
        match &self.guard {
            Some(guard) => guard.evaluate(context, event).await,
            None => true,
        }
    }

    /// Execute this transition's actions
    pub async fn execute_actions(&self, context: &mut Context, event: &Event) {
        for action in &self.actions {
            action.execute(context, event).await;
        }
    }
}

impl fmt::Display for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target {
            Some(target) => write!(f, "{} -- {} --> {}", self.source, self.event, target),
            None => write!(f, "{} -- {} (internal)", self.source, self.event),
        }
    }
}
