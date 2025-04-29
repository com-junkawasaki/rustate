use rustate::event::{Event, EventTrait, IntoEvent};
use rustate::state::StateTrait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fmt::{self, Debug, Display};

// Re-export rustate types if needed elsewhere, or use crate::state_machine::...

// --- State Machine Definition ---

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TodoState {
    #[default]
    Idle,
    AddingTodo {
        title: String,
    },
    ViewingTodos,
}

impl Display for TodoState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl TodoState {
    pub fn name(&self) -> &'static str {
        match self {
            TodoState::Idle => "Idle",
            TodoState::AddingTodo { .. } => "AddingTodo",
            TodoState::ViewingTodos => "ViewingTodos",
        }
    }
}

// Implement StateTrait for TodoState (required by Machine)
impl StateTrait for TodoState {
    // Return reference to self as required by StateTrait signature
    fn id(&self) -> &Self {
        self
    }
    // Other bounds like Debug, Display, Clone, Hash, Eq, etc. are handled by derive/impl Display
}

// Implement From<String> for TodoState (required by Machine)
impl From<String> for TodoState {
    fn from(value: String) -> Self {
        // Simple string matching, defaulting to Idle
        match value.to_lowercase().as_str() {
            "idle" => TodoState::Idle,
            // Maybe support creating other states from string if needed?
            _ => TodoState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(tag = "type")]
pub enum TodoEvent {
    Add {
        title: String,
        content: String,
    },
    View,
    Added {
        id: u32,
    },
    Viewed {
        count: usize,
    },
    #[default]
    BackToIdle,
}

impl EventTrait for TodoEvent {
    fn name(&self) -> &str {
        match self {
            TodoEvent::Add { .. } => "Add",
            TodoEvent::View => "View",
            TodoEvent::Added { .. } => "Added",
            TodoEvent::Viewed { .. } => "Viewed",
            TodoEvent::BackToIdle => "BackToIdle",
        }
    }

    fn event_type(&self) -> &str {
        self.name()
    }

    fn payload(&self) -> Option<&JsonValue> {
        match serde_json::to_value(self) {
            Ok(JsonValue::Object(mut map)) => {
                map.remove("type");
                if map.is_empty() {
                    None
                } else {
                    let owned_value = JsonValue::Object(map);
                    let static_ref: &'static JsonValue = Box::leak(Box::new(owned_value));
                    Some(static_ref)
                }
            }
            Ok(_) => None,
            Err(_) => None,
        }
    }
}

impl IntoEvent for TodoEvent {
    fn into_event(self) -> Event {
        let mut event = Event::new(self.event_type());
        // Assign directly to the public payload field
        event.payload = self.payload().cloned();
        event
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TodoContext {
    pub todos: Vec<TodoItem>,
    pub last_added_id: u32,
}

impl Display for TodoContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Context(todos: {}, last_id: {})",
            self.todos.len(),
            self.last_added_id
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TodoItem {
    pub id: u32,
    pub title: String,
    pub content: String,
    pub completed: bool,
}

// --- State Implementation ---
// NOTE: Use the actual trait from rustate: `rustate::state::StateTrait`
// The `impl StateTrait for TodoState` block is removed as it's not
// directly used by `rustate::Machine` and the methods were causing conflicts.
// The necessary trait bounds (Debug, Clone, Eq, Hash, Serialize, Deserialize, Display)
// are handled by derives or direct impls above.
// Entry/exit logic simulation remains tied to the main loop's subscription for now.
