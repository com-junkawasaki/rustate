use crate::actor::Snapshot;
use crate::event::IntoEvent;
use crate::{
    action::ActionType,
    actor::{ActorLogic, ActorStatus, Snapshot as ActorSnapshot},
    error::StateError,
    state::{HistoryType, State, StateCollection, StateType},
    transition::TransitionType,
    Action, Context, Error, Event, EventTrait, IntoAction, Result, StateTrait, Transition,
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

/// Represents a state machine instance
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "S: Serialize, C: Serialize",
    deserialize = "S: StateTrait, C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + Debug + 'static"
))]
pub struct Machine<C = Context, E = Event, S = String, O = ()>
where
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Clone
        + Eq
        + Serialize
        + DeserializeOwned
        + Debug
        + IntoEvent,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + Debug,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states (Use StateCollection for better management)
    #[serde(flatten)]
    pub states: StateCollection<S>,
    /// Collection of transitions (Grouped by source state ID)
    pub transitions: HashMap<S, Vec<Transition<S, C, E>>>,
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
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Clone
        + Eq
        + Serialize
        + DeserializeOwned
        + Debug
        + IntoEvent,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + Debug,
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

        // --- Group Transitions by Source State --- Start
        let mut grouped_transitions: HashMap<S, Vec<Transition<S, C, E>>> = HashMap::new();
        for t in transitions {
            // Validate source and target states
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
            // Group the transition
            grouped_transitions.entry(t.source.clone()).or_default().push(t);
        }
        // --- Group Transitions by Source State --- End

        if states.is_empty() {
            return Err(Error::InvalidConfiguration("No states defined".into()));
        }

        if !states.contains(&initial) {
            return Err(Error::StateNotFound(initial.to_string()));
        }

        let initial_context = context.unwrap_or_default();

        let mut final_entry_actions = entry_actions;
        let mut final_exit_actions = exit_actions;
        for state in states.all() {
            let id_str = state.id.to_string();
            if !state.entry.is_empty() {
                final_entry_actions
                    .entry(id_str.clone())
                    .or_default()
                    .extend(state.entry.iter().map(|ia| ia.into_action()));
            }
            if !state.exit.is_empty() {
                final_exit_actions
                    .entry(id_str)
                    .or_default()
                    .extend(state.exit.iter().map(|ia| ia.into_action()));
            }
        }

        let mut machine = Self {
            name,
            states,
            transitions: grouped_transitions,
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

        // Clone necessary data before potential mutable borrows
        let current_state_ids: Vec<S> = self.current_states.iter().cloned().collect();
        let current_context = self.context.clone(); // Clone context for read-only checks

        let mut enabled_transition: Option<(&Transition<S, C, E>, S)> = None; // Store source_id clone

        // Find transitions that match the current state and event
        for state_id in &current_state_ids {
            if let Some(state_transitions) = self.transitions.get(state_id) {
                for t in state_transitions {
                    // Check event match first
                    let event_matches = t
                        .event
                        .as_ref()
                        .map_or(false, |te| te.name() == event_ref.name());
                    if event_matches {
                        // Check guard condition if event matches
                        if t.is_enabled(&current_context, event_ref).await {
                            enabled_transition = Some((t, state_id.clone())); // Clone state_id
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
                let mut current_check_id = current_state_id.clone();
                loop {
                    if let Some(state_transitions) = self.transitions.get(&current_check_id) {
                        for t in state_transitions {
                            if t.event.is_none() {
                                // Check for transitions with no event specified
                                if t.is_enabled(&self.context, event_ref).await {
                                    enabled_transition = Some((t, current_state_id.clone())); // Clone state_id
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(parent_id) = self.states.get(current_check_id).and_then(|s| s.parent.clone()) {
                        current_check_id = parent_id;
                    } else {
                        break;
                    }
                }
            }
        }

        for t in &self.transitions {
            if t.source.to_string() == "*"
                && t.event == event_ref.name()
                && t.is_enabled(&self.context, event_ref).await
            {
                if enabled_transition.is_none() {
                    // Found a wildcard transition
                    // TODO: Determine the correct source state for wildcard transitions if needed.
                    // For now, let's assume it applies generally and pick the first current state.
                    if let Some(first_current_state) = current_state_ids.first() {
                        enabled_transition = Some((t, first_current_state.clone()));
                    } else {
                        // This case should ideally not happen if the machine is initialized
                        return Err(Error::InvalidState("No current state to apply wildcard transition".to_string()));
                    }
                }
            }
        }

        if let Some((transition, source_state_id)) = enabled_transition {
            self.execute_transition(&transition.clone(), &source_state_id, event_ref)
                .await?;
            processed = true;
        } else {
            // Check for wildcard transitions if no specific transition was found
            for t in &self.transitions {
                // Check if source is wildcard "*"
                if t.source.to_string() == "*" {
                    // Check event match
                    let event_matches = t
                        .event
                        .as_ref()
                        .map_or(false, |te| te.name() == event_ref.name());
                    if event_matches {
                        // Check guard
                        if t.is_enabled(&self.context, event_ref).await {
                            // Execute wildcard transition (source state doesn't matter here)
                            // Need a representative source state ID for history, maybe initial?
                            self.execute_transition(&t.clone(), &self.initial.clone(), event_ref)
                                .await?;
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
        // Ensure the state exists before proceeding
        let state_exists = self.states.contains(state_id);
        if !state_exists {
            return Err(Error::StateNotFound(state_id.to_string()));
        }

        // Add state to current states
        self.current_states.insert(state_id.clone());

        // Execute entry actions for this state
        self.execute_entry_actions(state_id, event).await?;

        // --- Handle entering child states --- Start
        // Clone necessary info to avoid borrowing issues
        let state_info = match self.states.get(state_id) {
            Some(s) => Some((s.state_type.clone(), s.initial.clone(), s.history.clone(), s.children.keys().cloned().collect::<Vec<_>>())), // Clone required fields
            None => None, // Should not happen due to check above, but handle defensively
        };

        if let Some((state_type, initial_child_opt, history_type_opt, child_keys)) = state_info {
            match state_type {
                StateType::Compound => {
                    // Determine the child state to enter
                    let child_to_enter: Option<S> = if let Some(history_type) = history_type_opt {
                        let state_id_str = state_id.to_string(); // Use Display impl of S
                        match history_type {
                            // Shallow history: Use last active direct child
                            HistoryType::Shallow => self.history.get(&state_id_str).cloned(), // Clone the value
                            // Deep history: TODO: Implement deep history logic (needs tracking nested history)
                            HistoryType::Deep => {
                                // For now, fallback to initial if deep history not found or implemented
                                self.history.get(&state_id_str).cloned().or(initial_child_opt.clone()) // Clone initial
                            }
                        }.or(initial_child_opt.clone()) // Fallback to initial if history is empty
                    } else {
                        initial_child_opt.clone() // Use the defined initial state if no history
                    };

                    if let Some(child_id) = child_to_enter {
                        self.enter_state(&child_id, event).await?; // Recurse into child
                    }
                }
                StateType::Parallel => {
                    // Enter all child states in parallel
                    let mut enter_futures = Vec::new();
                    // Iterate over cloned keys to avoid borrowing self.states within the loop
                    for child_key in child_keys {
                        // Assuming child_key (String) can be converted back to S
                        // This might need adjustment based on how S is defined and used as HashMap key
                        let child_id = S::from(child_key); // Use From<String> bound
                        enter_futures.push(self.enter_state(&child_id, event));
                    }
                    try_join_all(enter_futures).await?;
                }
                _ => {} // Normal, Final states have no children to enter
            }
        }
        // --- Handle entering child states --- End

        Ok(())
    }

    /// Exit a state and its active children recursively
    #[async_recursion]
    async fn exit_state(&mut self, state_id: &S, event: &E) -> Result<()> {
        // --- Handle exiting child states first (recursion/iteration needed) --- Start
        // Clone necessary info to avoid borrowing issues
        let children_to_exit = self.current_states.iter()
            .filter_map(|current_id| {
                // Check if current_id is a descendant of state_id
                // This requires traversing the parent links up from current_id
                let mut parent_opt = self.states.get(current_id).and_then(|s| s.parent.clone());
                while let Some(p_id) = parent_opt {
                    if &p_id == state_id {
                        return Some(current_id.clone()); // Found a child to exit
                    }
                    parent_opt = self.states.get(&p_id).and_then(|s| s.parent.clone());
                }
                None
            })
            .collect::<Vec<_>>();

        for child_id in children_to_exit {
            self.exit_state(&child_id, event).await?; // Recurse into children
        }
        // --- Handle exiting child states first --- End

        // Ensure state still exists after potential child exits
        if !self.states.contains(state_id) {
            // Might have been removed if it was a child of a previously exited state
            // This logic might need refinement depending on desired behavior.
            return Ok(()); // Or potentially an error/warning
        }

        // Update history before executing exit actions and removing state
        self.update_history_on_exit(state_id);

        // Execute exit actions for this state
        self.execute_exit_actions(state_id, event).await?;

        // Remove state from current states
        self.current_states.remove(state_id);

        Ok(())
    }

    /// Execute entry actions for a state
    async fn execute_entry_actions(&self, state_id: &S, event: &E) -> Result<(), Error> {
        let id_str = state_id.to_string();
        // Clone actions to avoid borrowing `self` mutably while iterating
        if let Some(actions) = self.entry_actions.get(&id_str).cloned() {
            for action in actions {
                // Execute action with mutable access to context
                action.execute(&mut self.context, event).await?;
            }
        }
        Ok(())
    }

    /// Execute exit actions for a state
    async fn execute_exit_actions(&self, state_id: &S, event: &E) -> Result<(), Error> {
        let id_str = state_id.to_string();
        // Clone actions to avoid borrowing `self` mutably while iterating
        if let Some(actions) = self.exit_actions.get(&id_str).cloned() {
            for action in actions {
                // Execute action with mutable access to context
                action.execute(&mut self.context, event).await?;
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
                // Convert String back to S (since S: From<String>)
                // and then borrow to pass &S to states.get()
                let ancestor_id_s = S::from(ancestor_id_str.clone());
                if let Some(state) = self.states.get(&ancestor_id_s) {
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

        while let Some(state) = self.states.get(&current_id) {
            // Use public get method
            if let Some(parent_id) = &state.parent {
                // Access parent field
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
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Default + Eq + Serialize + DeserializeOwned + Debug + IntoEvent,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + Debug,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states (Use StateCollection for better management)
    pub states: StateCollection<S>,
    /// Collection of transitions (Will be grouped in Machine::new)
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
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    E: EventTrait + Send + Sync + 'static + Clone + Default + Eq + Serialize + DeserializeOwned + Debug + IntoEvent,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + Debug,
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
#[serde(bound(
    serialize = "S: Serialize, C: Serialize, O: Serialize",
    deserialize = "S: StateTrait, C: DeserializeOwned, O: DeserializeOwned"
))]
pub struct MachineSnapshot<C, S, O>
where
    C: Clone + Serialize + DeserializeOwned + Send + Sync + 'static + Debug,
    S: StateTrait + Send + Sync + 'static,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Debug,
{
    #[serde(flatten)]
    pub inner: ActorSnapshot<C, O>,
    pub current_states: HashSet<S>,
    pub history_states: HashMap<S, HashSet<S>>,
    _phantom_s: PhantomData<S>,
}

impl<C, S, O> MachineSnapshot<C, S, O>
where
    C: Clone + Serialize + DeserializeOwned + Send + Sync + 'static + Debug,
    S: StateTrait + Send + Sync + 'static,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Debug,
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
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Clone
        + Default
        + Eq
        + Serialize
        + DeserializeOwned
        + Debug
        + IntoEvent,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String>,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + Debug,
{
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
}
