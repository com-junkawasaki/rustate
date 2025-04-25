use crate::{Context, Error, Event, EventTrait, IntoAction, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use uuid::Uuid;

/// Trait for state objects in a state machine
pub trait StateTrait: Clone + fmt::Debug + PartialEq + Eq + Hash + Send + Sync + 'static {
    /// Get the unique identifier for this state
    fn id(&self) -> &str;

    /// Get the state type
    fn state_type(&self) -> &StateType;

    /// Get the parent state id, if any
    fn parent(&self) -> Option<&str>;

    /// Get child states (for compound and parallel states)
    fn children(&self) -> &[S];

    /// Get initial state id (for compound states)
    fn initial(&self) -> Option<&str>;

    /// Get data associated with this state
    fn data(&self) -> Option<&Value>;

    /// Get the history type, if this is a history state
    fn history(&self) -> Option<HistoryType>;

    /// Check if the state is a final state
    fn is_final(&self) -> bool {
        *self.state_type() == StateType::Final
    }

    /// Check if the state is atomic (has no children)
    fn is_atomic(&self) -> bool {
        self.children().is_empty()
    }

    /// Check if the state is a compound state
    fn is_compound(&self) -> bool {
        *self.state_type() == StateType::Compound
    }

    /// Check if the state is a parallel state
    fn is_parallel(&self) -> bool {
        *self.state_type() == StateType::Parallel
    }

    /// Check if the state is a history state
    fn is_history(&self) -> bool {
        *self.state_type() == StateType::History || *self.state_type() == StateType::DeepHistory
    }
}

/// Represents a type of state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
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

/// Represents the history mechanism for history states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum HistoryType {
    /// A shallow history state
    Shallow,
    /// A deep history state
    Deep,
}

/// Represents a state in a state machine
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub struct State<S = String>
where
    S: StateTrait + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>,
{
    /// Unique identifier for the state
    pub id: S,
    /// Type of state
    pub state_type: StateType,
    /// Optional parent state id
    pub parent: Option<S>,
    /// Child states (for compound and parallel states)
    pub children: Vec<S>,
    /// Initial state (for compound states)
    pub initial: Option<S>,
    /// Data associated with this state
    pub data: Option<Value>,
    /// Internal unique identifier
    #[serde(default = "Uuid::new_v4")]
    pub(crate) uuid: Uuid,
    /// Optional meta information for the state
    pub meta: Option<Value>,
    /// History type for History states
    pub history: Option<HistoryType>,
}

impl<S> State<S>
where
    S: StateTrait + Send + Sync + 'static + Clone + From<String> + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new normal state
    pub fn new(id: S) -> Self {
        Self {
            id,
            state_type: StateType::Normal,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
        }
    }

    /// Create a new compound state
    pub fn new_compound(id: S, initial: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            id,
            state_type: StateType::Compound,
            parent: None,
            children: Vec::new(),
            initial: Some(initial.into()),
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
        }
    }

    /// Create a new parallel state
    pub fn new_parallel(id: S) -> Self {
        Self {
            id,
            state_type: StateType::Parallel,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
        }
    }

    /// Create a new final state
    pub fn new_final(id: S) -> Self {
        Self {
            id,
            state_type: StateType::Final,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
        }
    }

    /// Create a new history state
    pub fn new_history(id: S) -> Self {
        Self {
            id,
            state_type: StateType::History,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
        }
    }

    /// Create a new deep history state
    pub fn new_deep_history(id: S) -> Self {
        Self {
            id,
            state_type: StateType::DeepHistory,
            parent: None,
            children: Vec::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
        }
    }

    /// Add a child state to this state
    pub fn add_child(&mut self, child_id: S) -> &mut Self {
        self.children.push(child_id);
        self
    }

    /// Set the data associated with this state
    pub fn with_data(&mut self, data: Value) -> &mut Self {
        self.data = Some(data);
        self
    }

    /// Set the state type
    pub fn with_type(mut self, state_type: StateType) -> Self {
        self.state_type = state_type;
        self
    }

    /// Set the parent state ID
    pub fn with_parent(mut self, parent_id: S) -> Self {
        self.parent = Some(parent_id);
        self
    }

    /// Set the initial child state ID
    pub fn with_initial(mut self, initial_id: S) -> Self {
        self.initial = Some(initial_id);
        self
    }

    /// Set the state meta information
    pub fn with_meta(mut self, meta: Value) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Set the history type
    pub fn with_history(mut self, history_type: HistoryType) -> Self {
        self.history = Some(history_type);
        self
    }
}

impl<S> StateTrait for State<S>
where
    S: StateTrait + Send + Sync + 'static + Clone + From<String> + fmt::Display + Serialize + for<'de> Deserialize<'de>,
{
    fn id(&self) -> &str {
        Box::leak(self.id.to_string().into_boxed_str())
    }

    fn state_type(&self) -> &StateType {
        &self.state_type
    }

    fn parent(&self) -> Option<&str> {
        self.parent.as_ref().map(|s| Box::leak(s.to_string().into_boxed_str()) as &str)
    }

    fn children(&self) -> &[S] {
        &self.children
    }

    fn initial(&self) -> Option<&str> {
        self.initial.as_ref().map(|s| Box::leak(s.to_string().into_boxed_str()) as &str)
    }

    fn data(&self) -> Option<&Value> {
        self.data.as_ref()
    }

    fn history(&self) -> Option<HistoryType> {
        self.history.clone()
    }
}

/// A collection of states
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateCollection {
    states: HashMap<String, State<String>>,
}

impl StateCollection {
    /// Create a new empty state collection
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Add a state to the collection
    pub fn add(&mut self, state: State<String>) -> &mut Self {
        self.states.insert(state.id.to_string(), state);
        self
    }

    /// Get a state by id
    pub fn get(&self, id: &str) -> Option<&State<String>> {
        self.states.get(id)
    }

    /// Get a mutable reference to a state by id
    pub fn get_mut(&mut self, id: &str) -> Option<&mut State<String>> {
        self.states.get_mut(id)
    }

    /// Check if a state exists
    pub fn contains(&self, id: &str) -> bool {
        self.states.contains_key(id)
    }

    /// Get all states
    pub fn all(&self) -> impl Iterator<Item = &State<String>> {
        self.states.values()
    }
}
