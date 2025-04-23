use crate::event::IntoEvent;
use crate::{
    action::ActionType,
    actor::{ActorLogic, ActorStatus, Snapshot as ActorSnapshot},
    event::EventObject,
    state::StateType,
    Action, Context, Error, Event, IntoAction, Result, State, Transition,
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::marker::PhantomData;

/// Represents a state machine instance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Machine<C = Context, E = Event, O = ()>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventObject + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states
    pub states: HashMap<String, State>,
    /// Collection of transitions
    pub transitions: Vec<Transition>,
    /// Initial state id
    pub initial: String,
    /// Entry actions for states
    #[serde(skip)]
    pub(crate) entry_actions: HashMap<String, Vec<Action>>,
    /// Exit actions for states
    #[serde(skip)]
    pub(crate) exit_actions: HashMap<String, Vec<Action>>,
    /// History states mapping (state id -> last active child)
    pub(crate) history: HashMap<String, String>,
    /// The type markers
    #[serde(skip)]
    _phantom_c: PhantomData<C>,
    #[serde(skip)]
    _phantom_e: PhantomData<E>,
    #[serde(skip)]
    _phantom_o: PhantomData<O>,
}

impl<C, E, O> Machine<C, E, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventObject + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    /// Create a new state machine instance from a builder
    pub async fn new<BuilderC, BuilderE, BuilderO>(
        builder: MachineBuilder<BuilderC, BuilderE, BuilderO>,
    ) -> Result<Self>
    where
        BuilderC: Clone + Send + Sync + Default + 'static,
        BuilderE: EventObject + Send + Sync + 'static,
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
            _phantom_o: PhantomData,
        };

        // Initialize by entering the initial state
        machine.initialize().await?;

        Ok(machine)
    }

    /// Initialize the machine by entering the initial state
    async fn initialize(&mut self) -> Result<()> {
        let initial_state_id = self.initial.clone();
        self.enter_state(&initial_state_id, &Event::new("init"))
            .await?;
        Ok(())
    }

    /// Send an event to the machine
    pub async fn send<EV: IntoEvent + Send>(&mut self, event: EV) -> Result<bool> {
        let event = event.into_event();
        let mut processed = false;

        // Create a copy of current states to iterate over
        let current_states: Vec<_> = self.current_states.iter().cloned().collect();

        for state_id in current_states {
            if self.process_state_event(&state_id, &event).await? {
                processed = true;
            }
        }

        // 状態が変更された場合、キャッシュをクリア
        if processed {
            self.current_state_cache = None;
        }

        Ok(processed)
    }

    /// Process an event for a specific state
    #[async_recursion]
    async fn process_state_event(&mut self, state_id: &str, event: &Event) -> Result<bool> {
        // Find enabled transitions for this state
        let potential_transitions: Vec<_> = self
            .transitions
            .iter()
            .filter(|t| t.source == state_id || t.source == "*")
            .collect();

        let mut enabled_transition: Option<Transition> = None;
        for t in &potential_transitions {
            if t.is_enabled(&self.context, event).await {
                if enabled_transition.is_none() || t.source != "*" {
                    enabled_transition = Some((*t).clone());
                    if t.source != "*" {
                        break;
                    }
                }
            }
        }

        if let Some(transition) = enabled_transition {
            // Execute the transition
            self.execute_transition(&transition, event).await?;
            Ok(true)
        } else {
            // No transitions found for this state, check parent
            if let Some(parent_id) = self.get_parent_id(state_id) {
                self.process_state_event(&parent_id, event).await
            } else {
                Ok(false)
            }
        }
    }

    /// Execute a transition
    async fn execute_transition(&mut self, transition: &Transition, event: &Event) -> Result<()> {
        let source_id = if transition.source == "*" {
            // For wildcard transitions, use the current state
            if let Some(current_state) = self.current_states.iter().next() {
                current_state.clone()
            } else {
                return Err(Error::InvalidConfiguration(
                    "No current state for wildcard transition".into(),
                ));
            }
        } else {
            transition.source.clone()
        };

        // For internal transitions, just execute the actions
        if transition.target.is_none() {
            transition.execute_actions(&mut self.context, event).await;
            return Ok(());
        }

        let target_id = transition
            .target
            .as_ref()
            .ok_or_else(|| Error::InvalidTransition("No target state".into()))?
            .clone();

        // Exit source state
        self.exit_state(&source_id, event).await?;

        // Execute transition actions
        transition.execute_actions(&mut self.context, event).await;

        // Enter target state
        self.enter_state(&target_id, event).await?;

        Ok(())
    }

    /// Enter a state and its initial substates if applicable
    #[async_recursion]
    async fn enter_state(&mut self, state_id: &str, event: &Event) -> Result<()> {
        let state = self
            .states
            .get(state_id)
            .ok_or_else(|| Error::StateNotFound(state_id.to_string()))?
            .clone();

        // Add to current states
        self.current_states.insert(state_id.to_string());

        // Execute entry actions
        if let Some(actions) = self.entry_actions.get(state_id) {
            for action in actions.clone() {
                action.execute(&mut self.context, event).await;
            }
        }

        // Handle different state types
        match state.state_type {
            StateType::Compound => {
                // Enter initial substate
                if let Some(initial) = state.initial {
                    self.enter_state(&initial, event).await?;
                } else {
                    return Err(Error::InvalidConfiguration(format!(
                        "Compound state '{}' has no initial state",
                        state_id
                    )));
                }
            }
            StateType::Parallel => {
                // Enter all child states sequentially
                for child_id in state.children {
                    self.enter_state(&child_id, event).await?; // Await sequentially
                }
            }
            StateType::History => {
                // Enter the last active child state if in history, otherwise the parent's initial
                if let Some(last_active) = self.history.get(state_id).cloned() {
                    self.enter_state(&last_active, event).await?;
                } else if let Some(parent_id) = self.get_parent_id(state_id) {
                    let parent = self
                        .states
                        .get(&parent_id)
                        .ok_or_else(|| Error::StateNotFound(parent_id.to_string()))?
                        .clone();

                    if let Some(initial) = parent.initial {
                        self.enter_state(&initial, event).await?;
                    }
                }
            }
            StateType::DeepHistory => {
                // Deep history logic would be more complex in a real implementation
                // For now, just use the same logic as regular history
                if let Some(last_active) = self.history.get(state_id).cloned() {
                    self.enter_state(&last_active, event).await?;
                } else if let Some(parent_id) = self.get_parent_id(state_id) {
                    let parent = self
                        .states
                        .get(&parent_id)
                        .ok_or_else(|| Error::StateNotFound(parent_id.to_string()))?
                        .clone();

                    if let Some(initial) = parent.initial {
                        self.enter_state(&initial, event).await?;
                    }
                }
            }
            _ => {} // Normal and Final states don't have special entry logic
        }

        Ok(())
    }

    /// Exit a state and its substates
    #[async_recursion]
    async fn exit_state(&mut self, state_id: &str, event: &Event) -> Result<()> {
        let state = self
            .states
            .get(state_id)
            .ok_or_else(|| Error::StateNotFound(state_id.to_string()))?
            .clone();

        // Record in history if it has a parent
        if let Some(parent_id) = self.get_parent_id(state_id) {
            self.history.insert(parent_id, state_id.to_string());
        }

        // First exit children (if any) sequentially
        match state.state_type {
            StateType::Compound | StateType::Parallel => {
                // Get all active children
                let active_children: Vec<_> = state
                    .children
                    .iter()
                    .filter(|child_id| self.current_states.contains(*child_id))
                    .cloned()
                    .collect();

                // Exit each active child sequentially
                for child_id in active_children {
                    self.exit_state(&child_id, event).await?; // Await sequentially
                }
            }
            _ => {} // Other state types don't have children to exit
        }

        // Execute exit actions
        if let Some(actions) = self.exit_actions.get(state_id) {
            for action in actions.clone() {
                action.execute(&mut self.context, event).await;
            }
        }

        // Remove from current states
        self.current_states.remove(state_id);

        Ok(())
    }

    /// Get the parent id of a state
    fn get_parent_id(&self, state_id: &str) -> Option<String> {
        self.states
            .get(state_id)
            .and_then(|state| state.parent.clone())
    }

    /// Check if a state is active
    pub fn is_in(&self, state_id: &str) -> bool {
        self.current_states.contains(state_id)
    }

    /// Serialize the machine to JSON
    pub fn to_json(&self) -> Result<String> {
        let json = serde_json::to_string_pretty(self)?;
        Ok(json)
    }

    /// Deserialize the machine from JSON
    pub fn from_json(json: &str) -> Result<Self> {
        let machine: Self = serde_json::from_str(json)?;
        Ok(machine)
    }

    /// 状態IDから型Sへのマッピング関数を設定
    pub fn with_state_mapper(mut self, mapper: fn(&str) -> S) -> Self {
        self.state_mapper = Some(mapper);
        self
    }

    /// Get the current state
    pub fn current_state(&self) -> S {
        // 現在アクティブな状態IDを取得
        if self.current_states.is_empty() {
            // 初期化されていない場合はエラー
            panic!("ステートマシンが初期化されていません。send() を呼び出す前に initialize() を呼び出してください。");
        }

        // アクティブな状態の中から最初の一つを取得
        // 注: より複雑な状態階層では、最も具体的な（leaf）状態を選択するロジックが必要かもしれません
        let state_id = self.current_states.iter().next().unwrap();

        // 状態IDからS型への変換
        if let Some(mapper) = self.state_mapper {
            // 現在の状態を返す（所有権を移す）
            mapper(state_id)
        } else {
            // マッパーが設定されていない場合はエラー
            panic!("状態マッパーが設定されていません。Machine::with_state_mapper()を使用してマッパーを設定してください。");
        }
    }

    /// Apply a transition with the given event
    pub async fn transition<EV: IntoEvent + Send>(
        &mut self,
        event: EV,
        context: Context,
    ) -> Result<S> {
        self.context = context;
        let event = event.into_event();
        let result = self.send(event).await?;

        // 状態が変更された場合、キャッシュをクリア
        if result {
            self.current_state_cache = None;
        }

        Ok(self.current_state())
    }
}

// Implement ActorLogic for Machine
#[async_trait::async_trait]
impl<C, E, O> ActorLogic<MachineSnapshot<C, O>, E> for Machine<C, E, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventObject + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    fn get_initial_snapshot(&self, input: Option<()>) -> MachineSnapshot<C, O> {
        let initial_context = C::default(); // Or from machine definition/input
        let initial_value = serde_json::Value::String(self.initial.clone());

        MachineSnapshot {
            inner: ActorSnapshot {
                value: initial_value,
                context: initial_context,
                output: None,
                status: ActorStatus::Active,
            },
        }
    }

    async fn transition(
        &self,
        snapshot: MachineSnapshot<C, O>,
        event: E,
    ) -> Result<MachineSnapshot<C, O>> {
        self.step(snapshot, event).await
    }
}

/// Builder for constructing state machines
pub struct MachineBuilder<C = Context, E = Event, O = ()>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventObject + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    /// Name of the machine
    pub name: String,
    /// Collection of states
    pub states: HashMap<String, State>,
    /// Collection of transitions
    pub transitions: Vec<Transition>,
    /// Initial state id
    pub initial: String,
    /// Context for the machine
    pub context: Option<C>,
    /// Entry actions for states
    pub(crate) entry_actions: HashMap<String, Vec<Action>>,
    /// Exit actions for states
    pub(crate) exit_actions: HashMap<String, Vec<Action>>,
    /// Type markers
    _phantom_c: PhantomData<C>,
    _phantom_e: PhantomData<E>,
    _phantom_o: PhantomData<O>,
}

impl<C, E, O> MachineBuilder<C, E, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventObject + Send + Sync + 'static,
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
            _phantom_o: PhantomData,
        }
    }

    /// Set the initial state
    pub fn initial(mut self, state_id: impl Into<String>) -> Self {
        self.initial = state_id.into();
        self
    }

    /// Add a state to the machine
    pub fn state(mut self, state: State) -> Self {
        if self.states.is_empty() && self.initial.is_empty() {
            self.initial = state.id.clone();
        }
        self.states.insert(state.id.clone(), state);
        self
    }

    /// Add a transition to the machine
    pub fn transition(mut self, transition: Transition) -> Self {
        self.transitions.push(transition);
        self
    }

    /// Add an entry action to a state
    pub fn on_entry<A: IntoAction>(mut self, state_id: impl Into<String>, action: A) -> Self {
        let state_id = state_id.into();
        let action = action.into_action(ActionType::Entry);
        self.entry_actions.entry(state_id).or_default().push(action);
        self
    }

    /// Add an exit action to a state
    pub fn on_exit<A: IntoAction>(mut self, state_id: impl Into<String>, action: A) -> Self {
        let state_id = state_id.into();
        let action = action.into_action(ActionType::Exit);
        self.exit_actions.entry(state_id).or_default().push(action);
        self
    }

    /// Set the context for the machine
    pub fn context(mut self, context: C) -> Self {
        self.context = Some(context);
        self
    }

    /// Build the state machine
    pub async fn build(self) -> Result<Machine<C, E, O>> {
        Machine::new(self).await
    }
}

impl<C, E, O> Clone for MachineBuilder<C, E, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventObject + Send + Sync + 'static,
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
            _phantom_o: PhantomData,
        }
    }
}

// --- MachineSnapshot ---
// (Definition remains the same as previous step)
#[derive(Clone, Debug, PartialEq)]
pub struct MachineSnapshot<C, O = ()> {
    inner: ActorSnapshot<C, O>,
}

impl<C, O> MachineSnapshot<C, O> {
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
impl<C, E, O> Machine<C, E, O>
where
    C: Clone + Send + Sync + Default + 'static,
    E: EventObject + Send + Sync + 'static,
    O: Clone + Send + Sync + 'static,
{
    async fn step(
        &self,
        current_snapshot: MachineSnapshot<C, O>,
        event: E,
    ) -> Result<MachineSnapshot<C, O>> {
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
    ) -> Result<Option<(Transition, String)>> {
        let mut candidates = Vec::new();

        for state_id in active_states {
            let mut current_id_opt = Some(state_id.clone());
            while let Some(current_id) = current_id_opt {
                for t in &self.transitions {
                    if t.source == current_id || t.source == "*" {
                        // Check current state or wildcard
                        if t.is_enabled(context, event).await {
                            candidates.push((
                                t.clone(),
                                state_id.clone(),
                                self.get_state_depth(&current_id),
                            ));
                            // Don't break, collect all candidates from this path
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
            .map(|(t, src_id, _)| (t.clone(), src_id.clone())))
    }

    /// Calculates the set of states to exit, enter, and the common ancestor.
    fn calculate_transition_sets(
        &self,
        source_id: &str,
        transition: &Transition,
    ) -> (HashSet<String>, HashSet<String>, Option<String>) {
        if transition.target.is_none() {
            // Internal transition
            return (
                HashSet::new(),
                HashSet::new(),
                self.get_parent_id(source_id),
            );
        }
        let target_id = transition.target.as_ref().unwrap();

        let mut exit_set = HashSet::new();
        let mut entry_set = HashSet::new();

        let source_ancestors = self.get_ancestors(source_id);
        let target_ancestors = self.get_ancestors(target_id);

        // Find LCA (Least Common Ancestor)
        let mut common_ancestor = None;
        for ancestor in source_ancestors.iter().rev() {
            // Check from root downwards
            if target_ancestors.contains(ancestor) {
                common_ancestor = Some(ancestor.clone());
                break;
            }
        }

        // Calculate exit set (states exited up to LCA)
        let mut current_exit_id = Some(source_id.to_string());
        while let Some(id) = current_exit_id {
            if common_ancestor.as_ref() == Some(&id) {
                break;
            }
            exit_set.insert(id.clone());
            current_exit_id = self.get_parent_id(&id);
        }

        // Calculate entry set (states entered from LCA downwards, including initial substates)
        let mut entry_path = VecDeque::new();
        let mut current_entry_id = Some(target_id.to_string());
        while let Some(id) = current_entry_id {
            if common_ancestor.as_ref() == Some(&id) {
                break;
            }
            entry_path.push_front(id.clone());
            current_entry_id = self.get_parent_id(&id);
        }

        for id in entry_path {
            entry_set.insert(id.clone());
            self.add_initial_descendants(&id, &mut entry_set);
        }

        (exit_set, entry_set, common_ancestor)
    }

    /// Recursively adds initial descendant states for compound/parallel states.
    fn add_initial_descendants(&self, state_id: &str, entry_set: &mut HashSet<String>) {
        if let Some(state) = self.states.get(state_id) {
            match state.state_type {
                StateType::Compound => {
                    if let Some(initial_child) = &state.initial {
                        if entry_set.insert(initial_child.clone()) {
                            self.add_initial_descendants(initial_child, entry_set);
                        }
                    }
                }
                StateType::Parallel => {
                    for child_id in &state.children {
                        if entry_set.insert(child_id.clone()) {
                            self.add_initial_descendants(child_id, entry_set);
                        }
                    }
                }
                _ => {}
            }
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

    /// Get the parent id of a state
    fn get_parent_id(&self, state_id: &str) -> Option<String> {
        self.states
            .get(state_id)
            .and_then(|state| state.parent.clone())
    }

    /// Get ancestors of a state up to the root.
    fn get_ancestors(&self, state_id: &str) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current_id = Some(state_id.to_string());
        while let Some(id) = current_id {
            ancestors.push(id.clone());
            current_id = self.get_parent_id(&id);
        }
        ancestors // Root is the last element
    }

    /// Get the depth of a state node (root = 0).
    fn get_state_depth(&self, state_id: &str) -> usize {
        self.get_ancestors(state_id).len().saturating_sub(1)
    }
}
