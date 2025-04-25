use crate::event::IntoEvent;
use crate::{
    action::{ActionType, error::ActionError},
    actor::{ActorLogic, ActorStatus, Snapshot as ActorSnapshot},
    state::{StateType, HistoryType},
    transition::TransitionType,
    Action, Context, Error, Event, EventTrait, IntoAction, Result, State, StateTrait, Transition,
    error::StateError,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{self, Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Deref;
use std::str::FromStr;

/// Represents a state machine instance
#[derive(Clone, Debug)]
pub struct Machine<C = Context, E = Event, S = String, O = ()>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Default + Eq + From<Event> + Clone,
    S: StateTrait
        + Display
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Clone
        + From<String>
        + Deref<Target = str>
        + Serialize
        + for<'de> DeserializeOwned,
    O: Clone + Send + Sync + 'static,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states
    pub states: HashMap<String, State<S>>,
    /// Collection of transitions
    pub transitions: Vec<Transition<S, C, E>>,
    /// Initial state id
    pub initial: String,
    /// Entry actions for states
    #[serde(skip)]
    pub(crate) entry_actions: HashMap<String, Vec<Action<C, E>>>,
    /// Exit actions for states
    #[serde(skip)]
    pub(crate) exit_actions: HashMap<String, Vec<Action<C, E>>>,
    /// History states mapping (state id -> last active child)
    pub(crate) history: HashMap<String, String>,
    /// The type markers
    #[serde(skip)]
    _phantom_c: PhantomData<C>,
    #[serde(skip)]
    _phantom_e: PhantomData<E>,
    #[serde(skip)]
    _phantom_s: PhantomData<S>,
    #[serde(skip)]
    _phantom_o: PhantomData<O>,
    pub current_states: HashSet<S>,
    pub context: C,
}

impl<C, E, S, O> Machine<C, E, S, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Default + Eq + From<Event> + Clone,
    S: StateTrait
        + Display
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Clone
        + From<String>
        + Deref<Target = str>
        + Serialize
        + for<'de> DeserializeOwned,
    O: Clone + Send + Sync + 'static,
{
    /// Create a new state machine instance from a builder
    pub async fn new<BuilderC, BuilderE, BuilderS, BuilderO>(
        builder: MachineBuilder<BuilderC, BuilderE, BuilderS, BuilderO>,
    ) -> Result<Self>
    where
        BuilderC: Clone + Send + Sync + Default + 'static,
        BuilderE: EventTrait + Send + Sync + 'static + Default + Eq + From<Event> + Clone,
        BuilderS: StateTrait
            + Send
            + Sync
            + 'static
            + Clone
            + From<String>
            + Deref<Target = str>
            + Serialize
            + for<'de> DeserializeOwned,
        BuilderO: Clone + Send + Sync + 'static,
    {
        let MachineBuilder {
            name,
            states,
            transitions,
            initial,
            entry_actions,
            exit_actions,
            context,
            _phantom_c: _,
            _phantom_e: _,
            _phantom_s: _,
            _phantom_o: _,
        } = builder;

        if states.is_empty() {
            return Err(Error::InvalidConfiguration("No states defined".into()));
        }

        if !states.contains_key(&initial) {
            return Err(Error::StateNotFound(initial.clone()));
        }

        let mut machine = Self {
            name,
            states,
            transitions,
            initial,
            entry_actions,
            exit_actions,
            history: HashMap::new(),
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
            _phantom_s: PhantomData,
            _phantom_o: PhantomData,
            current_states: HashSet::new(),
            context,
        };

        // Initialize by entering the initial state
        machine.initialize().await?;

        Ok(machine)
    }

    /// Initialize the machine by entering the initial state
    async fn initialize(&mut self) -> Result<()> {
        let initial_state_id = S::from(self.initial.clone());
        let init_event = E::from(Event::new("init"));
        self.enter_state(&initial_state_id, &init_event).await?;
        Ok(())
    }

    /// Send an event to the machine
    pub async fn send<EV: IntoEvent + Send>(&mut self, event_in: EV) -> Result<bool> {
        let event: E = event_in.into_event().into();
        let mut processed = false;

        let current_state_ids: Vec<S> = self.current_states.iter().cloned().collect();

        for state_id in current_state_ids {
            if self.process_state_event(&state_id, &event).await? {
                processed = true;
            }
        }

        Ok(processed)
    }

    /// Process an event for a specific state
    #[async_recursion]
    async fn process_state_event(&mut self, state_id: &S, event: &E) -> Result<bool> {
        let state_id_str: &str = state_id;
        let state = self
            .states
            .get(state_id_str)
            .ok_or_else(|| Error::StateNotFound(state_id_str.to_string()))?;

        let mut transitions_to_check = vec![];
        let mut current_check_id: Option<S> = Some(state_id.clone());
        while let Some(check_id) = current_check_id {
            let check_id_str: &str = &check_id;
            transitions_to_check.extend(self.transitions.iter().filter(|t| t.source == check_id));
            current_check_id = self.states.get(check_id_str).and_then(|s| s.parent.clone());
        }
        transitions_to_check.extend(self.transitions.iter().filter(|t| t.source.id() == "*"));

        let mut enabled_transition: Option<&Transition<S, C, E>> = None;
        for t in transitions_to_check {
            if t.is_enabled(&self.context, event).await {
                let current_depth =
                    enabled_transition.map_or(0, |et| self.get_state_depth(&et.source));
                let new_depth = self.get_state_depth(&t.source);

                if enabled_transition.is_none()
                    || new_depth > current_depth
                    || (new_depth == current_depth && t.source.id() != "*")
                {
                    enabled_transition = Some(t);
                }
            }
        }

        if let Some(transition) = enabled_transition {
            self.execute_transition(&transition.clone(), event).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Execute a transition
    async fn execute_transition(
        &mut self,
        transition: &Transition<S, C, E>,
        event: &E,
    ) -> Result<()> {
        let source_state_obj = self
            .states
            .get(transition.source.id())
            .ok_or_else(|| Error::StateNotFound(transition.source.id().to_string()))?;

        if transition.transition_type == TransitionType::Internal || transition.target.is_none() {
            transition.execute_actions(&mut self.context, event).await?;
            return Ok(());
        }

        let target_state_id = transition.target.as_ref().unwrap();
        let target_state_obj = self
            .states
            .get(target_state_id.id())
            .ok_or_else(|| Error::StateNotFound(target_state_id.id().to_string()))?;

        let lcca = self.find_lcca(&transition.source, target_state_id);

        let mut exit_queue = VecDeque::new();
        let mut current_exit: Option<S> = Some(transition.source.clone());
        while let Some(state_to_exit) = current_exit {
            if Some(state_to_exit.id()) == lcca.as_deref() {
                break;
            }
            exit_queue.push_back(state_to_exit.clone());
            let state_obj = self.states.get(state_to_exit.id()).unwrap();
            current_exit = state_obj.parent.clone();
        }
        while let Some(state_to_exit) = exit_queue.pop_front() {
            self.execute_exit_actions(&state_to_exit, event).await?;
            self.update_history_on_exit(&state_to_exit);
            self.current_states.remove(&state_to_exit);
        }

        transition.execute_actions(&mut self.context, event).await?;

        let mut entry_queue = VecDeque::new();
        let mut current_entry: Option<S> = Some(target_state_id.clone());
        while let Some(state_to_enter) = current_entry {
            if Some(state_to_enter.id()) == lcca.as_deref() {
                break;
            }
            entry_queue.push_front(state_to_enter.clone());
            let state_obj = self.states.get(state_to_enter.id()).unwrap();
            current_entry = state_obj.parent.clone();
        }
        while let Some(state_to_enter) = entry_queue.pop_front() {
            self.enter_state(&state_to_enter, event).await?;
        }

        Ok(())
    }

    /// Enter a state (handle atomic, compound, parallel, history)
    #[async_recursion]
    async fn enter_state(&mut self, state_id: &S, event: &E) -> Result<()> {
        let state_id_str: &str = state_id;
        let state = self
            .states
            .get(state_id_str)
            .ok_or_else(|| Error::StateNotFound(state_id_str.to_string()))?
            .clone();

        self.current_states.insert(state_id.clone());

        self.execute_entry_actions(state_id, event).await?;

        match state.state_type() {
            StateType::Compound => {
                let initial_target_id = state.initial().or_else(|| {
                    state.children().iter().find_map(|child_id| {
                        self.states.get(child_id.id()).and_then(|s| {
                            if s.is_history() {
                                self.history.get(s.id()).cloned().map(S::from)
                            } else {
                                None
                            }
                        })
                    })
                });

                if let Some(initial_id) = initial_target_id {
                    self.enter_state(&initial_id, event).await?;
                } else {
                    return Err(Error::InvalidConfiguration(format!(
                        "Compound state '{}' needs an initial state or a history state with history",
                        state_id_str
                    )));
                }
            }
            StateType::Parallel => {
                for child_id in state.children().to_vec() {
                    self.enter_state(&child_id, event).await?;
                }
            }
            StateType::History | StateType::DeepHistory => {
                self.current_states.remove(state_id);
            }
            StateType::Normal | StateType::Final => {
                // Atomic states, nothing more to enter.
            }
        }

        if let Some(parent_id) = state.parent() {
            if let Some(parent_state) = self.states.get(parent_id) {
                if parent_state.is_compound() {
                    if let Some(history_state) = parent_state
                        .children()
                        .iter()
                        .find_map(|cid| self.states.get(cid.id()).filter(|s| s.is_history()))
                    {
                        let history_type = history_state.history().unwrap_or(HistoryType::Shallow);
                        match history_type {
                            HistoryType::Shallow => {
                                self.history.insert(
                                    history_state.id().to_string(),
                                    state_id_str.to_string(),
                                );
                            }
                            HistoryType::Deep => {
                                if state.is_atomic() {
                                    self.history.insert(
                                        history_state.id().to_string(),
                                        state_id_str.to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Exit a state (handle atomic, compound, parallel)
    #[async_recursion]
    async fn exit_state(&mut self, state_id: &S, event: &E) -> Result<()> {
        let state_id_str: &str = state_id;
        let state = self
            .states
            .get(state_id_str)
            .ok_or_else(|| Error::StateNotFound(state_id_str.to_string()))?
            .clone();

        match state.state_type() {
            StateType::Compound | StateType::Parallel => {
                let active_children_to_exit: Vec<S> = self
                    .current_states
                    .iter()
                    .filter(|active_s| {
                        self.states
                            .get(active_s.id())
                            .map_or(false, |s| s.parent().as_deref() == Some(state_id_str))
                    })
                    .cloned()
                    .collect();
                for child_id in active_children_to_exit {
                    self.exit_state(&child_id, event).await?;
                }
            }
            _ => {}
        }

        self.execute_exit_actions(state_id, event).await?;

        self.current_states.remove(state_id);

        self.update_history_on_exit(state_id);

        Ok(())
    }

    async fn execute_entry_actions(&self, state_id: &S, event: &E) -> Result<(), ActionError> {
        let state_id_str: &str = state_id;
        if let Some(actions) = self.entry_actions.get(state_id_str) {
            for action in actions {
                eprintln!(
                    "Stub: Would execute entry action '{}' for state {}",
                    action.name, state_id_str
                );
            }
        }
        Ok(())
    }

    async fn execute_exit_actions(&self, state_id: &S, event: &E) -> Result<(), ActionError> {
        let state_id_str: &str = state_id;
        if let Some(actions) = self.exit_actions.get(state_id_str) {
            for action in actions {
                eprintln!(
                    "Stub: Would execute exit action '{}' for state {}",
                    action.name, state_id_str
                );
            }
        }
        Ok(())
    }

    fn update_history_on_exit(&mut self, exited_state_id: &S) {
        let exited_state_id_str: &str = exited_state_id;
        if let Some(parent_id) = self
            .states
            .get(exited_state_id_str)
            .and_then(|s| s.parent())
        {
            if let Some(parent_state) = self.states.get(parent_id) {
                if parent_state.is_compound() {
                    if let Some(history_state) = parent_state
                        .children()
                        .iter()
                        .find_map(|cid| self.states.get(cid.id()).filter(|s| s.is_history()))
                    {
                        let history_type = history_state.history().unwrap_or(HistoryType::Shallow);
                        match history_type {
                            HistoryType::Shallow => {
                                self.history.insert(
                                    history_state.id().to_string(),
                                    exited_state_id_str.to_string(),
                                );
                            }
                            HistoryType::Deep => {
                                if self
                                    .states
                                    .get(exited_state_id_str)
                                    .map_or(false, |s| s.is_atomic())
                                {
                                    self.history.insert(
                                        history_state.id().to_string(),
                                        exited_state_id_str.to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn find_lcca(&self, state1_id: &S, state2_id: &S) -> Option<String> {
        let ancestors1 = self.get_ancestors_inclusive(state1_id);
        let ancestors2 = self.get_ancestors_inclusive(state2_id);

        ancestors1
            .into_iter()
            .rev()
            .find(|id1| ancestors2.iter().rev().any(|id2| id1 == id2))
    }

    fn get_ancestors_inclusive(&self, state_id: &S) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current_id = Some(state_id.clone());
        while let Some(id) = current_id {
            let id_str: &str = &id;
            ancestors.push(id_str.to_string());
            current_id = self.states.get(id_str).and_then(|s| s.parent.clone());
        }
        ancestors
    }

    fn get_parent_id(&self, state_id: &S) -> Option<S> {
        let state_id_str: &str = state_id;
        self.states.get(state_id_str).and_then(|s| s.parent.clone())
    }

    pub fn is_in(&self, state_id: &S) -> bool {
        let state_id_str: &str = state_id;
        self.current_states.iter().any(|active_state| {
            let active_state_str: &str = active_state;
            active_state_str == state_id_str
                || self
                    .get_ancestors_inclusive(active_state)
                    .contains(&state_id_str.to_string())
        })
    }

    pub fn to_json(&self) -> Result<String> {
        Err(Error::UnsupportedOperation(
            "Direct serialization not supported, use snapshot".to_string(),
        ))
    }

    fn get_state_depth(&self, state_id: &S) -> usize {
        let state_id_str: &str = state_id;
        let mut depth = 0;
        let mut current_id = self.states.get(state_id_str).and_then(|s| s.parent());
        while let Some(parent_id) = current_id {
            depth += 1;
            current_id = self.states.get(parent_id).and_then(|s| s.parent());
        }
        depth
    }

    fn get_ancestors(&self, state_id: &S) -> Vec<S> {
        let mut ancestors = Vec::new();
        let mut current_id = self.get_parent_id(state_id);
        while let Some(id) = current_id {
            ancestors.push(id.clone());
            current_id = self.get_parent_id(&id);
        }
        ancestors
    }
}

/// Builder for constructing state machines
pub struct MachineBuilder<C = Context, E = Event, S = String, O = ()>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Default,
    S: StateTrait + fmt::Display + Eq + Hash + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states
    pub states: HashMap<String, State<S>>,
    /// Collection of transitions
    pub transitions: Vec<Transition<S, C, E>>,
    /// Initial state id
    pub initial: String,
    /// Context for the machine
    pub context: Option<C>,
    /// Entry actions for states
    pub(crate) entry_actions: HashMap<String, Vec<Action<C, E>>>,
    /// Exit actions for states
    pub(crate) exit_actions: HashMap<String, Vec<Action<C, E>>>,
    /// Type markers
    _phantom_c: PhantomData<C>,
    _phantom_e: PhantomData<E>,
    _phantom_s: PhantomData<S>,
    _phantom_o: PhantomData<O>,
}

impl<C, E, S, O> MachineBuilder<C, E, S, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Default,
    S: StateTrait + fmt::Display + Eq + Hash + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    /// Create a new state machine builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            states: HashMap::new(),
            transitions: Vec::new(),
            initial: String::new(),
            context: None,
            entry_actions: HashMap::new(),
            exit_actions: HashMap::new(),
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
            _phantom_s: PhantomData,
            _phantom_o: PhantomData,
        }
    }

    /// Set the initial state
    pub fn initial(mut self, state_id: impl Into<String>) -> Self {
        self.initial = state_id.into();
        self
    }

    /// Add a state to the machine
    pub fn state(mut self, state: State<S>) -> Self {
        self.states.insert(state.id.to_string(), state);
        self
    }

    /// Add a transition to the machine
    pub fn transition(mut self, transition: Transition<S, C, E>) -> Self {
        self.transitions.push(transition);
        self
    }

    /// Add an entry action to a state
    pub fn on_entry<A: IntoAction<C, E>>(mut self, state_id: impl Into<String>, action: A) -> Self {
        self.entry_actions
            .entry(state_id.into())
            .or_default()
            .push(action.into_action());
        self
    }

    /// Add an exit action to a state
    pub fn on_exit<A: IntoAction<C, E>>(mut self, state_id: impl Into<String>, action: A) -> Self {
        self.exit_actions
            .entry(state_id.into())
            .or_default()
            .push(action.into_action());
        self
    }

    /// Set the context for the machine
    pub fn context(mut self, context: C) -> Self {
        self.context = Some(context);
        self
    }

    /// Build the state machine
    pub async fn build(self) -> Result<Machine<C, E, S, O>> {
        Machine::new(self).await
    }
}

impl<C, E, S, O> Clone for MachineBuilder<C, E, S, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Default,
    S: StateTrait + Clone + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            states: self.states.clone(),
            transitions: self.transitions.clone(),
            initial: self.initial.clone(),
            context: self.context.clone(),
            entry_actions: self.entry_actions.clone(),
            exit_actions: self.exit_actions.clone(),
            _phantom_c: PhantomData,
            _phantom_e: PhantomData,
            _phantom_s: PhantomData,
            _phantom_o: PhantomData,
        }
    }
}

// --- MachineSnapshot ---
// (Definition remains the same as previous step)
#[derive(Clone, Debug, PartialEq)]
pub struct MachineSnapshot<C, S, O = ()>
where
    S: StateTrait + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    inner: ActorSnapshot<C, O>,
    current_states: HashSet<String>,
    history_states: HashMap<String, String>,
    _phantom_s: PhantomData<S>,
}

impl<C, S, O> MachineSnapshot<C, S, O>
where
    S: StateTrait + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    pub fn value(&self) -> &serde_json::Value {
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

    pub fn is_in(&self, state_id: &str) -> bool {
        match &self.inner.value {
            serde_json::Value::String(s) => s == state_id,
            serde_json::Value::Object(map) => {
                // Check if the key exists at the top level or nested
                let mut queue = VecDeque::new();
                queue.push_back(map);

                while let Some(current_map) = queue.pop_front() {
                    if current_map.contains_key(state_id) {
                        return true;
                    }
                    for val in current_map.values() {
                        if let serde_json::Value::Object(nested_map) = val {
                            queue.push_back(nested_map);
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }
}

// --- Core Non-Mutating Transition Logic ---
impl<C, E, S, O> Machine<C, E, S, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Default + From<Event> + Clone + Eq,
    S: StateTrait
        + Display
        + Eq
        + Hash
        + Send
        + Sync
        + 'static
        + Clone
        + From<String>
        + Deref<Target = str>
        + Serialize
        + for<'de> DeserializeOwned,
    O: Clone + Send + Sync + 'static,
{
    async fn step(
        &self,
        current_snapshot: MachineSnapshot<C, S, O>,
        event: E,
    ) -> Result<MachineSnapshot<C, S, O>> {
        if *current_snapshot.status() != ActorStatus::Active {
            return Ok(current_snapshot); // Do not transition if not active
        }

        let active_states = self.get_active_atomic_states(current_snapshot.value());
        let mut next_context = current_snapshot.context().clone();
        let mut next_value = current_snapshot.value().clone();
        let mut next_status = *current_snapshot.status();
        let mut next_output = current_snapshot.inner.output.clone();

        let mut transition_found = false;

        // 1. Find Enabled Transition (considering hierarchy)
        if let Some((transition, source_state_id)) = self
            .select_transition(&active_states, &next_context, &event)
            .await?
        {
            transition_found = true;

            // 2. Determine Exit/Entry Sets
            let (exit_set, entry_set, common_ancestor) =
                self.calculate_transition_sets(&source_state_id, &transition);

            // 3. Execute Exit Actions
            let exit_tasks: Vec<_> = exit_set
                .iter()
                .filter_map(|id| self.exit_actions.get(id))
                .flatten()
                .map(|action| action.execute_borrowed(&mut next_context, &event))
                .collect();
            try_join_all(exit_tasks).await?; // Execute actions concurrently

            // 4. Execute Transition Actions
            let transition_tasks: Vec<_> = transition
                .actions
                .iter()
                .map(|action| action.execute_borrowed(&mut next_context, &event))
                .collect();
            try_join_all(transition_tasks).await?; // Execute actions concurrently

            // 5. Execute Entry Actions
            let entry_tasks: Vec<_> = entry_set
                .iter()
                .filter_map(|id| self.entry_actions.get(id))
                .flatten()
                .map(|action| action.execute_borrowed(&mut next_context, &event))
                .collect();
            try_join_all(entry_tasks).await?; // Execute actions concurrently

            // 6. Compute Next State Value
            next_value = self.compute_next_value(
                &exit_set,
                &entry_set,
                &common_ancestor,
                current_snapshot.value(),
            );

            // 7. Update Status/Output if Final State Entered
            if entry_set.iter().any(|id| {
                self.states
                    .get(id)
                    .map_or(false, |s| s.state_type == StateType::Final)
            }) {
                next_status = ActorStatus::Done;
                // TODO: Determine output based on final state definition or context
                // next_output = Some(...);
            }
        }

        // TODO: Handle "always" transitions if no event transition was taken

        Ok(MachineSnapshot {
            inner: ActorSnapshot {
                value: next_value,
                context: next_context,
                output: next_output,
                status: next_status,
            },
            current_states: active_states,
            history_states: self.history.clone(),
            _phantom_s: PhantomData,
        })
    }

    // --- Helper Functions for Step Logic ---

    /// Finds the highest-priority enabled transition for the current active states.
    /// Returns the transition and the actual state ID that triggered it.
    async fn select_transition(
        &self,
        active_states: &HashSet<String>,
        context: &C,
        event: &E,
    ) -> Result<Option<(Transition<S, C, E>, String)>> {
        let mut candidates = Vec::new();

        for state_id_str in active_states {
            let state_id = S::from(state_id_str.clone());
            let mut current_id_opt = Some(state_id.clone());
            while let Some(current_id) = current_id_opt {
                for t in &self.transitions {
                    if t.source == current_id || t.source.id() == "*" {
                        if t.is_enabled(context, event).await {
                            candidates.push((
                                t.clone(),
                                state_id_str.clone(),
                                self.get_state_depth(&current_id),
                            ));
                        }
                    }
                }
                current_id_opt = self.get_parent_id(&current_id);
            }
        }

        // Sort candidates: specific source first, then by depth (deeper is higher priority)
        candidates.sort_by(|(t1, _, depth1), (t2, _, depth2)| {
            if t1.source == "*" && t2.source != "*" {
                std::cmp::Ordering::Greater
            } else if t1.source != "*" && t2.source == "*" {
                std::cmp::Ordering::Less
            } else {
                depth2.cmp(depth1) // Higher depth means higher priority
            }
        });

        Ok(candidates
            .first()
            .map(|(t, src_id_str, _)| (t.clone(), src_id_str.clone())))
    }

    /// Calculates the set of states to exit, enter, and the common ancestor.
    fn calculate_transition_sets(
        &self,
        source_id_str: &str,
        transition: &Transition<S, C, E>,
    ) -> (HashSet<String>, HashSet<String>, Option<String>) {
        let source_id = S::from(source_id_str.to_string());

        if transition.target.is_none() {
            return (
                HashSet::new(),
                HashSet::new(),
                self.get_parent_id(&source_id).map(|s| s.to_string()),
            );
        }
        let target_id = transition.target.as_ref().unwrap();

        let mut exit_set = HashSet::new();
        let mut entry_set = HashSet::new();

        let source_ancestors = self.get_ancestors(&source_id).iter().map(|s| s.to_string()).collect::<HashSet<_>>();
        let target_ancestors = self.get_ancestors(target_id).iter().map(|s| s.to_string()).collect::<HashSet<_>>();

        // Find LCA (Least Common Ancestor)
        let mut common_ancestor: Option<String> = None;
        let source_ancestors_vec = self.get_ancestors_inclusive(&source_id);
        for ancestor_s in source_ancestors_vec.iter().rev() {
            let ancestor_str = ancestor_s.to_string();
            if target_ancestors.contains(&ancestor_str) {
                common_ancestor = Some(ancestor_str);
                break;
            }
        }

        // Calculate exit set
        let mut current_exit_id = Some(source_id.clone());
        while let Some(id) = current_exit_id {
            let id_str = id.to_string();
            if common_ancestor.as_ref() == Some(&id_str) {
                break;
            }
            exit_set.insert(id_str);
            current_exit_id = self.get_parent_id(&id);
        }

        // Calculate entry set
        let mut entry_path = VecDeque::new();
        let mut current_entry_id = Some(target_id.clone());
        while let Some(id) = current_entry_id {
            let id_str = id.to_string();
            if common_ancestor.as_ref() == Some(&id_str) {
                break;
            }
            entry_path.push_front(id_str);
            current_entry_id = self.get_parent_id(&id);
        }

        for id_str in entry_path {
            entry_set.insert(id_str.clone());
            self.add_initial_descendants(&id_str, &mut entry_set);
        }

        (exit_set, entry_set, common_ancestor)
    }

    /// Recursively adds initial descendant states for compound/parallel states.
    fn add_initial_descendants(&self, state_id_str: &str, entry_set: &mut HashSet<String>) {
        let mut queue = VecDeque::new();
        if let Some(state) = self.states.get(state_id_str) {
            match state.state_type {
                StateType::Compound => {
                    let initial_child_str = self
                        .history
                        .get(state_id_str)
                        .cloned()
                        .or_else(|| state.initial.as_ref().map(|s| s.to_string()));
                    if let Some(child_id) = initial_child_str {
                        if entry_set.insert(child_id.clone()) {
                            queue.push_back(child_id);
                        }
                    }
                }
                StateType::Parallel => {
                    for child_id_str in state.children.keys() {
                        if entry_set.insert(child_id_str.clone()) {
                            queue.push_back(child_id_str.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        while let Some(current_id_str) = queue.pop_front() {
            self.add_initial_descendants(&current_id_str, entry_set);
        }
    }

    /// Computes the next state value JSON based on exits and entries.
    fn compute_next_value(
        &self,
        exit_set: &HashSet<String>,
        entry_set: &HashSet<String>,
        common_ancestor: &Option<String>,
        current_value: &serde_json::Value,
    ) -> serde_json::Value {
        // This is complex. Needs to modify the JSON structure.
        // Simplified placeholder: assumes atomic states only for now.
        if let Some(id) = entry_set
            .iter()
            .find(|id| self.states.get(*id).map_or(false, |s| s.is_atomic()))
        {
            serde_json::Value::String(id.clone())
        } else {
            // Need a robust way to represent parallel/hierarchical states in JSON
            // and update it based on entry/exit sets relative to LCA.
            // For now, return the most specific entered state ID or keep current.
            entry_set.iter().next().map_or_else(
                || current_value.clone(),
                |id| serde_json::Value::String(id.clone()),
            )
        }
    }

    /// Gets all active atomic states from the state value representation.
    fn get_active_atomic_states(&self, value: &serde_json::Value) -> HashSet<String> {
        let mut active_states = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(value);

        while let Some(current_value) = queue.pop_front() {
            match current_value {
                serde_json::Value::String(s) => {
                    if self.states.get(s).map_or(false, |st| st.is_atomic()) {
                        active_states.insert(s.clone());
                    }
                }
                serde_json::Value::Object(map) => {
                    for val in map.values() {
                        queue.push_back(val);
                    }
                }
                _ => {}
            }
        }
        active_states
    }

    /// Get the depth of a state node (root = 0).
    fn get_state_depth(&self, state_id: &S) -> usize {
        self.get_ancestors(state_id).len().saturating_sub(1)
    }
}

// Implement ActorLogic for Machine
#[async_trait::async_trait]
impl<C, E, S, O> ActorLogic<MachineSnapshot<C, S, O>, E> for Machine<C, E, S, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventTrait + Send + Sync + 'static + Default + PartialEq + From<Event> + Clone,
    S: StateTrait + fmt::Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + Deref<Target = str> + Serialize + for<'de> DeserializeOwned,
    O: Clone + Send + Sync + 'static,
{
    fn get_initial_snapshot(&self, _input: Option<()>) -> MachineSnapshot<C, S, O> {
        let mut initial_states = HashSet::new();
        let initial_state_str = self.initial.clone();
        initial_states.insert(initial_state_str.clone());
        // self.add_initial_descendants(&initial_state_str, &mut initial_states); // Can't call methods needing E: From<Event>

        MachineSnapshot::new(
            ActorSnapshot::new(
                self.context.clone(),
                None, // output
                ActorStatus::Active,
                serde_json::Value::String(initial_state_str), // initial value
            ),
            initial_states,
            self.history.clone(),
        )
    }

    #[async_recursion]
    async fn transition(
        &self,
        mut snapshot: MachineSnapshot<C, S, O>,
        event: E,
    ) -> Result<MachineSnapshot<C, S, O>> {
        // Simplified transition logic - find and execute ONE transition
        let current_states_ids: Vec<_> = snapshot.current_state_ids().iter().cloned().collect();
        let mut transition_to_execute: Option<&Transition<S, C, E>> = None;

        // Find applicable transition (simplified: first one found)
        'outer: for state_id in &current_states_ids {
            let mut current_check = Some(state_id.as_str());
            while let Some(check_id) = current_check {
                for t in &self.transitions {
                    if t.source == check_id || t.source == "*" {
                        if t.is_enabled(snapshot.context(), &event).await {
                            transition_to_execute = Some(t);
                            break 'outer;
                        }
                    }
                }
                current_check = self.get_parent_id(check_id).as_deref();
            }
        }

        if let Some(transition) = transition_to_execute {
            // Re-implement execute_transition logic here or call helper
            let source_id = if transition.source == "*" {
                current_states_ids.first().cloned().ok_or_else(|| {
                    Error::InvalidConfiguration(
                        "No current state in snapshot for wildcard transition".into(),
                    )
                })?
            } else {
                transition.source.clone()
            };

            if transition.target.is_none() {
                // Internal transition
                transition
                    .execute_actions(snapshot.context_mut(), &event)
                    .await;
            } else {
                // External transition
                let target_id = transition.target.as_ref().unwrap().clone();
                let (exit_set, entry_set, _common_ancestor) =
                    self.calculate_transition_sets(&source_id, transition);

                // Execute exit actions
                for exit_id in exit_set {
                    if let Some(actions) = self.exit_actions.get(&exit_id) {
                        for action in actions {
                            action.execute(snapshot.context_mut(), &event).await;
                        }
                    }
                    snapshot.current_states.remove(&exit_id);
                }

                // Execute transition actions
                transition
                    .execute_actions(snapshot.context_mut(), &event)
                    .await;

                // Execute entry actions and update snapshot
                for entry_id in entry_set {
                    if let Some(actions) = self.entry_actions.get(&entry_id) {
                        for action in actions {
                            action.execute(snapshot.context_mut(), &event).await;
                        }
                    }
                    snapshot.current_states.insert(entry_id.clone());
                    // Add descendants for compound/parallel
                    self.add_initial_descendants(&entry_id, &mut snapshot.current_states);
                }
                snapshot.update_history(&self.states);
            }
        } else {
            // Event not handled
            return Err(Error::EventNotHandled(event.event_type().to_string()));
        }

        Ok(snapshot)
    }
}
