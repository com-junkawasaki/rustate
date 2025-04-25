use crate::event::IntoEvent;
use crate::{
    action::ActionType,
    actor::{ActorLogic, ActorStatus, Snapshot as ActorSnapshot},
    error::StateError,
    state::{HistoryType, State, StateCollection, StateType},
    transition::TransitionType,
    Action, Context, Error, Event, EventTrait, IntoAction, Result, StateTrait, Transition,
    context::ContextTrait,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::future::try_join_all;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{self, Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::str::FromStr;
use crate::actor::Snapshot;

/// Represents a state machine instance
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S: Serialize, C: Serialize",
    deserialize = "S: DeserializeOwned, C: DeserializeOwned"
))]
pub struct Machine<C = Context, E = Event, S = String, O = ()>
where
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + 'static,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Clone
        + Default
        + Eq
        + Serialize
        + DeserializeOwned
        + IntoEvent,
    S: StateTrait
        + Display
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Clone
        + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states (Use StateCollection for better management)
    #[serde(flatten)]
    pub states: StateCollection<S>,
    /// Collection of transitions
    pub transitions: Vec<Transition<S, C, E>>,
    /// Initial state id
    pub initial: S,
    /// Current active state IDs
    pub current_states: HashSet<S>,
    /// Current context data
    pub context: C,
    /// History states mapping (state id -> last active child)
    #[serde(default)]
    pub(crate) history: HashMap<String, S>,

    /// Entry/Exit actions are not typically serialized directly;
    /// they are part of the machine definition.
    /// Entry actions for states (managed internally or during build)
    #[serde(skip)]
    pub(crate) entry_actions: HashMap<String, Vec<Action<C, E>>>,
    /// Exit actions for states (managed internally or during build)
    #[serde(skip)]
    pub(crate) exit_actions: HashMap<String, Vec<Action<C, E>>>,

    /// The type markers
    #[serde(skip)]
    _phantom_e: PhantomData<E>,
    #[serde(skip)]
    _phantom_o: PhantomData<O>,
}

impl<C, E, S, O> Machine<C, E, S, O>
where
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + 'static,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Clone
        + Default
        + Eq
        + Serialize
        + DeserializeOwned
        + IntoEvent,
    S: StateTrait
        + Display
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Clone
        + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default,
{
    /// Create a new state machine instance from a builder
    pub async fn new(builder: MachineBuilder<C, E, S, O>) -> Result<Self> {
        let MachineBuilder {
            name,
            mut states,
            transitions,
            initial,
            entry_actions,
            exit_actions,
            context,
            _phantom_e: _,
            _phantom_o: _,
        } = builder;

        if states.states.is_empty() {
            return Err(Error::InvalidConfiguration("No states defined".into()));
        }

        if !states.contains(&initial) {
            return Err(Error::StateNotFound(initial.to_string()));
        }

        let initial_context = context.unwrap_or_default();

        for t in &transitions {
            if !states.contains(&t.source) {
                return Err(Error::StateNotFound(format!(
                    "Transition source '{}' not found",
                    t.source
                )));
            }
            if let Some(target) = &t.target {
                if !states.contains(target) {
                    return Err(Error::StateNotFound(format!(
                        "Transition target '{}' not found",
                        target
                    )));
                }
            }
        }

        let mut final_entry_actions = entry_actions;
        let mut final_exit_actions = exit_actions;
        for (id_str, state) in states.states.iter() {
            if !state.entry.is_empty() {
                final_entry_actions
                    .entry(id_str.clone())
                    .or_default()
                    .extend(state.entry.iter().map(|ia| ia.into_action()));
            }
            if !state.exit.is_empty() {
                final_exit_actions
                    .entry(id_str.clone())
                    .or_default()
                    .extend(state.exit.iter().map(|ia| ia.into_action()));
            }
        }

        let mut machine = Self {
            name,
            states,
            transitions,
            initial: initial.clone(),
            entry_actions: final_entry_actions,
            exit_actions: final_exit_actions,
            history: HashMap::new(),
            _phantom_e: PhantomData,
            _phantom_o: PhantomData,
            current_states: HashSet::new(),
            context: initial_context,
        };

        machine.initialize(&initial).await?;

        Ok(machine)
    }

    /// Initialize the machine by entering the initial state
    async fn initialize(&mut self, initial_state_id: &S) -> Result<()> {
        let init_event = E::from(Event::new("init"));
        self.enter_state(initial_state_id, &init_event).await?;
        Ok(())
    }

    /// Send an event to the machine
    pub async fn send<EV: IntoEvent + Send>(&mut self, event_in: EV) -> Result<bool> {
        let event = event_in.into_event();
        let event_ref: &E = &event;
        let mut processed = false;

        let current_state_ids: Vec<S> = self.current_states.iter().cloned().collect();

        let mut enabled_transition: Option<(&Transition<S, C, E>, &S)> = None;

        // Find transitions that match the current state and event
        for state_id in current_state_ids {
            if let Some(transitions) = self.transitions.get(&state_id) {
                for t in transitions {
                    // Check event match first
                    let event_matches = t.event.as_ref().map_or(false, |te| te.name() == event_ref.name());
                    if event_matches {
                        // Check guard condition if event matches
                        if t.is_enabled(&self.context, event_ref).await {
                            enabled_transition = Some((t, &state_id));
                            break; // Found the highest priority transition for this state
                        }
                    }
                }
            }
            if enabled_transition.is_some() {
                break; // Found a transition from one of the current states
            }
        }

        // Also check for transitions without a specific event (always triggers)
        if enabled_transition.is_none() {
            for current_state_id in &self.current_states {
                if let Some(transitions) = self.transitions.get(current_state_id) {
                    for t in transitions {
                        if t.event.is_none() { // Check for transitions with no event specified
                            if t.is_enabled(&self.context, event_ref).await {
                                enabled_transition = Some((t, current_state_id));
                                break;
                            }
                        }
                    }
                }
                if enabled_transition.is_some() {
                    break;
                }
            }
        }

        for t in &self.transitions {
            if t.source.to_string() == "*"
                && t.event == event_ref.name()
                && t.is_enabled(&self.context, event_ref).await
            {
                if enabled_transition.is_none() {
                    // Let's skip wildcards for now until source representation is clear.
                    // Log a warning maybe?
                    // eprintln!("Warning: Wildcard transition source representation unclear, skipping.");
                }
            }
        }

        if let Some((transition, source_state_id)) = enabled_transition {
            self.execute_transition(&transition.clone(), source_state_id, event_ref).await?;
            processed = true;
        } else {
            // Check for wildcard transitions if no specific transition was found
            for t in &self.transitions {
                // Check if source is wildcard "*"
                if t.source.to_string() == "*" {
                    // Check event match
                    let event_matches = t.event.as_ref().map_or(false, |te| te.name() == event_ref.name());
                    if event_matches {
                         // Check guard
                        if t.is_enabled(&self.context, event_ref).await {
                            // Execute wildcard transition (source state doesn't matter here)
                            // Need a representative source state ID for history, maybe initial?
                            self.execute_transition(&t.clone(), &self.initial.clone(), event_ref).await?;
                            processed = true;
                            break; // Assume only one wildcard transition executes
                        }
                    }
                }
            }

            if !processed {
                // No transition found for this event in the current states or via wildcard
                 return Ok(false);
            }
        }

        // Update history and potentially other logic after transition
        self.update_history();

        Ok(processed)
    }

    /// Execute a transition
    async fn execute_transition(
        &mut self,
        transition: &Transition<S, C, E>,
        source_state_id: &S,
        event: &E,
    ) -> Result<()> {
        if transition.target.is_none() || transition.transition_type == TransitionType::Internal {
            transition.execute_actions(&mut self.context, event).await?;
            return Ok(());
        }

        let target_state_id = transition.target.as_ref().unwrap();

        let lcca_id_str = self.find_lcca(source_state_id, target_state_id);

        let mut exit_queue = VecDeque::new();
        let mut current_exit: Option<S> = Some(source_state_id.clone());
        while let Some(state_to_exit) = current_exit {
            if lcca_id_str.as_deref() == Some(&state_to_exit.to_string()) {
                break;
            }
            exit_queue.push_back(state_to_exit.clone());
            current_exit = self
                .states
                .get(state_to_exit)
                .and_then(|s| s.parent.clone());
        }

        let mut entry_queue = VecDeque::new();
        let mut current_entry: Option<S> = Some(target_state_id.clone());
        while let Some(state_to_enter) = current_entry {
            if lcca_id_str.as_deref() == Some(&state_to_enter.to_string()) {
                break;
            }
            entry_queue.push_front(state_to_enter.clone());
            current_entry = self
                .states
                .get(state_to_enter)
                .and_then(|s| s.parent.clone());
        }

        for state_id in exit_queue {
            self.exit_state(&state_id, event).await?;
        }

        transition.execute_actions(&mut self.context, event).await?;

        for state_id in entry_queue {
            self.enter_state(&state_id, event).await?;
        }

        Ok(())
    }

    /// Enter a state and its initial children recursively
    #[async_recursion]
    async fn enter_state(&mut self, state_id: &S, event: &E) -> Result<()> {
        let state_id_str = state_id.to_string();
        println!("Entering state: {}", state_id_str);

        self.current_states.insert(state_id.clone());
        self.execute_entry_actions(state_id, event).await?;

        let state = self.states.get(state_id).unwrap();

        if let Some(parent_id) = &state.parent {
            if let Some(parent_state) = self.states.get(parent_id) {
                if parent_state.is_history() {
                    self.history.insert(parent_id.to_string(), state_id.clone());
                }
            }
        }

        match state.state_type {
            StateType::Compound => {
                if let Some(initial_child_id) = &state.initial {
                    self.enter_state(initial_child_id, event).await?;
                } else if let Some(history_type) = state.history() {
                    if let Some(last_active_child) = self.history.get(&state_id_str) {
                        self.enter_state(last_active_child, event).await?;
                    } else if let Some(initial_child_id) = &state.initial {
                        self.enter_state(initial_child_id, event).await?;
                    }
                    if history_type == HistoryType::Deep {
                        // TODO: Implement deep history traversal logic
                    }
                } else {
                    if let Some(first_child_key) = state.children().keys().next() {
                        if let Some(first_child_state) = state.children().get(first_child_key) {
                            self.enter_state(&first_child_state.id, event).await?;
                        }
                    }
                }
            }
            StateType::Parallel => {
                let child_ids: Vec<S> = state.children().values().map(|s| s.id.clone()).collect();
                for child_id in child_ids {
                    self.enter_state(&child_id, event).await?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Exit a state and its active children recursively
    #[async_recursion]
    async fn exit_state(&mut self, state_id: &S, event: &E) -> Result<()> {
        let state_id_str = state_id.to_string();
        println!("Exiting state: {}", state_id_str);

        let state = self.states.get(state_id).unwrap().clone();

        let active_children: Vec<S> = state
            .children()
            .values()
            .filter(|child| self.current_states.contains(&child.id))
            .map(|child| child.id.clone())
            .collect();

        for child_id in active_children {
            self.exit_state(&child_id, event).await?;
        }

        self.update_history_on_exit(state_id);

        self.execute_exit_actions(state_id, event).await?;
        self.current_states.remove(state_id);

        Ok(())
    }

    /// Execute entry actions for a state
    async fn execute_entry_actions(&self, state_id: &S, event: &E) -> Result<(), Error> {
        if let Some(actions) = self.entry_actions.get(&state_id.to_string()) {
            for action in actions {
                action.execute(&mut self.context.clone(), event).await?;
            }
        }
        Ok(())
    }

    /// Execute exit actions for a state
    async fn execute_exit_actions(&self, state_id: &S, event: &E) -> Result<(), Error> {
        if let Some(actions) = self.exit_actions.get(&state_id.to_string()) {
            for action in actions {
                action.execute(&mut self.context.clone(), event).await?;
            }
        }
        Ok(())
    }

    /// Update history state when exiting a state
    fn update_history_on_exit(&mut self, exited_state_id: &S) {
        if let Some(exited_state) = self.states.get(exited_state_id) {
            if let Some(parent_id) = &exited_state.parent {
                if let Some(parent_state) = self.states.get(parent_id) {
                    match parent_state.state_type {
                        StateType::Compound => {
                            if parent_state.is_history() {
                                self.history
                                    .insert(parent_id.to_string(), exited_state_id.clone());
                            }
                        }
                        StateType::DeepHistory => {
                            self.history
                                .insert(parent_id.to_string(), exited_state_id.clone());
                            let mut current_parent_id = parent_state.parent.clone();
                            while let Some(ancestor_id) = current_parent_id {
                                if let Some(ancestor_state) = self.states.get(&ancestor_id) {
                                    if ancestor_state.state_type == StateType::DeepHistory {
                                        // Find the descendant of the ancestor that is the ancestor of the exited state
                                        // This seems overly complex, maybe just store the full path?
                                        // Let's stick to simple parent update for now.
                                        // self.history.insert(ancestor_id.to_string(), descendant_id.clone());
                                    }
                                    current_parent_id = ancestor_state.parent.clone();
                                } else {
                                    break;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Find the least common compound ancestor (LCCA) of two states
    fn find_lcca(&self, state1_id: &S, state2_id: &S) -> Option<String> {
        let ancestors1 = self.get_ancestors_inclusive(state1_id);
        let ancestors2 = self.get_ancestors_inclusive(state2_id);
        let ancestors1_set: HashSet<_> = ancestors1.iter().collect();

        for ancestor_id_str in ancestors2.iter() {
            if ancestors1_set.contains(ancestor_id_str) {
                if let Some(state) = self.states.states.get(ancestor_id_str) {
                    if state.is_compound() || state.is_parallel() {
                        return Some(ancestor_id_str.clone());
                    }
                }
            }
        }
        None
    }

    /// Get all ancestors of a state, including itself (as Strings)
    fn get_ancestors_inclusive(&self, state_id: &S) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current_id = Some(state_id.clone());
        while let Some(id) = current_id {
            ancestors.push(id.to_string());
            current_id = self.get_parent_id(&id);
        }
        ancestors.reverse();
        ancestors
    }

    /// Get the parent ID of a state
    fn get_parent_id(&self, state_id: &S) -> Option<S> {
        self.states.get(state_id).and_then(|s| s.parent.clone())
    }

    /// Check if the machine is currently in a specific state
    pub fn is_in(&self, state_id: &S) -> bool {
        if self.current_states.contains(state_id) {
            return true;
        }
        for current in &self.current_states {
            let mut parent = self.get_parent_id(current);
            while let Some(p) = parent {
                if p == *state_id {
                    return true;
                }
                parent = self.get_parent_id(&p);
            }
        }
        false
    }

    /// Returns a list of ancestor state IDs for a given state ID.
    fn get_ancestors(&self, state_id: &S) -> Vec<S> {
        let mut ancestors = Vec::new();
        let mut current_id = state_id.clone();

        while let Some(state) = self.states.get(&current_id) { // Use public get method
            if let Some(parent_id) = &state.parent { // Access parent field
                ancestors.push(parent_id.clone());
                current_id = parent_id.clone();
            } else {
                break; // Reached root state
            }
        }
        ancestors // Return the collected ancestors
    }

    /// Serializes the machine definition to a JSON string.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| StateError::Serialization(e.to_string()))
    }

    /// Get the depth of a state in the hierarchy
    fn get_state_depth(&self, state_id: &S) -> usize {
        let mut depth = 0;
        let mut current_id = Some(state_id.clone());
        while let Some(id) = current_id {
            if let Some(parent_id) = self.get_parent_id(&id) {
                depth += 1;
                current_id = Some(parent_id);
            } else {
                break;
            }
        }
        depth
    }
}

/// Builder for creating Machine instances
#[derive(Clone)]
pub struct MachineBuilder<C = Context, E = Event, S = String, O = ()>
where
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + 'static,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Clone
        + Default
        + Eq
        + Serialize
        + DeserializeOwned,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states (Use StateCollection for better management)
    pub states: StateCollection<S>,
    /// Collection of transitions
    pub transitions: Vec<Transition<S, C, E>>,
    /// Initial state id (Use S directly)
    pub initial: S,
    /// Context for the machine
    pub context: Option<C>,
    /// Entry actions for states
    pub(crate) entry_actions: HashMap<String, Vec<Action<C, E>>>,
    /// Exit actions for states
    pub(crate) exit_actions: HashMap<String, Vec<Action<C, E>>>,
    /// Type markers
    _phantom_e: PhantomData<E>,
    _phantom_o: PhantomData<O>,
}

impl<C, E, S, O> MachineBuilder<C, E, S, O>
where
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + 'static,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Clone
        + Default
        + Eq
        + Serialize
        + DeserializeOwned,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default,
{
    /// Create a new MachineBuilder
    pub fn new(name: impl Into<String>, initial: S) -> Self {
        Self {
            name: name.into(),
            states: StateCollection::new(),
            transitions: Vec::new(),
            initial,
            context: None,
            entry_actions: HashMap::new(),
            exit_actions: HashMap::new(),
            _phantom_e: PhantomData,
            _phantom_o: PhantomData,
        }
    }

    /// Add a state definition
    pub fn state(mut self, state: State<S>) -> Self {
        self.states.add(state);
        self
    }

    /// Add a global transition definition
    pub fn transition(mut self, transition: Transition<S, C, E>) -> Self {
        self.transitions.push(transition);
        self
    }

    /// Add an entry action for a specific state
    pub fn on_entry<A: Into<Action<C, E>> + 'static>(mut self, state_id: &S, action: A) -> Self {
        self.entry_actions
            .entry(state_id.to_string())
            .or_default()
            .push(action.into().into_action());
        self
    }

    /// Add an exit action for a specific state
    pub fn on_exit<A: Into<Action<C, E>> + 'static>(mut self, state_id: &S, action: A) -> Self {
        self.exit_actions
            .entry(state_id.to_string())
            .or_default()
            .push(action.into().into_action());
        self
    }

    /// Set the initial context for the machine
    pub fn context(mut self, context: C) -> Self {
        self.context = Some(context);
        self
    }

    /// Build the Machine instance
    pub async fn build(self) -> Result<Machine<C, E, S, O>> {
        Machine::new(self).await
    }
}

/// Snapshot of the machine state for actors
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineSnapshot<C, S, O> {
    pub inner: ActorSnapshot<C, O>,
    pub current_states: HashSet<S>,
    pub history_states: HashMap<S, HashSet<S>>,
    _phantom_s: PhantomData<S>,
}

impl<C, S, O> MachineSnapshot<C, S, O>
where
    S: StateTrait + Send + Sync + 'static + Serialize + DeserializeOwned,
    C: Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
    O: Clone + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    pub fn value(&self) -> &Value {
        &self.inner.value
    }
    pub fn context(&self) -> &C {
        &self.inner.context
    }
    pub fn output(&self) -> Option<&O> {
        self.inner.output.as_ref()
    }
    pub fn status(&self) -> &ActorStatus {
        &self.inner.status
    }
    pub fn is_in(&self, state_id: &S) -> bool {
        self.current_states.contains(state_id)
    }
    pub fn current_states(&self) -> &HashSet<S> {
        &self.current_states
    }
}

#[async_trait]
impl<C, E, S, O> ActorLogic<MachineSnapshot<C, S, O>, E, O> for Machine<C, E, S, O>
where
    S: StateTrait + 'static + Send + Sync,
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + 'static,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Clone
        + Default
        + Eq
        + Serialize
        + DeserializeOwned
        + IntoEvent,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default,
{
    type Input = Option<O>;
    type Query = String;
    type Response = Result<MachineSnapshot<C, S, O>, StateError>;

    fn get_initial_snapshot(&self, input: Option<O>) -> MachineSnapshot<C, S, O> {
        let initial_state_id = self.initial.clone();
        let mut initial_states = HashSet::new();
        initial_states.insert(initial_state_id.clone());

        let initial_context = self.context.clone();

        MachineSnapshot {
            inner: ActorSnapshot {
                context: initial_context,
                value: serde_json::to_value(&initial_state_id).unwrap_or(Value::Null),
                output: input.or_else(O::default),
                status: ActorStatus::Active,
            },
            current_states: initial_states,
            history_states: self.history.clone(),
            _phantom_s: PhantomData,
        }
    }

    async fn transition(
        &self,
        snapshot: MachineSnapshot<C, S, O>,
        event: E,
    ) -> Result<MachineSnapshot<C, S, O>, StateError> {
        let mut current_snapshot = snapshot;
        let mut temp_machine = self.clone();
        temp_machine.current_states = current_snapshot.current_states.clone();
        temp_machine.context = current_snapshot.inner.context.clone();
        temp_machine.history = current_snapshot.history_states.clone();

        let processed = temp_machine.send(event.clone()).await?;

        if processed {
            current_snapshot.current_states = temp_machine.current_states;
            current_snapshot.inner.context = temp_machine.context;
            current_snapshot.history_states = temp_machine.history;
            current_snapshot.inner.value =
                serde_json::to_value(&current_snapshot.current_states).unwrap_or(Value::Null);
            current_snapshot.inner.output = current_snapshot
                .current_states
                .iter()
                .find_map(|state_id| self.outputs.get(state_id))
                .cloned();

            if current_snapshot
                .current_states
                .iter()
                .any(|s| self.states.get(s).map_or(false, |st| st.is_final()))
            {
                current_snapshot.inner.status = ActorStatus::Done;
                if current_snapshot.inner.output.is_none() {
                     current_snapshot.inner.output = Some(O::default());
                }
            } else {
                 current_snapshot.inner.status = ActorStatus::Active;
            }
        }

        Ok(current_snapshot)
    }

    fn on_error(
        &self,
        error: StateError,
        mut snapshot: MachineSnapshot<C, S, O>,
    ) -> MachineSnapshot<C, S, O> {
        eprintln!("Actor state machine error: {:?}", error);
        snapshot.inner.status = ActorStatus::Error;
        snapshot
    }
}
