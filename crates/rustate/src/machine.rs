use crate::{
    action::ActionType, state::StateType, Action, Context, Error, Event, IntoAction,
    Result, State, Transition,
};
use crate::event::IntoEvent;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Represents a state machine instance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Machine {
    /// Name of the machine
    pub name: String,
    /// Collection of states
    pub states: HashMap<String, State>,
    /// Collection of transitions
    pub transitions: Vec<Transition>,
    /// Initial state id
    pub initial: String,
    /// Current state(s)
    pub current_states: HashSet<String>,
    /// Extended state (context)
    pub context: Context,
    /// Entry actions for states
    #[serde(skip)]
    pub(crate) entry_actions: HashMap<String, Vec<Action>>,
    /// Exit actions for states
    #[serde(skip)]
    pub(crate) exit_actions: HashMap<String, Vec<Action>>,
    /// History states mapping (state id -> last active child)
    pub(crate) history: HashMap<String, String>,
}

impl Machine {
    /// Create a new state machine instance from a builder
    pub fn new(builder: MachineBuilder) -> Result<Self> {
        let MachineBuilder {
            name,
            states,
            transitions,
            initial,
            entry_actions,
            exit_actions,
            context,
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
            current_states: HashSet::new(),
            context: context.unwrap_or_else(Context::new),
            entry_actions,
            exit_actions,
            history: HashMap::new(),
        };

        // Initialize by entering the initial state
        machine.initialize()?;

        Ok(machine)
    }

    /// Initialize the machine by entering the initial state
    fn initialize(&mut self) -> Result<()> {
        let initial_state_id = self.initial.clone();
        self.enter_state(&initial_state_id, &Event::new("init"))?;
        Ok(())
    }

    /// Send an event to the machine
    pub fn send<E: IntoEvent>(&mut self, event: E) -> Result<bool> {
        let event = event.into_event();
        let mut processed = false;

        // Create a copy of current states to iterate over
        let current_states: Vec<_> = self.current_states.iter().cloned().collect();

        for state_id in current_states {
            if self.process_state_event(&state_id, &event)? {
                processed = true;
            }
        }

        Ok(processed)
    }

    /// Process an event for a specific state
    fn process_state_event(&mut self, state_id: &str, event: &Event) -> Result<bool> {
        // Find enabled transitions for this state
        let enabled_transition = self
            .transitions
            .iter()
            .find(|t| t.source == state_id && t.is_enabled(&self.context, event))
            .cloned();

        if let Some(transition) = enabled_transition {
            // Execute the transition
            self.execute_transition(&transition, event)?;
            Ok(true)
        } else {
            // No transitions found for this state, check parent
            if let Some(parent_id) = self.get_parent_id(state_id) {
                self.process_state_event(&parent_id, event)
            } else {
                Ok(false)
            }
        }
    }

    /// Execute a transition
    fn execute_transition(&mut self, transition: &Transition, event: &Event) -> Result<()> {
        let source_id = transition.source.clone();
        
        // For internal transitions, just execute the actions
        if transition.target.is_none() {
            transition.execute_actions(&mut self.context, event);
            return Ok(());
        }

        let target_id = transition
            .target
            .as_ref()
            .ok_or_else(|| Error::InvalidTransition("No target state".into()))?
            .clone();

        // Exit source state
        self.exit_state(&source_id, event)?;
        
        // Execute transition actions
        transition.execute_actions(&mut self.context, event);
        
        // Enter target state
        self.enter_state(&target_id, event)?;
        
        Ok(())
    }

    /// Enter a state and its initial substates if applicable
    fn enter_state(&mut self, state_id: &str, event: &Event) -> Result<()> {
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
                action.execute(&mut self.context, event);
            }
        }
        
        // Handle different state types
        match state.state_type {
            StateType::Compound => {
                // Enter initial substate
                if let Some(initial) = state.initial {
                    self.enter_state(&initial, event)?;
                } else {
                    return Err(Error::InvalidConfiguration(format!(
                        "Compound state '{}' has no initial state",
                        state_id
                    )));
                }
            }
            StateType::Parallel => {
                // Enter all child states
                for child_id in state.children {
                    self.enter_state(&child_id, event)?;
                }
            }
            StateType::History => {
                // Enter the last active child state if in history, otherwise the parent's initial
                if let Some(last_active) = self.history.get(state_id).cloned() {
                    self.enter_state(&last_active, event)?;
                } else if let Some(parent_id) = self.get_parent_id(state_id) {
                    let parent = self
                        .states
                        .get(&parent_id)
                        .ok_or_else(|| Error::StateNotFound(parent_id.to_string()))?
                        .clone();
                    
                    if let Some(initial) = parent.initial {
                        self.enter_state(&initial, event)?;
                    }
                }
            }
            StateType::DeepHistory => {
                // Deep history logic would be more complex in a real implementation
                // For now, just use the same logic as regular history
                if let Some(last_active) = self.history.get(state_id).cloned() {
                    self.enter_state(&last_active, event)?;
                } else if let Some(parent_id) = self.get_parent_id(state_id) {
                    let parent = self
                        .states
                        .get(&parent_id)
                        .ok_or_else(|| Error::StateNotFound(parent_id.to_string()))?
                        .clone();
                    
                    if let Some(initial) = parent.initial {
                        self.enter_state(&initial, event)?;
                    }
                }
            }
            _ => {} // Normal and Final states don't have special entry logic
        }
        
        Ok(())
    }

    /// Exit a state and its substates
    fn exit_state(&mut self, state_id: &str, event: &Event) -> Result<()> {
        let state = self
            .states
            .get(state_id)
            .ok_or_else(|| Error::StateNotFound(state_id.to_string()))?
            .clone();
        
        // Record in history if it has a parent
        if let Some(parent_id) = self.get_parent_id(state_id) {
            self.history.insert(parent_id, state_id.to_string());
        }
        
        // First exit children (if any) in reverse order
        match state.state_type {
            StateType::Compound | StateType::Parallel => {
                // Get all active children
                let active_children: Vec<_> = state
                    .children
                    .iter()
                    .filter(|child_id| self.current_states.contains(*child_id))
                    .cloned()
                    .collect();
                
                // Exit each active child
                for child_id in active_children {
                    self.exit_state(&child_id, event)?;
                }
            }
            _ => {} // Other state types don't have children to exit
        }
        
        // Execute exit actions
        if let Some(actions) = self.exit_actions.get(state_id) {
            for action in actions.clone() {
                action.execute(&mut self.context, event);
            }
        }
        
        // Remove from current states
        self.current_states.remove(state_id);
        
        Ok(())
    }

    /// Get the parent id of a state
    fn get_parent_id(&self, state_id: &str) -> Option<String> {
        self.states.get(state_id).and_then(|state| state.parent.clone())
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
}

/// Builder for creating state machines
#[derive(Default)]
pub struct MachineBuilder {
    /// Name of the machine
    pub name: String,
    /// Collection of states
    pub states: HashMap<String, State>,
    /// Collection of transitions
    pub transitions: Vec<Transition>,
    /// Initial state id
    pub initial: String,
    /// Context for the machine
    pub context: Option<Context>,
    /// Entry actions for states
    pub(crate) entry_actions: HashMap<String, Vec<Action>>,
    /// Exit actions for states
    pub(crate) exit_actions: HashMap<String, Vec<Action>>,
}

impl MachineBuilder {
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
        self.entry_actions
            .entry(state_id)
            .or_insert_with(Vec::new)
            .push(action);
        self
    }

    /// Add an exit action to a state
    pub fn on_exit<A: IntoAction>(mut self, state_id: impl Into<String>, action: A) -> Self {
        let state_id = state_id.into();
        let action = action.into_action(ActionType::Exit);
        self.exit_actions
            .entry(state_id)
            .or_insert_with(Vec::new)
            .push(action);
        self
    }

    /// Set the context for the machine
    pub fn context(mut self, context: Context) -> Self {
        self.context = Some(context);
        self
    }

    /// Build the state machine
    pub fn build(self) -> Result<Machine> {
        Machine::new(self)
    }
} 