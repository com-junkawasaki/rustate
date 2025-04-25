use crate::event::IntoEvent;
use crate::{
    actor::{ActorLogic, ActorStatus, Snapshot as ActorSnapshot},
    error::StateError,
    state::{HistoryType, State, StateCollection, StateType},
    transition::TransitionType,
    Action, Context, Error, Event, EventTrait, IntoAction, Result, StateTrait, Transition,
};
use async_trait::async_trait;
use futures::stream::{self, StreamExt, TryStreamExt};
use futures::FutureExt;
use log;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{self, Debug, Display};
use std::future::Future;
use std::hash::Hash;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

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
        + fmt::Debug
        + IntoEvent
        + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states (Use StateCollection for better management)
    #[serde(flatten)]
    pub states: StateCollection<S, C, E>,
    /// Collection of transitions (Grouped by source state ID)
    pub transitions: HashMap<S, Vec<Transition<S, C, E>>>,
    /// Initial state id
    pub initial: S,
    /// Current active state IDs
    pub current_states: HashSet<S>,
    /// Current context data wrapped in Arc<RwLock>
    #[serde(skip)]
    pub context: Arc<RwLock<C>>,
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
        + fmt::Debug
        + IntoEvent
        + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
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
            context_opt,
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
            grouped_transitions
                .entry(t.source.clone())
                .or_default()
                .push(t);
        }
        // --- Group Transitions by Source State --- End

        if states.is_empty() {
            return Err(Error::InvalidConfiguration("No states defined".into()));
        }

        if !states.contains(&initial) {
            return Err(Error::StateNotFound(initial.to_string()));
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
            initial: initial.clone(),
            entry_actions: final_entry_actions,
            exit_actions: final_exit_actions,
            history: HashMap::new(),
            _phantom_e: PhantomData,
            _phantom_o: PhantomData,
            current_states: HashSet::new(),
            context: context_rw,
        };

        machine.initialize(&initial).await?;

        Ok(machine)
    }

    /// Initialize the machine by entering the initial state
    async fn initialize(&mut self, initial_state_id: &S) -> Result<()> {
        let init_event = E::default();
        self.enter_state(initial_state_id, &init_event, self.context.clone()).await?;
        Ok(())
    }

    /// Send an event to the machine
    #[tracing::instrument(skip(self, event), fields(machine_id = %self.name, event = ?event))]
    pub async fn send(&mut self, event: E) -> Result<bool> {
        let event = event.clone();
        let current_state_ids = self.current_states.clone();
        let mut executed = false;
        let mut valid_transitions = Vec::new();

        // Read context once using Arc<RwLock>
        let current_context_locked = self.context.read().await;
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
            self.execute_transition(&transition, &current_state_ids, &event)
                .await?;
            executed = true;
        }

        Ok(executed)
    }

    /// Execute a transition
    async fn execute_transition(
        &mut self,
        transition: &Transition<S, C, E>,
        current_state_ids: &HashSet<S>,
        event: &E,
    ) -> Result<()> {
        let target_states = match &transition.target {
            Some(target) => {
                if !self.states.contains(target) {
                    return Err(StateError::StateNotFound(target.to_string()).into());
                }
                Some(target.clone())
            }
            None => None, // Targetless transition
        };

        // 1. Find the LCCA (Least Common Compound Ancestor)
        // For transitions originating from the machine root (no source specified in transition def)
        // or for targetless transitions, LCCA is implicitly the root.
        let source_state_id = transition.source.as_ref().unwrap_or(&self.initial); // Assume root if source is None
        let lcca_id = target_states
            .as_ref()
            .and_then(|target_id| self.find_lcca(source_state_id, target_id));

        // 2. Determine states to exit
        let mut exit_states = HashSet::new();
        for current_id in current_state_ids {
            // Only exit states that are descendants of the LCCA (or all if LCCA is root)
            // For external transitions, the source state itself should also be exited if it's an ancestor of the target within LCCA boundary
            if let Some(lcca) = lcca_id {
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
                    && self.is_ancestor(target_states.as_ref().unwrap(), lcca)
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
            if let Some(source) = transition.source {
                if exit_states.contains(&source)
                    || (lcca_id == Some(&source) && target_states.is_some())
                {
                    // Ensure source is exited for external
                    exit_states.insert(source.clone());
                } else if lcca_id.map_or(false, |lcca| self.is_ancestor(&source, lcca)) {
                    exit_states.insert(source.clone());
                }
            }
        }

        // 3. Execute exit actions and recursive exit
        let mut exit_futures = Vec::new();
        let context_clone = self.context.clone(); // Clone Arc for passing to async blocks
        for id in &exit_states {
            // Pass context clone here
            exit_futures.push(self.exit_state(id, event, context_clone.clone()).boxed());
        }
        // Execute exits concurrently
        let exit_results = futures::future::join_all(exit_futures).await;
        // Propagate the first error encountered during exit
        for result in exit_results {
            result?;
        }

        // 3.5 Update history for exited states *after* exiting
        for id in &exit_states {
            self.update_history_on_exit(id);
        }

        // 4. Execute transition actions (if any)
        if let Some(actions) = &transition.actions {
            let mut context_guard = self.context.write().await;
            for action in actions {
                action.execute(&mut *context_guard, event).await;
            }
            // Drop guard explicitly to release lock before entering states
            drop(context_guard);
        }

        // 5. Determine states to enter
        let mut enter_states = HashSet::new();
        if let Some(target_id) = target_states {
            // Add target and its ancestors up to (but not including) the LCCA
            let mut current = Some(target_id.clone());
            while let Some(id) = current {
                if let Some(lcca) = lcca_id {
                    // Stop if we reach LCCA, unless it's an internal transition *within* LCCA
                    if id == *lcca {
                        // If it's an internal transition and target is LCCA itself, don't enter it again.
                        // If target is a descendant of LCCA, LCCA should not be entered.
                        if transition.transition_type == TransitionType::Internal
                            && &target_id == lcca
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
                if let Some(lcca) = lcca_id {
                    if exit_states.contains(lcca) && self.is_ancestor(&target_id, lcca) {
                        enter_states.insert(lcca.clone());
                    }
                }
            }
        }

        // 6. Execute entry actions and recursive entry
        let mut enter_futures = Vec::new();
        let context_clone = self.context.clone(); // Clone Arc for passing to async blocks
        for id in &enter_states {
            // Pass context clone here
            enter_futures.push(self.enter_state(id, event, context_clone.clone()).boxed());
        }
        // Execute entries concurrently
        let enter_results = futures::future::join_all(enter_futures).await;
        // Propagate the first error encountered during entry
        for result in enter_results {
            result?;
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
    async fn enter_state(&self, state_id: &S, event: &E, context: Arc<RwLock<C>>) -> Result<()> {
        log::debug!("Entering state: {}", state_id);

        // Add state to current_states immediately? No, let execute_transition handle the final set.

        // Execute entry actions for this state first
        self.execute_entry_actions(state_id, event, context.clone())
            .await?;

        // --- Handle entering child states --- Start
        let state_info = match self.states.get(state_id) {
            Some(s) => Some((
                s.state_type.clone(),
                s.initial.clone(),
                s.history.clone(),
                s.children.keys().cloned().collect::<Vec<_>>(), // Use keys directly if S: From<String>
            )),
            None => return Err(StateError::StateNotFound(state_id.to_string()).into()), // Return error if state doesn't exist
        };

        if let Some((state_type, initial_child_opt, history_type_opt, child_keys)) = state_info {
            match state_type {
                StateType::Compound => {
                    let child_to_enter: Option<S> = if let Some(history_type) = history_type_opt {
                        let state_id_str = state_id.to_string();
                        match history_type {
                            HistoryType::Shallow => self.history.get(&state_id_str).cloned(),
                            HistoryType::Deep => {
                                // TODO: Implement proper deep history logic
                                // Needs to traverse down the history map based on nested states
                                self.history
                                    .get(&state_id_str)
                                    .cloned()
                                    .or(initial_child_opt.clone())
                            }
                        }
                        .or(initial_child_opt.clone())
                    } else {
                        initial_child_opt.clone()
                    };

                    if let Some(child_id) = child_to_enter {
                        log::debug!("Entering child {} of {}", child_id, state_id);
                        // Pass context down recursively
                        self.enter_state(&child_id, event, context).await?;
                    }
                }
                StateType::Parallel => {
                    log::debug!("Entering parallel children of {}", state_id);
                    let mut enter_futures = Vec::new();
                    let event_clone = event.clone(); // Clone event once before the loop
                    for child_key in child_keys {
                        let child_id = S::from(child_key); // Assuming S: From<String>
                        log::debug!("Entering parallel child: {}", child_id);
                        let child_id_clone = child_id.clone(); // Clone child_id for the async block
                        let context_for_child = context.clone(); // Clone Arc for the async block
                        let event_for_child = event_clone.clone(); // Clone event for the async block
                        // Pass context down recursively, clone Arc for each future
                        enter_futures.push(Box::pin(async move {
                            self.enter_state(&child_id_clone, &event_for_child, context_for_child).await
                        }));
                    }
                    let results: Vec<Result<(), StateError>> =
                        futures::future::join_all(enter_futures).await;
                    // Check all results, return first error
                    for result in results {
                        result?;
                    }
                    log::debug!("Finished entering parallel children of {}", state_id);
                }
                _ => {} // Normal, Final states
            }
        }
        // --- Handle entering child states --- End
        log::debug!("Finished entering state: {}", state_id);
        Ok(())
    }

    /// Exit a state and its active children recursively
    async fn exit_state(&self, state_id: &S, event: &E, context: Arc<RwLock<C>>) -> Result<()> {
        log::debug!("Exiting state: {}", state_id);
        // --- Handle exiting child states first (recursion/iteration needed) --- Start
        // Find currently active states that are direct children of state_id
        // Need immutable borrow of self.current_states and self.states here
        let active_children_to_exit = self
            .current_states
            .iter()
            .filter(|current_id| self.get_parent_id(current_id).as_ref() == Some(state_id))
            .cloned()
            .collect::<Vec<_>>();

        // Clone context for recursive calls
        let context_clone = context.clone();
        let event_clone = event.clone(); // Clone event for the async block
        let mut exit_futures = Vec::new();
        for child_id in active_children_to_exit {
            log::debug!("Exiting child {} of {}", child_id, state_id);
            let child_id_clone = child_id.clone(); // Clone child_id for the async block
            let context_for_child = context_clone.clone(); // Clone Arc for the async block
            let event_for_child = event_clone.clone(); // Clone event for the async block
            // Pass context down recursively
            exit_futures.push(
                // Wrap the recursive call in an async block to ensure the resulting future is Send
                async move {
                    self.exit_state(&child_id_clone, &event_for_child, context_for_child).await
                }.boxed()
            );
        }
        // Await all child exits before exiting the parent
        let results: Vec<Result<(), StateError>> = futures::future::join_all(exit_futures).await;
        // Check all results, return first error
        for result in results {
            result?; // Propagate errors
        }
        log::debug!("Finished exiting children of {}", state_id);
        // --- Handle exiting child states first --- End

        // Ensure state exists (immutable check)
        // Removed check: If it's in current_states (implicitly checked by caller context), it should exist.
        // If called directly, this check might be needed, but within transition flow it's less critical.
        // if !self.states.contains(state_id) {
        //     log::warn!("State {} not found during exit", state_id);
        //     return Ok(());
        // }

        // Removed: Update history - moved to execute_transition
        // self.update_history_on_exit(state_id);

        // Execute exit actions for this state
        self.execute_exit_actions(state_id, event, context).await?;

        // Removed: Remove state from current states - handled in execute_transition
        // self.current_states.remove(state_id);

        log::debug!("Finished exiting state: {}", state_id);
        Ok(())
    }

    /// Execute entry actions for a state
    async fn execute_entry_actions(
        &self,
        state_id: &S,
        event: &E,
        context: Arc<RwLock<C>>,
    ) -> Result<(), Error> {
        let id_str = state_id.to_string();
        // Use immutable borrow of self.entry_actions
        if let Some(actions) = self.entry_actions.get(&id_str) {
            log::debug!("Executing entry actions for {}", state_id);
            // Actions are cloned implicitly if Action struct doesn't borrow self,
            // otherwise, we need actions.clone() if actions require cloning.
            // Assuming Action::execute takes &mut C, not &mut self.
            let actions_to_run = actions.clone(); // Clone Vec<Action>
            let mut context_guard = context.write().await;
            for action in actions_to_run {
                // Iterate over cloned actions
                action.execute(&mut *context_guard, event).await;
            }
            // Drop guard explicitly after all actions for this state
            drop(context_guard);
            log::debug!("Finished entry actions for {}", state_id);
        }
        Ok(())
    }

    /// Execute exit actions for a state
    async fn execute_exit_actions(
        &self,
        state_id: &S,
        event: &E,
        context: Arc<RwLock<C>>,
    ) -> Result<(), Error> {
        let id_str = state_id.to_string();
        // Use immutable borrow of self.exit_actions
        if let Some(actions) = self.exit_actions.get(&id_str) {
            log::debug!("Executing exit actions for {}", state_id);
            let actions_to_run = actions.clone(); // Clone Vec<Action>
            let mut context_guard = context.write().await;
            for action in actions_to_run {
                // Iterate over cloned actions
                action.execute(&mut *context_guard, event).await;
            }
            // Drop guard explicitly after all actions for this state
            drop(context_guard);
            log::debug!("Finished exit actions for {}", state_id);
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
    fn find_lcca<'a>(&'a self, state1_id: &'a S, state2_id: &'a S) -> Option<&'a S> {
        let mut path1 = HashSet::new();
        let mut current = Some(state1_id);
        while let Some(id) = current {
            path1.insert(id);
            current = self.states.get(id).and_then(|s| s.parent.as_ref());
        }

        current = Some(state2_id);
        while let Some(id) = current {
            if path1.contains(id) {
                return Some(id);
            }
            current = self.states.get(id).and_then(|s| s.parent.as_ref());
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

    fn is_descendant(&self, descendant_id: &S, ancestor_id: &S) -> bool {
        let mut current = Some(descendant_id);
        while let Some(id) = current {
            if id == ancestor_id {
                return true;
            }
            current = self.states.get(id).and_then(|s| s.parent.as_ref());
        }
        false
    }

    fn is_ancestor(&self, ancestor_id: &S, descendant_id: &S) -> bool {
        let mut current = Some(ancestor_id);
        while let Some(id) = current {
            if id == descendant_id {
                return true;
            }
            current = self.states.get(id).and_then(|s| s.parent.as_ref());
        }
        false
    }
}

/// Builder for creating Machine instances
#[derive(Clone)]
pub struct MachineBuilder<C, E, S, O>
where
    C: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
    E: EventTrait + Serialize + DeserializeOwned + fmt::Debug + IntoEvent + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq,
    O: Serialize + DeserializeOwned + Clone + Send + Sync + 'static + Default + fmt::Debug,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states (Use StateCollection for better management)
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
    E: EventTrait + Serialize + DeserializeOwned + fmt::Debug + IntoEvent + Default,
    S: StateTrait + Display + Eq + Hash + Send + Sync + 'static + Clone + From<String> + PartialEq,
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
    pub history_states: HashMap<String, S>,
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
