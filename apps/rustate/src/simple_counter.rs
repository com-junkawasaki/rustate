use crate::actor::{Actor, ActorError};
use crate::context::Context;
use crate::event::{Event, EventTrait, IntoEvent};
use crate::integration::error::Error as IntegrationError;
use crate::machine::Machine;
use crate::state::State;
use crate::MachineBuilder;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

/// Represents the state of the `CounterActor`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum CounterEvent {
    /// An event instructing the actor to increment its count.
    #[default]
    Increment,
    /// An event instructing the actor to decrement its count.
    Decrement,
    /// An event instructing the actor to print its current count to the console.
    /// Note: A `Get` event typically requires a reply mechanism (ask pattern)
    ///       which is not implemented in the basic `Actor::receive` signature.
    ///       For demonstration, this variant just prints.
    Print,
}

/// Implement EventTrait for CounterEvent
impl EventTrait for CounterEvent {
    fn event_type(&self) -> &str {
        match self {
            CounterEvent::Increment => "INCREMENT",
            CounterEvent::Decrement => "DECREMENT",
            CounterEvent::Print => "PRINT",
        }
    }

    fn payload(&self) -> Option<&serde_json::Value> {
        None // CounterEvent has no payload
    }

    fn name(&self) -> &str {
        self.event_type() // Use the event type as the name
    }
}

/// Implement IntoEvent for CounterEvent
impl IntoEvent for CounterEvent {
    fn into_event(self) -> Event {
        // Convert CounterEvent to a payload-less Event using its type name
        Event::new(self.event_type())
    }
}

/// Define the actor state
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CounterState {
    /// The current count value.
    pub count: i32,
}

/// A simple actor that maintains an integer counter.
///
/// This actor demonstrates the basic implementation of the `Actor` trait.
/// It manages a `CounterState` and responds to `CounterEvent`s
/// (`Increment`, `Decrement`, `Print`).
#[derive(Debug, Clone)]
pub struct CounterActor {
    state: Arc<Mutex<CounterState>>,
    _machine: Arc<Mutex<Machine<Context, CounterEvent, String, ()>>>,
}

impl CounterActor {
    /// Constructor now async to allow building the machine
    pub async fn new() -> Self {
        let initial_state = CounterState::default();

        // Attempt to build a minimal machine - still needs proper config
        let machine_result = MachineBuilder::<Context, CounterEvent, String, ()>::new(
            "counter_machine".to_string(),
            "Idle".to_string(),
        )
        .state(State::new("Idle".to_string())) // Need at least one state
        .context(Context::new())
        .build()
        .await;

        let machine = match machine_result {
            Ok(m) => m,
            Err(e) => {
                // Handle error properly - maybe panic or return Result<Self, _>
                panic!("Failed to build dummy machine: {}", e);
            }
        };

        Self {
            state: Arc::new(Mutex::new(initial_state)),
            _machine: Arc::new(Mutex::new(machine)),
        }
    }

    pub fn get_state(&self) -> Result<CounterState, IntegrationError> {
        let state = self
            .state
            .lock()
            .map_err(|_e: PoisonError<_>| IntegrationError::LockError)?;
        Ok(state.clone())
    }

    pub fn increment(&self) -> Result<(), IntegrationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_e: PoisonError<_>| IntegrationError::LockError)?;
        state.count += 1;
        Ok(())
    }

    pub fn decrement(&self) -> Result<(), IntegrationError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_e: PoisonError<_>| IntegrationError::LockError)?;
        state.count -= 1;
        Ok(())
    }
}

#[async_trait]
impl Actor for CounterActor {
    type Context = Context;
    type StateId = String;
    type Event = CounterEvent;
    type Output = ();
    type State = CounterState;

    /// Implement initial_state from Actor trait
    fn initial_state(&self) -> Self::State {
        CounterState::default()
    }

    /// Implement receive from Actor trait
    async fn receive(
        &self,
        state: Self::State, // Current state passed in
        event: Self::Event,
    ) -> Result<Self::State, ActorError> {
        log::debug!(
            "CounterActor executing receive for event: {:?} with state: {:?}",
            event,
            state
        );

        // --- Machine Interaction (Optional based on design) ---
        // If state transitions are driven by the machine:
        // 1. Convert specific event to generic Event if needed by machine.
        // 2. Send event to machine.
        // 3. Machine executes actions/transitions which might update context.
        // 4. Potentially read updated context from machine to influence state update.
        // NOTE: The current structure with both `receive` and an internal `machine`
        // might be redundant or indicate a different pattern is needed.
        // For now, `receive` will directly update the state like `handle_event` did.
        // If machine should drive state, this logic needs refactoring.

        let mut new_state = state.clone(); // Clone current state to modify

        match event {
            CounterEvent::Increment => {
                new_state.count += 1;
                log::info!("Counter incremented: {}", new_state.count);
            }
            CounterEvent::Decrement => {
                new_state.count -= 1;
                log::info!("Counter decremented: {}", new_state.count);
            }
            CounterEvent::Print => {
                log::info!("Current count (in receive): {}", new_state.count);
                // Print doesn't change state
            }
        }

        // Return the potentially modified state
        Ok(new_state)
    }

    // Removed send, get_state, handle_event as they are not part of Actor trait
}
