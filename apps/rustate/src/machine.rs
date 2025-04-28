use crate::{
    action::{Action, IntoAction},
    context::Context,
    error::{Result as StateResult, StateError},
    event::{Event, EventTrait, IntoEvent},
    state::{State, StateCollection, StateTrait, StateType, HistoryType},
    transition::{Transition, TransitionType},
};
use crate::{Actor, ActorError};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use log;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_recursion::async_recursion;

/// Define the serializable state structure
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(bound = "S: DeserializeOwned, C: DeserializeOwned")]
struct SerializableMachineState<S, C>
where
    S: StateTrait + Serialize + DeserializeOwned + Clone + Eq + Hash,
    C: Default + Clone + Debug + Serialize + DeserializeOwned,
{
    current_states: HashSet<S>,
    context: C,
    history: HashMap<String, S>,
}

/// Represents the state machine configuration and runtime.
#[derive(Clone, Debug, Serialize)]
pub struct Machine<C = Context, E = Event, S = String, O = ()>
where
    C: Send + Sync + 'static + Default + Clone + Debug + Serialize + DeserializeOwned,
    E: EventTrait + Serialize + DeserializeOwned + fmt::Debug + Clone + Send + Sync + Eq + Hash + IntoEvent + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq + Serialize + DeserializeOwned,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states (Use StateCollection with correct order)
    #[serde(flatten)]
    pub states: StateCollection<S, C, E>,
    /// Collection of transitions (Grouped by source state ID)
    #[serde(bound(serialize = "S: Serialize"))]
    pub transitions: HashMap<S, Vec<Transition<S, C, E>>>,
    /// Initial state id
    pub initial: Option<S>,
    /// Current active state IDs
    #[serde(bound(serialize = "S: Serialize"))]
    pub current_states: HashSet<S>,
    /// Current context data wrapped in Arc<RwLock>
    #[serde(skip)]
    pub context: Arc<RwLock<C>>,
    /// History states mapping (state id -> last active child)
    #[serde(bound(serialize = "S: Serialize"), default)]
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
    E: EventTrait + Serialize + DeserializeOwned + fmt::Debug + Clone + Send + Sync + Eq + Hash + IntoEvent + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq + Serialize + DeserializeOwned,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
{
    /// Create a new state machine instance from a builder
    pub async fn new(builder: MachineBuilder<C, E, S, O>) -> StateResult<Self> {
        let MachineBuilder {
            name,
            mut states,
            transitions,
            initial,
            entry_actions,
            exit_actions,
            context_opt,
            _phantom_e: _,
            _phantom_o: _,
        } = builder;

        // --- Group Transitions by Source State --- Start
        let mut grouped_transitions: HashMap<S, Vec<Transition<S, C, E>>> = HashMap::new();
        for t in transitions {
            // Validate source and target states using the mutable states collection
            if !states.contains(&t.source) {
                return Err(StateError::StateNotFound(format!(
                    "Transition source '{}' not found",
                    t.source
                )));
            }
            if let Some(target) = &t.target {
                if !states.contains(target) {
                    return Err(StateError::StateNotFound(format!(
                        "Transition target '{}' not found",
                        target
                    )));
                }
            }
            // Group the transition
            grouped_transitions
                .entry(t.source.clone())
                .or_default()
                .push(t);
        }
        // --- Group Transitions by Source State --- End

        if states.is_empty() {
            return Err(StateError::InvalidConfiguration("No states defined".into()));
        }

        if !states.contains(&initial) {
            return Err(StateError::StateNotFound(initial.to_string()));
        }

        // Wrap context in Arc<RwLock>
        let initial_context = context_opt.unwrap_or_default();
        let context_rw = Arc::new(RwLock::new(initial_context));

        let mut final_entry_actions = entry_actions;
        let mut final_exit_actions = exit_actions;
        for state in states.all() {
            let id_str = state.id.to_string();
            if !state.entry.is_empty() {
                final_entry_actions
                    .entry(id_str.clone())
                    .or_default()
                    .extend(state.entry.iter().cloned());
            }
            if !state.exit.is_empty() {
                final_exit_actions
                    .entry(id_str)
                    .or_default()
                    .extend(state.exit.iter().cloned());
            }
        }

        let mut machine = Self {
            name,
            states,
            transitions: grouped_transitions,
            initial: Some(initial.clone()),
            entry_actions: final_entry_actions,
            exit_actions: final_exit_actions,
            history: HashMap::new(),
            _phantom_e: PhantomData,
            _phantom_o: PhantomData,
            current_states: HashSet::new(),
            context: context_rw,
        };

        // Fix State parent links *before* initialization
        let state_ids: Vec<S> = machine.states.all().map(|s| s.id.clone()).collect();
        for state_id in state_ids {
            let children_ids: Vec<String> = machine.states.get(&state_id).map_or(Vec::new(), |s| s.children.keys().cloned().collect());
            for child_key in children_ids {
                let child_id = S::from(child_key);
                if let Some(child_state) = machine.states.get_mut(&child_id) {
                    child_state.parent = Some(state_id.clone());
                }
            }
        }

        machine.initialize(&initial).await?;

        Ok(machine)
    }

    /// Initialize the machine by entering the initial state
    async fn initialize(&mut self, initial_state_id: &S) -> StateResult<()> {
        // Clear current states before initialization
        self.current_states.clear();
        self.enter_state(initial_state_id, None, self.context.clone())
            .await?;
        Ok(())
    }

    /// Send an event to the machine
    #[tracing::instrument(skip(self, event), fields(machine_id = %self.name, event = ?event))]
    pub async fn send(&mut self, event: E) -> StateResult<bool> {
        // Log entry and initial state
        log::debug!(
            "Machine '{}' received event: {:?}. Current states: {:?}",
            self.name,
            event,
            self.current_states
        );

        let event = event.clone();
        let mut executed = false;
        let mut valid_transitions = Vec::new();

        // Read context once using Arc<RwLock>
        let current_context_locked = self.context.read().await;
        let current_context_cloned = (*current_context_locked).clone();
        drop(current_context_locked);

        // --- Find Valid Transitions --- Start
        let current_state_ids_before_send = self.current_states.clone();

        for state_id in current_state_ids_before_send.iter() {
            if let Some(state_transitions) = self.transitions.get(state_id) {
                let stream = stream::iter(state_transitions)
                    .filter(|t| futures::future::ready(t.matches_event(&event)))
                    .then(|t| {
                        let context_clone = current_context_cloned.clone();
                        let event_clone = event.clone();
                        async move {
                            if t.is_enabled(&context_clone, &event_clone).await {
                                Some(t.clone())
                            } else {
                                None
                            }
                        }
                    })
                    .filter_map(|t| futures::future::ready(t));
                valid_transitions.extend(stream.collect::<Vec<_>>().await);
            }
        }
        // --- Find Valid Transitions --- End

        // --- Execute First Valid Transition --- Start
        if let Some(transition_to_execute) = valid_transitions.first() {
            log::debug!(
                "Found valid transition: {:?}. Attempting to execute actions.",
                transition_to_execute
            );

            // 1. Execute Actions
            let action_result = self
                .execute_transition_actions(transition_to_execute, &event)
                .await;
            log::debug!(
                "Action execution result for transition {:?}: {:?}",
                transition_to_execute,
                action_result
            );
            action_result?; // Propagate error

            // 2. Perform State Changes if External
            if transition_to_execute.transition_type == TransitionType::External {
                self.execute_transition(
                    transition_to_execute,
                    &current_state_ids_before_send,
                    &event,
                )
                .await?;
            }
            executed = true;
        }

        Ok(executed)
    }

    /// Execute a transition
    #[tracing::instrument(skip(self, transition, current_state_ids, event), fields(transition = ?transition))]
    async fn execute_transition(
        &mut self,
        transition: &Transition<S, C, E>,
        current_state_ids: &HashSet<S>,
        event: &E,
    ) -> StateResult<()> {
        let target_states = match &transition.target {
            Some(target) => {
                if !self.states.contains(target) {
                    return Err(StateError::StateNotFound(target.to_string()));
                }
                Some(target.clone())
            }
            None => None, // Targetless transition
        };

        // 1. Find the LCCA (Least Common Compound Ancestor)
        // Source is not optional
        let source_state_id = &transition.source;
        // Clone the result of find_lcca to avoid holding the borrow
        let lcca_id = target_states
            .as_ref()
            .and_then(|target_id| self.find_lcca(source_state_id, target_id))
            .map(|s| s.clone()); // Correctly clone the value inside Option

        // 2. Determine states to exit
        let mut exit_states = HashSet::new();
        for current_id in current_state_ids {
            // Use cloned lcca_id
            if let Some(ref lcca) = lcca_id {
                // Borrow lcca_id here
                if self.is_descendant(current_id, lcca) && current_id != lcca {
                    // For external transitions, exit source even if it's the LCCA?
                    // Let's stick to exiting only proper descendants for now.
                    // External transition handling might need finer logic here based on source/target relationship to LCCA.
                    if transition.transition_type == TransitionType::External
                        || self.is_descendant(current_id, source_state_id)
                    {
                        exit_states.insert(current_id.clone());
                    }
                } else if transition.transition_type == TransitionType::External
                    && current_id == lcca
                    // Borrow target_states instead of unwrap
                    && target_states.as_ref().is_some_and(|ts| self.is_ancestor(ts, lcca))
                {
                    // Special case for external transition where LCCA is the source state
                    exit_states.insert(current_id.clone());
                }
            } else {
                // If no LCCA (e.g., targetless or root transition), exit all descendants of the source.
                if self.is_descendant(current_id, source_state_id) {
                    exit_states.insert(current_id.clone());
                }
            }
        }

        // Refine exit states based on transition type (especially for external)
        if transition.transition_type == TransitionType::External {
            let source = &transition.source;
            // Use cloned lcca_id
            if exit_states.contains(source)
                || (lcca_id.as_ref() == Some(source) && target_states.is_some())
            {
                exit_states.insert(source.clone());
            } else if lcca_id
                .as_ref()
                .map_or(false, |lcca| self.is_ancestor(source, lcca))
            {
                exit_states.insert(source.clone());
            }
        }

        // For external transitions, ensure the source state itself is exited
        if transition.transition_type == TransitionType::External {
            exit_states.insert(transition.source.clone());
        }

        // 3. Execute exit actions and recursive exit - Execute sequentially to avoid Send issues
        let context_clone_for_exit = self.context.clone(); // Clone Arc for exit states
        for id in &exit_states {
            // Execute sequentially instead of using join_all + boxed()
            self.exit_state(id, event, context_clone_for_exit.clone())
                .await?;
            // Propagate errors immediately
        }

        // 3.5 Update history for exited states *after* exiting
        for id in &exit_states {
            self.update_history_on_exit(id);
        }

        // 4. Execute transition actions (if any)
        if !transition.actions.is_empty() {
            log::debug!("Executing actions for transition: {:?}", transition);
            // Clone Arc for the loop
            let context_arc = self.context.clone();
            for action in &transition.actions {
                // Clone Arc *inside* the loop for the async move block
                let context_clone_for_action = Arc::clone(&context_arc);
                let action = action.clone();
                let event_clone = event.clone(); // Clone event for the async block
                let fut = async move {
                    action.execute(context_clone_for_action, &event_clone).await
                    // Pass Arc clone and event clone
                };
                // Execute actions sequentially for now. TODO: Consider concurrent execution?
                // Handle the result using ?
                fut.await?;
            }
        }

        // 5. Determine states to enter
        let mut enter_states = HashSet::new();
        // Borrow target_states instead of moving
        if let Some(ref target_id) = target_states {
            let mut current = Some(target_id.clone());
            while let Some(id) = current {
                // Use cloned lcca_id
                if let Some(ref lcca) = lcca_id {
                    // Borrow lcca_id here
                    if &id == lcca {
                        // If it's an internal transition and target is LCCA itself, don't enter it again.
                        // If target is a descendant of LCCA, LCCA should not be entered.
                        if transition.transition_type == TransitionType::Internal
                            && target_id == lcca
                        // Compare references
                        {
                            // Don't enter LCCA on internal transition targeting LCCA
                        } else {
                            break;
                        }
                    }
                }
                enter_states.insert(id.clone());
                current = self.get_parent_id(&id);
            }

            // For external transitions, if LCCA was exited, it needs to be re-entered if it's an ancestor of the target
            if transition.transition_type == TransitionType::External {
                // Use cloned lcca_id
                if let Some(ref lcca) = lcca_id {
                    if exit_states.contains(lcca) && self.is_ancestor(target_id, lcca) {
                        enter_states.insert(lcca.clone());
                    }
                }
            }
        }

        // 7. Update current_states
        // Remove exited states (important: use the final exit_states set)
        self.current_states.retain(|id| !exit_states.contains(id));
        // Add entered states
        self.current_states.extend(enter_states);

        // If targetless transition, current_states remains modified by exits only.

        Ok(())
    }

    /// Enter a state and its children recursively
    #[async_recursion::async_recursion]
    async fn enter_state(
        &mut self,
        state_id: &S,
        event: Option<&E>,
        context: Arc<RwLock<C>>,
    ) -> StateResult<()> {
        log::debug!("Entering state: {}", state_id);
        if self.current_states.contains(state_id) {
            log::warn!("Attempted to re-enter active state: {}", state_id);
            return Ok(());
        }

        self.current_states.insert(state_id.clone());

        self.execute_entry_actions(state_id, event, context.clone())
            .await?;

        let state_opt = self.states.get(state_id).cloned();

        if let Some(state) = state_opt {
            match state.state_type {
                StateType::Compound => {
                    let history_val = self.history.get(state_id.to_string().as_str()).cloned();
                    let initial_child_id = if state.history.is_some() {
                        history_val
                    } else {
                        None
                    };
                    let child_to_enter = initial_child_id.or(state.initial);

                    if let Some(child_id) = child_to_enter {
                        // Recursive call - okay as state/history are cloned/owned
                        self.enter_state(&child_id, event, context.clone()).await?;
                    } else {
                        log::warn!(
                            "Compound state {} has no initial child or history value",
                            state_id
                        );
                    }
                }
                StateType::Parallel => {
                    let child_ids: Vec<S> = state.children.keys().cloned().map(S::from).collect();
                    for child_id in child_ids {
                        self.enter_state(&child_id, event, context.clone()).await?;
                    }
                }
                _ => {} // Atomic or Final
            }
        } else {
            return Err(StateError::StateNotFound(format!(
                "State definition not found for {} during enter_state",
                state_id
            )));
        }

        Ok(())
    }

    /// Exit a state and its active children recursively
    #[tracing::instrument(skip(self, event, context), fields(state = %state_id))]
    async fn exit_state(
        &mut self,
        state_id: &S,
        event: &E,
        context: Arc<RwLock<C>>,
    ) -> StateResult<()> {
        log::debug!("Exiting state: {}", state_id);

        // Remove state from current states FIRST
        self.current_states.remove(state_id);

        // Update history before executing exit actions
        self.update_history_on_exit(state_id);

        // Execute exit actions
        self.execute_exit_actions(state_id, event, context.clone())
            .await?;

        // Handle exiting children implicitly (no recursive call needed here)
        // Children should be removed from current_states when their parent exits.
        // This might require adjustment in how current_states is managed during transitions.

        Ok(())
    }

    /// Execute entry actions for a state
    async fn execute_entry_actions(
        &self,
        state_id: &S,
        event: Option<&E>,
        context: Arc<RwLock<C>>,
    ) -> StateResult<()> {
        if let Some(actions) = self.entry_actions.get(&state_id.to_string()) {
            // Use E::default() which is now available due to the trait bound
            let actual_event = event.cloned().unwrap_or_else(E::default);
            for action in actions {
                // Handle the result using ?
                action.execute(context.clone(), &actual_event).await?; // Pass &E
            }
        }
        Ok(())
    }

    /// Execute exit actions for a state
    async fn execute_exit_actions(
        &self,
        state_id: &S,
        event: &E, // Exit actions always have an event context
        context: Arc<RwLock<C>>,
    ) -> StateResult<()> {
        if let Some(actions) = self.exit_actions.get(&state_id.to_string()) {
            for action in actions {
                // Handle the result using ?
                action.execute(context.clone(), event).await?;
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

    /// Find the Least Common Compound Ancestor (LCCA) of two states.
    /// Returns Option<S> (owned) instead of Option<&S>.
    fn find_lcca(&self, state1_id: &S, state2_id: &S) -> Option<S> {
        let mut path1 = HashSet::new();
        let mut current = Some(state1_id.clone()); // Clone start ID
        while let Some(id) = current {
            path1.insert(id.clone()); // Insert cloned ID
            current = self.get_parent_id(&id); // get_parent_id returns owned Option<S>
        }

        let mut current = Some(state2_id.clone()); // Clone start ID
        while let Some(id) = current {
            if path1.contains(&id) {
                return Some(id); // Return the owned ID found
            }
            current = self.get_parent_id(&id);
        }

        None
    }

    /// Get the parent state ID for a given state ID
    fn get_parent_id(&self, state_id: &S) -> Option<S> {
        // Access the states map using the StateCollection helper
        self.states
            .get(state_id)
            .and_then(|state| state.parent.clone())
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
    #[allow(dead_code)]
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
    pub fn to_json(&self) -> StateResult<String> {
        // Changed Result to StateResult
        serde_json::to_string_pretty(self).map_err(|e| StateError::Serialization(e.to_string()))
        // Use correct variant name
    }

    /// Get the depth of a state in the hierarchy.
    #[allow(dead_code)]
    fn get_state_depth(&self, state_id: &S) -> usize {
        let mut depth = 0;
        let mut current = state_id.clone();
        while let Some(parent_id_str) = self.states.get(&current).and_then(|s| s.parent()) {
            let parent_id = S::from(parent_id_str.to_string()); // Convert &str to String before From
            current = parent_id;
            depth += 1;
        }
        depth
    }

    /// Check if `descendant_id` is a descendant of `ancestor_id`.
    fn is_descendant(&self, descendant_id: &S, ancestor_id: &S) -> bool {
        if descendant_id == ancestor_id {
            return false; // Not a *proper* descendant
        }
        let mut current = self.get_parent_id(descendant_id);
        while let Some(id) = current {
            if &id == ancestor_id {
                return true;
            }
            current = self.get_parent_id(&id);
        }
        false
    }

    /// Check if `ancestor_id` is an ancestor of `descendant_id`.
    fn is_ancestor(&self, ancestor_id: &S, descendant_id: &S) -> bool {
        self.is_descendant(descendant_id, ancestor_id)
    }

    /// Serializes the current *state* (active states, context, history) to JSON.
    /// Does not include the machine definition (states/transitions).
    pub fn serialize_state(&self) -> StateResult<String> {
        // Read context asynchronously
        let context_guard = futures::executor::block_on(self.context.read());
        let serializable_state = SerializableMachineState {
            current_states: self.current_states.clone(),
            context: (*context_guard).clone(),
            history: self.history.clone(),
        };
        serde_json::to_string(&serializable_state)
            .map_err(|e| StateError::Serialization(e.to_string())) // Use correct variant name
    }

    /// Processes transitions based on the current state and event.
    #[allow(dead_code)]
    async fn process_transitions(
        &mut self,
        event: E,
        current_state_ids: &HashSet<S>,
        context: Arc<RwLock<C>>,
    ) -> StateResult<()> {
        let mut valid_transitions = Vec::new();

        // Read context once using Arc<RwLock>
        let current_context_locked = context.read().await;
        let current_context_cloned = (*current_context_locked).clone(); // Clone the inner C
        drop(current_context_locked); // Release read lock quickly

        for state_id in current_state_ids.iter() {
            // Find direct transitions
            if let Some(state_transitions) = self.transitions.get(state_id) {
                let stream = stream::iter(state_transitions)
                    .filter(|t| futures::future::ready(t.matches_event(&event)))
                    .then(|t| {
                        let context_clone = current_context_cloned.clone(); // Use cloned C
                        let event_clone = event.clone();
                        async move {
                            if t.is_enabled(&context_clone, &event_clone).await {
                                Some(t.clone())
                            } else {
                                None
                            }
                        }
                    })
                    .filter_map(|t| futures::future::ready(t));
                valid_transitions.extend(stream.collect::<Vec<_>>().await);
            }
            // Find wildcard transitions for the state
            if let Some(wildcard_transitions) = self.transitions.get(&S::from("*".to_string())) {
                let stream = stream::iter(wildcard_transitions)
                    .then(|t| {
                        let context_clone = current_context_cloned.clone(); // Use cloned C
                        let event_clone = event.clone();
                        async move {
                            if t.is_enabled(&context_clone, &event_clone).await {
                                Some(t.clone())
                            } else {
                                None
                            }
                        }
                    })
                    .filter_map(|t| futures::future::ready(t));
                valid_transitions.extend(stream.collect::<Vec<_>>().await);
            }
        }

        if let Some(transition) = valid_transitions.into_iter().next() {
            self.execute_transition(&transition, current_state_ids, &event)
                .await?;
        }

        Ok(())
    }

    // Implement _get_ancestors directly using StateCollection::get
    fn _get_ancestors(&self, state_id: &S) -> Vec<S> {
        let mut ancestors = Vec::new();
        let mut current_id_opt = Some(state_id.clone());
        while let Some(current_id) = current_id_opt {
            // Use StateCollection::get(&S)
            if let Some(state) = self.states.get(&current_id) {
                if let Some(parent_id) = &state.parent {
                    ancestors.push(parent_id.clone());
                    current_id_opt = Some(parent_id.clone());
                } else {
                    current_id_opt = None; // Reached root
                }
            } else {
                current_id_opt = None; // State not found, break loop
            }
        }
        ancestors
    }

    // Renamed and simplified: Only executes actions, takes &self
    // Removed tracing temporarily
    // #[tracing::instrument(skip(self, transition, event), fields(transition = ?transition))]
    async fn execute_transition_actions(
        &self,
        transition: &Transition<S, C, E>,
        event: &E,
    ) -> StateResult<()> {
        // Add log here
        log::debug!(
            "Inside execute_transition_actions for transition: {:?}. Action count: {}",
            transition,
            transition.actions.len()
        );
        if !transition.actions.is_empty() {
            log::debug!("Executing actions for transition: {:?}", transition);
            let context_arc = self.context.clone();
            for action in &transition.actions {
                let context_clone_for_action = Arc::clone(&context_arc);
                let action = action.clone();
                let event_clone = event.clone();
                let fut =
                    async move { action.execute(context_clone_for_action, &event_clone).await };
                fut.await?; // Propagate error
            }
        }
        Ok(())
    }

    // Implement sort_states_by_depth using _get_state_depth
    #[allow(dead_code)]
    fn sort_states_by_depth(&self, states: &HashSet<S>, reverse: bool) -> Vec<S> {
        let mut sorted: Vec<S> = states.iter().cloned().collect();
        sorted.sort_by(|a, b| self._get_state_depth(a).cmp(&self._get_state_depth(b)));
        if reverse {
            sorted.reverse();
        }
        sorted
    }

    // Prefix unused methods with underscore
    fn _get_ancestors_inclusive(&self, state_id: &S) -> Vec<String> {
        let mut ancestors = vec![state_id.to_string()];
        ancestors.extend(self._get_ancestors(state_id).iter().map(|s| s.to_string()));
        ancestors
    }

    // Prefix unused methods with underscore
    fn _get_state_depth(&self, state_id: &S) -> usize {
        self._get_ancestors(state_id).len()
    }
}

// Move handle_event outside the impl Actor block
impl<C, E, S, O> Machine<C, E, S, O>
where
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    E: EventTrait + Serialize + DeserializeOwned + fmt::Debug + Clone + Send + Sync + Eq + Hash + IntoEvent + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq + Serialize + DeserializeOwned,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
{
    // ... other methods like new, send, etc. ...

    // Moved handle_event here
    // TODO: Re-evaluate the purpose and implementation of this function.
    // It currently accesses private fields and has type mismatches if uncommented.
    #[allow(dead_code)]
    async fn handle_event(&mut self, _event: Event) -> Result<(), ActorError> {
        // Prefix event with _
        // Prefix unused _child_state
        // Commenting out due to private access (self.states.states) and E0308 type error
        // for (_id, _child_state) in self.states.states.iter_mut() {
        //     // ... logic using _child_state ...
        // }
        // Temporary return to satisfy type signature
        Ok(())
    }
}

#[async_trait]
impl<C, E, S, O> Actor for Machine<C, E, S, O>
where
    C: Clone + Default + Serialize + DeserializeOwned + Send + Sync + Debug + 'static,
    E: EventTrait + Serialize + DeserializeOwned + fmt::Debug + Clone + Send + Sync + Eq + Hash + IntoEvent + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq + Serialize + DeserializeOwned,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
{
    // Define the actor's state as the machine's current states
    type State = HashSet<S>;
    type Context = C;
    type Event = E;
    type StateId = S;
    type Output = O; // Ensure this matches the Machine's O parameter

    fn initial_state(&self) -> Self::State {
        self.current_states.clone() // Return a clone of the current states
    }

    // Add back the receive method implementation
    async fn receive(
        &self,
        _state: Self::State, // Current state passed in
        _event: Self::Event, // Event received
    ) -> Result<Self::State, ActorError> {
        // Machine::send uses &mut self, Actor::receive uses &self.
        // Therefore, receive cannot directly mutate the machine state via Machine::send.
        // It should likely return the current state or handle events in a read-only way.
        // For now, return the current state, indicating the event was acknowledged
        // but not processed in a state-mutating way by this specific method.
        Ok(self.current_states.clone())
    }

    // Remove the handle_event method from here
    // async fn handle_event(&mut self, event: Event) -> Result<(), ActorError> { ... }

    // Implement other required Actor trait methods like `update`, `context`, etc.
}

/// Builder for creating Machine instances
#[derive(Clone, Debug, Serialize)]
#[serde(bound(serialize = "S: Serialize, C: Serialize"))]
pub struct MachineBuilder<C, E, S, O>
where
    C: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
    E: EventTrait + Serialize + DeserializeOwned + fmt::Debug + IntoEvent + Clone + Eq + Send + Sync + Hash + 'static + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq + Serialize + DeserializeOwned,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states (Use StateCollection with correct order)
    pub states: StateCollection<S, C, E>,
    /// Collection of transitions (Will be grouped in Machine::new)
    pub transitions: Vec<Transition<S, C, E>>,
    /// Initial state id (Use S directly)
    pub initial: S,
    /// Context for the machine
    pub context_opt: Option<C>,
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
    C: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
    E: EventTrait + Serialize + DeserializeOwned + fmt::Debug + IntoEvent + Clone + Eq + Send + Sync + Hash + 'static + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq + Serialize + DeserializeOwned,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
{
    /// Create a new MachineBuilder
    pub fn new(name: impl Into<String>, initial: S) -> Self {
        Self {
            name: name.into(),
            states: StateCollection::new(),
            transitions: Vec::new(),
            initial,
            context_opt: None,
            entry_actions: HashMap::new(),
            exit_actions: HashMap::new(),
            _phantom_e: PhantomData,
            _phantom_o: PhantomData,
        }
    }

    /// Add a state definition
    pub fn state(mut self, state: State<S, C, E>) -> Self {
        self.states.add(state);
        self
    }

    /// Add a global transition definition
    pub fn transition(mut self, transition: Transition<S, C, E>) -> Self {
        self.transitions.push(transition);
        self
    }

    /// Add an entry action for a specific state
    pub fn on_entry<A: IntoAction<C, E> + 'static>(mut self, state_id: &S, action: A) -> Self {
        self.entry_actions
            .entry(state_id.to_string())
            .or_default()
            .push(action.into_action());
        self
    }

    /// Add an exit action for a specific state
    pub fn on_exit<A: IntoAction<C, E> + 'static>(mut self, state_id: &S, action: A) -> Self {
        self.exit_actions
            .entry(state_id.to_string())
            .or_default()
            .push(action.into_action());
        self
    }

    /// Set the initial context for the machine
    pub fn context(mut self, context: C) -> Self {
        self.context_opt = Some(context);
        self
    }

    /// Build the Machine instance
    pub async fn build(self) -> StateResult<Machine<C, E, S, O>> {
        Machine::new(self).await
    }
}

/// Snapshot of the machine state for actors
#[derive(Clone, Debug, Serialize)]
#[serde(bound(serialize = "S: Serialize"))]
pub struct MachineSnapshot<C, S, O>
where
    C: Clone + Serialize + DeserializeOwned + Send + Sync + 'static + Debug,
    S: StateTrait + Send + Sync + 'static + Eq + Hash + Serialize + DeserializeOwned + Clone,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Debug,
{
    pub current_states: HashSet<S>,
    pub history_states: HashMap<String, S>,
    _phantom_s: PhantomData<S>,
    _phantom_c: PhantomData<C>,
    _phantom_o: PhantomData<O>,
}

impl<C, S, O> MachineSnapshot<C, S, O>
where
    C: Clone + Serialize + DeserializeOwned + Send + Sync + 'static + Debug,
    S: StateTrait + Send + Sync + 'static + Eq + Hash + Serialize + DeserializeOwned + Clone,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Debug,
{
    pub fn is_in(&self, state_id: &S) -> bool {
        self.current_states.contains(state_id)
    }
    pub fn current_states(&self) -> &HashSet<S> {
        &self.current_states
    }
}

pub fn get_ancestors<S: StateTrait + Clone + Eq + Hash + Serialize + DeserializeOwned + Display>(
    states: &HashMap<S, State<S>>,
    state_id: &S,
) -> Vec<S> {
    let mut ancestors = Vec::new();
    let mut current_id = state_id;
    while let Some(state) = states.get(current_id) {
        if let Some(parent_id) = &state.parent {
            ancestors.push(parent_id.clone());
            current_id = parent_id;
        } else {
            break;
        }
    }
    ancestors
}
