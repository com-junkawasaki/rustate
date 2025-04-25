use crate::action::{Action, ActionExecutor, ActionType};
use crate::transition::Transition;
use crate::{Context, Error, Event, EventTrait, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::{self, Debug, Display};
use std::hash::Hash;
use std::ops::Deref;
use uuid::Uuid;

/// Trait defining requirements for a state identifier
pub trait StateTrait:
    Serialize + DeserializeOwned + Clone + Debug + Display + Hash + Eq + Send + Sync + 'static
{
    // Methods related to hierarchy and type are now part of State<S>
}

// Implement StateTrait for String, a common use case for state IDs
impl StateTrait for String {}

// Implement StateTrait for simple static strings if needed
// impl StateTrait for &'static str {} // Requires DeserializeOwned, tricky for &'static str

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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(
    serialize = "S: Serialize",
    deserialize = "S: DeserializeOwned" // Explicit bound for S
))]
pub struct State<S, C = Context, E = Event>
where
    S: StateTrait, // S is the identifier type
    C: Clone + Send + Sync + Default + 'static + Serialize + DeserializeOwned, // Add C bounds
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Default
        + Eq
        + From<Event>
        + Clone
        + Serialize
        + DeserializeOwned, // Add E bounds
{
    /// Unique identifier for the state
    pub id: S,
    /// Type of state
    #[serde(rename = "type")]
    pub state_type: StateType,
    /// Optional parent state id
    pub parent: Option<S>,
    /// Child states (for compound and parallel states)
    // Use S directly as key if possible, otherwise String conversion needed.
    // String is simpler for now due to HashMap constraints.
    pub children: HashMap<String, State<S, C, E>>,
    /// Initial state (for compound states)
    pub initial: Option<S>,
    /// Data associated with this state
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// Internal unique identifier
    #[serde(default = "Uuid::new_v4")]
    pub(crate) uuid: Uuid,
    /// Optional meta information for the state
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    /// History type for History states
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<HistoryType>,
    /// Entry actions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entry: Vec<Action<C, E>>,
    /// Exit actions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exit: Vec<Action<C, E>>,
    /// Transitions originating from this state
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub on: HashMap<String, Vec<Transition<S, C, E>>>,

    // Add PhantomData for C and E
    #[serde(skip)]
    _phantom_c: std::marker::PhantomData<C>,
    #[serde(skip)]
    _phantom_e: std::marker::PhantomData<E>,
}

impl<S, C, E> State<S, C, E>
where
    S: StateTrait,
    C: Clone + Send + Sync + Default + 'static + Serialize + DeserializeOwned,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Default
        + Eq
        + From<Event>
        + Clone
        + Serialize
        + DeserializeOwned,
{
    /// Create a new normal state
    pub fn new(id: S) -> Self {
        Self {
            id,
            state_type: StateType::Normal,
            parent: None,
            children: HashMap::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
            entry: Vec::new(),
            exit: Vec::new(),
            on: HashMap::new(),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Create a new compound state
    // Changed to take S directly for id and initial
    pub fn new_compound(id: S, initial: S) -> Self {
        Self {
            id,
            state_type: StateType::Compound,
            parent: None,
            children: HashMap::new(),
            initial: Some(initial),
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
            entry: Vec::new(),
            exit: Vec::new(),
            on: HashMap::new(),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Create a new parallel state
    pub fn new_parallel(id: S) -> Self {
        Self {
            id,
            state_type: StateType::Parallel,
            parent: None,
            children: HashMap::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
            entry: Vec::new(),
            exit: Vec::new(),
            on: HashMap::new(),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Create a new final state
    pub fn new_final(id: S) -> Self {
        Self {
            id,
            state_type: StateType::Final,
            parent: None,
            children: HashMap::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: None,
            entry: Vec::new(),
            exit: Vec::new(),
            on: HashMap::new(),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Create a new history state
    pub fn new_history(id: S, history_type: HistoryType) -> Self {
        Self {
            id,
            state_type: StateType::History,
            parent: None,
            children: HashMap::new(),
            initial: None,
            data: None,
            uuid: Uuid::new_v4(),
            meta: None,
            history: Some(history_type),
            entry: Vec::new(),
            exit: Vec::new(),
            on: HashMap::new(),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Add a child state to this state
    pub fn add_child(&mut self, mut child_state: State<S, C, E>) -> &mut Self {
        child_state.parent = Some(self.id.clone()); // Set parent link
                                                    // Use Display trait from StateTrait for the key
        self.children
            .insert(child_state.id.to_string(), child_state);
        self
    }

    /// Add an entry action
    pub fn add_entry(&mut self, action: impl Into<Action<C, E>>) -> &mut Self {
        self.entry.push(action.into());
        self
    }

    /// Add an exit action
    pub fn add_exit(&mut self, action: impl Into<Action<C, E>>) -> &mut Self {
        self.exit.push(action.into());
        self
    }

    /// Add a transition for a specific event
    pub fn add_transition(
        &mut self,
        event: impl Into<String>,
        transition: impl Into<Transition<S, C, E>>,
    ) -> &mut Self {
        self.on
            .entry(event.into())
            .or_default()
            .push(transition.into());
        self
    }

    /// Set the data associated with this state
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
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

    pub fn id(&self) -> &S {
        &self.id
    }

    pub fn state_type(&self) -> StateType {
        self.state_type.clone() // Clone enum
    }

    pub fn parent(&self) -> Option<&S> {
        self.parent.as_ref()
    }

    pub fn children(&self) -> &HashMap<String, State<S, C, E>> {
        &self.children
    }

    pub fn initial(&self) -> Option<&S> {
        self.initial.as_ref()
    }

    pub fn data(&self) -> Option<&Value> {
        self.data.as_ref()
    }

    pub fn meta(&self) -> Option<&Value> {
        self.meta.as_ref()
    }

    pub fn history(&self) -> Option<HistoryType> {
        self.history.clone() // Clone enum
    }

    pub fn entry_actions(&self) -> &Vec<Action<C, E>> {
        &self.entry
    }

    pub fn exit_actions(&self) -> &Vec<Action<C, E>> {
        &self.exit
    }

    pub fn transitions(&self) -> &HashMap<String, Vec<Transition<S, C, E>>> {
        &self.on
    }

    /// Checks if the state is a final state
    pub fn is_final(&self) -> bool {
        self.state_type == StateType::Final
    }

    /// Checks if the state is an atomic state (no children)
    pub fn is_atomic(&self) -> bool {
        self.state_type == StateType::Normal && self.children.is_empty()
    }

    /// Checks if the state is a compound state
    pub fn is_compound(&self) -> bool {
        self.state_type == StateType::Compound
    }

    /// Checks if the state is a parallel state
    pub fn is_parallel(&self) -> bool {
        self.state_type == StateType::Parallel
    }

    /// Checks if the state is a history state
    pub fn is_history(&self) -> bool {
        matches!(self.state_type, StateType::History | StateType::DeepHistory)
    }
}

/// A collection of states
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: DeserializeOwned"))]
pub struct StateCollection<S, C = Context, E = Event>
where
    S: StateTrait,
    C: Clone + Send + Sync + Default + 'static + Serialize + DeserializeOwned,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Default
        + Eq
        + From<Event>
        + Clone
        + Serialize
        + DeserializeOwned,
{
    // Use String as key for simplicity, derived from S::Display
    states: HashMap<String, State<S, C, E>>,

    // Add PhantomData for C and E
    #[serde(skip)]
    _phantom_c: std::marker::PhantomData<C>,
    #[serde(skip)]
    _phantom_e: std::marker::PhantomData<E>,
}

impl<S, C, E> StateCollection<S, C, E>
where
    S: StateTrait,
    C: Clone + Send + Sync + Default + 'static + Serialize + DeserializeOwned,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Default
        + Eq
        + From<Event>
        + Clone
        + Serialize
        + DeserializeOwned,
{
    /// Create a new empty state collection
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            _phantom_c: std::marker::PhantomData,
            _phantom_e: std::marker::PhantomData,
        }
    }

    /// Add a state to the collection
    pub fn add(&mut self, state: State<S, C, E>) -> &mut Self {
        self.states.insert(state.id.to_string(), state);
        self
    }

    /// Get a state by id
    pub fn get(&self, id: &S) -> Option<&State<S, C, E>> {
        self.states.get(&id.to_string())
    }

    /// Get a mutable reference to a state by id
    pub fn get_mut(&mut self, id: &S) -> Option<&mut State<S, C, E>> {
        self.states.get_mut(&id.to_string())
    }

    /// Check if a state exists
    pub fn contains(&self, id: &S) -> bool {
        self.states.contains_key(&id.to_string())
    }

    /// Get all states
    pub fn all(&self) -> impl Iterator<Item = &State<S, C, E>> {
        self.states.values()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}

/// Ensure Transition struct definition uses S: StateTrait consistently
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: DeserializeOwned"))]
pub struct Transition<S, C = Context, E = Event>
where
    S: StateTrait,
    C: Clone + Send + Sync + Default + 'static + Serialize + DeserializeOwned,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Default
        + Eq
        + From<Event>
        + Clone
        + Serialize
        + DeserializeOwned,
{
    pub target: S,
    // ... other fields like actions, guards ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<String>, // Placeholder for guard condition reference
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<IntoAction<S>>,
}

impl<S, C, E> Transition<S, C, E>
where
    S: StateTrait,
    C: Clone + Send + Sync + Default + 'static + Serialize + DeserializeOwned,
    E: EventTrait
        + Send
        + Sync
        + 'static
        + Default
        + Eq
        + From<Event>
        + Clone
        + Serialize
        + DeserializeOwned,
{
    pub fn new(target: S) -> Self {
        Self {
            target,
            guard: None,
            actions: Vec::new(),
        }
    }

    pub fn with_guard(mut self, guard_ref: impl Into<String>) -> Self {
        self.guard = Some(guard_ref.into());
        self
    }

    pub fn add_action(mut self, action: impl Into<IntoAction<S>>) -> Self {
        self.actions.push(action.into());
        self
    }
}
