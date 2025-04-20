use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Represents a type of state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateType {
    /// A normal state
    Normal,
    /// A state with children (compound state)
    Compound,
    /// A parallel state that can be in multiple child states simultaneously
    Parallel,
    /// A final state
    Final,
    /// A history state that remembers the last active state
    History,
    /// A deep history state that remembers the last active substate
    DeepHistory,
}

/// Represents a state in a state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// Unique identifier for the state
    pub id: String,
    /// Type of state
    pub state_type: StateType,
    /// Optional parent state id
    pub parent: Option<String>,
    /// Child states (for compound and parallel states)
    pub children: Vec<String>,
    /// Initial state (for compound states)
    pub initial: Option<String>,
    /// Data associated with this state
    pub data: Option<serde_json::Value>,
    /// Internal unique identifier
    #[serde(default = "Uuid::new_v4")]
    pub(crate) uuid: Uuid,
}

impl State {
    /// Create a new normal state
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state_type: StateType::Normal,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
        }
    }

    /// Create a new compound state
    pub fn new_compound(id: impl Into<String>, initial: impl Into<String>) -> Self {
        let initial = initial.into();
        Self {
            id: id.into(),
            state_type: StateType::Compound,
            parent: None,
            children: Vec::new(),
            initial: Some(initial),
            data: None,
            uuid: Uuid::new_v4(),
        }
    }

    /// Create a new parallel state
    pub fn new_parallel(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state_type: StateType::Parallel,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
        }
    }

    /// Create a new final state
    pub fn new_final(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state_type: StateType::Final,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
        }
    }

    /// Create a new history state
    pub fn new_history(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state_type: StateType::History,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
        }
    }

    /// Create a new deep history state
    pub fn new_deep_history(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            state_type: StateType::DeepHistory,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
        }
    }

    /// Add a child state to this state
    pub fn add_child(&mut self, child_id: impl Into<String>) -> &mut Self {
        self.children.push(child_id.into());
        self
    }

    /// Set the data associated with this state
    pub fn with_data(&mut self, data: impl Into<serde_json::Value>) -> &mut Self {
        self.data = Some(data.into());
        self
    }
}

/// A collection of states
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateCollection {
    states: HashMap<String, State>,
}

impl StateCollection {
    /// Create a new empty state collection
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Add a state to the collection
    pub fn add(&mut self, state: State) -> &mut Self {
        self.states.insert(state.id.clone(), state);
        self
    }

    /// Get a state by id
    pub fn get(&self, id: &str) -> Option<&State> {
        self.states.get(id)
    }

    /// Get a mutable reference to a state by id
    pub fn get_mut(&mut self, id: &str) -> Option<&mut State> {
        self.states.get_mut(id)
    }

    /// Check if a state exists
    pub fn contains(&self, id: &str) -> bool {
        self.states.contains_key(id)
    }

    /// Get all states
    pub fn all(&self) -> impl Iterator<Item = &State> {
        self.states.values()
    }
} 