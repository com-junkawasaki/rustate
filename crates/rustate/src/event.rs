use serde::{Deserialize, Serialize};
use std::fmt;

/// Trait for event objects in a state machine
pub trait EventTrait {
    /// Get the event type
    fn event_type(&self) -> &str;
    
    /// Get the payload data, if any
    fn payload(&self) -> Option<&serde_json::Value>;
}

/// Represents an event that can trigger state transitions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// The event type
    pub event_type: String,
    /// Optional payload data
    pub payload: Option<serde_json::Value>,
}

impl Event {
    /// Create a new event
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            payload: None,
        }
    }

    /// Create a new event with payload
    pub fn with_payload(event_type: impl Into<String>, payload: impl Into<serde_json::Value>) -> Self {
        Self {
            event_type: event_type.into(),
            payload: Some(payload.into()),
        }
    }
}

impl EventTrait for Event {
    fn event_type(&self) -> &str {
        &self.event_type
    }
    
    fn payload(&self) -> Option<&serde_json::Value> {
        self.payload.as_ref()
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

impl<'a> IntoEvent for &'a String {
    fn into_event(self) -> Event {
        Event::new(self)
    }
}

/// A wildcard event that matches any event
pub const WILDCARD_EVENT: &str = "*"; 