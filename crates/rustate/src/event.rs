use serde::{Deserialize, Serialize};
use std::fmt;

/// Trait for event objects in a state machine
pub trait EventTrait: Clone + fmt::Debug + PartialEq + Eq + Send + Sync + 'static {
    /// Get the event type
    fn event_type(&self) -> &str;

    /// Get the payload data, if any
    fn payload(&self) -> Option<&serde_json::Value>;

    /// Get the name identifier of the event.
    fn name(&self) -> &str;
}

/// Represents an event that can trigger state transitions
///
/// Events are identified by their type and can optionally carry a payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Event {
    /// The event type
    #[serde(rename = "type")]
    pub event_type: String,
    /// Optional payload data
    pub payload: Option<serde_json::Value>,
}

impl Event {
    /// Create a new event
    pub fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            payload: None,
        }
    }

    /// Create a new event with payload
    pub fn with_payload(event_type: &str, payload: serde_json::Value) -> Self {
        Self {
            event_type: event_type.to_string(),
            payload: Some(payload),
        }
    }

    // Expose payload_mut if mutable access is needed
    pub fn payload_mut(&mut self) -> Option<&mut serde_json::Value> {
        self.payload.as_mut()
    }
}

impl EventTrait for Event {
    fn event_type(&self) -> &str {
        &self.event_type
    }

    fn payload(&self) -> Option<&serde_json::Value> {
        self.payload.as_ref()
    }

    fn name(&self) -> &str {
        match &self.event_type[..] {
            "NULL" => "NULL",
            _ => &self.event_type,
        }
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.payload {
            Some(payload) => write!(f, "{}({})", self.event_type, payload),
            None => write!(f, "{}", self.event_type),
        }
    }
}

/// Trait for types that can be converted into an event
pub trait IntoEvent {
    /// Convert into an event
    fn into_event(self) -> Event;
}

impl IntoEvent for Event {
    fn into_event(self) -> Event {
        self
    }
}

impl IntoEvent for &str {
    fn into_event(self) -> Event {
        Event::new(self)
    }
}

impl IntoEvent for String {
    fn into_event(self) -> Event {
        Event::new(self)
    }
}

impl IntoEvent for &String {
    fn into_event(self) -> Event {
        Event::new(self)
    }
}

/// A wildcard event that matches any event
pub const WILDCARD_EVENT: &str = "*";
