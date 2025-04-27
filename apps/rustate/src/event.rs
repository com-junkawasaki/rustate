//!
//! Defines the core concepts related to events in the RuState framework.
//!
//! Events are occurrences that can trigger state transitions within a state machine.

use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::Hash;

/// A trait defining the common behavior for all event types used in RuState.
///
/// Any type used as an event in a `Machine` must implement this trait.
/// It ensures events can be identified, potentially carry data, and satisfy
/// necessary bounds for use in the framework (cloning, debugging, equality,
/// thread-safety).
pub trait EventTrait: Clone + Debug + PartialEq + Eq + Hash + Send + Sync + 'static {
    /// Returns a string slice representing the type or category of the event.
    /// Used for matching transitions defined with string identifiers.
    ///
    /// Example: "TIMER_ELAPSED", "USER_CLICK"
    fn event_type(&self) -> &str;

    /// Returns an optional reference to the event's payload data.
    /// The payload is represented as a `serde_json::Value` for flexibility,
    /// allowing arbitrary data structures to be associated with an event.
    fn payload(&self) -> Option<&serde_json::Value>;

    /// Returns a name or identifier for the specific event instance.
    /// Often, this can be the same as `event_type`, but allows for more specific
    /// identification if needed (e.g., distinguishing different instances of the same type).
    fn name(&self) -> &str;
}

/// A concrete representation of an event.
///
/// This struct provides a common way to represent events with a string `event_type`
/// and an optional `serde_json::Value` payload.
/// It implements [`EventTrait`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub struct Event {
    /// The type identifier for the event (e.g., "SUBMIT", "CANCEL").
    /// Renamed to `type` during JSON serialization for convention.
    #[serde(rename = "type")]
    #[serde(default)]
    pub event_type: String,
    /// Optional data associated with the event, represented as a JSON Value.
    #[serde(skip_serializing_if = "Option::is_none")] // Don't serialize if None
    #[serde(default)] // Ensure None is used if missing during deserialization
    pub payload: Option<serde_json::Value>,
}

impl Event {
    /// Creates a new `Event` with the given type and no payload.
    ///
    /// # Arguments
    /// * `event_type` - A string slice representing the event type.
    pub fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            payload: None,
        }
    }

    /// Creates a new `Event` with the given type and payload.
    ///
    /// # Arguments
    /// * `event_type` - A string slice representing the event type.
    /// * `payload` - A `serde_json::Value` containing the event data.
    pub fn with_payload(event_type: &str, payload: serde_json::Value) -> Self {
        Self {
            event_type: event_type.to_string(),
            payload: Some(payload),
        }
    }

    /// Returns a mutable reference to the optional payload.
    pub fn payload_mut(&mut self) -> Option<&mut serde_json::Value> {
        self.payload.as_mut()
    }
}

// Implement the core EventTrait for the concrete Event struct.
impl EventTrait for Event {
    fn event_type(&self) -> &str {
        &self.event_type
    }

    fn payload(&self) -> Option<&serde_json::Value> {
        self.payload.as_ref()
    }

    /// Returns the event type as the name for the concrete `Event` struct.
    fn name(&self) -> &str {
        // Special case handled here, but generally just returns the type.
        // Consider if the "NULL" special case is still needed.
        // match &self.event_type[..] {
        //     "NULL" => "NULL",
        //     _ => &self.event_type,
        // }
        &self.event_type
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.payload {
            Some(_payload) => write!(f, "{}(...)", self.event_type),
            None => write!(f, "{}", self.event_type),
        }
    }
}

/// A trait for types that can be conveniently converted into an [`Event`].
///
/// This is useful for defining transitions using simple types like string slices.
pub trait IntoEvent {
    /// Performs the conversion into an `Event`.
    fn into_event(self) -> Event;
}

// Allow converting an existing Event into an Event (identity conversion).
impl IntoEvent for Event {
    fn into_event(self) -> Event {
        self
    }
}

// Allow converting string slices into an Event with no payload.
impl IntoEvent for &str {
    fn into_event(self) -> Event {
        Event::new(self)
    }
}

// Implement From for convenience, consistent with IntoEvent.
impl From<&str> for Event {
    fn from(s: &str) -> Self {
        Event::new(s)
    }
}

// Allow converting owned Strings into an Event with no payload.
impl IntoEvent for String {
    fn into_event(self) -> Event {
        Event::new(&self)
    }
}

/// Represents a wildcard event often used in transitions to match any event.
/// This is typically used for default transitions when no specific event matches.
pub const WILDCARD_EVENT: &str = "*";
