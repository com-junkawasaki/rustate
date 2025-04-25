use crate::actor::{Actor, ActorError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Represents the state of the `CounterActor`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CounterState {
    /// The current count value.
    pub count: i32,
}

/// Represents the events that the `CounterActor` can process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CounterEvent {
    /// An event instructing the actor to increment its count.
    Increment,
    /// An event instructing the actor to decrement its count.
    Decrement,
    /// An event instructing the actor to print its current count to the console.
    /// Note: A `Get` event typically requires a reply mechanism (ask pattern)
    ///       which is not implemented in the basic `Actor::receive` signature.
    ///       For demonstration, this variant just prints.
    Print,
}

/// A simple actor that maintains an integer counter.
///
/// This actor demonstrates the basic implementation of the `Actor` trait.
/// It manages a `CounterState` and responds to `CounterEvent`s
/// (`Increment`, `Decrement`, `Print`).
#[derive(Debug, Clone, Default)] // Default derive is convenient for initial_state
pub struct CounterActor;

#[async_trait]
impl Actor for CounterActor {
    type State = CounterState;
    type Event = CounterEvent;
    /// This actor does not produce specific external output, so `Output` is `()`.
    type Output = ();

    /// Returns the initial state of the counter (count = 0).
    fn initial_state(&self) -> Self::State {
        CounterState { count: 0 }
    }

    /// Handles incoming `CounterEvent`s and updates the state.
    async fn receive(
        &self,
        mut state: Self::State, // `mut` is needed to modify the count
        event: Self::Event,
    ) -> Result<Self::State, ActorError> {
        println!(
            "CounterActor received event: {:?}, current state: {:?}",
            event, state
        ); // Basic logging
        match event {
            CounterEvent::Increment => {
                state.count += 1;
                Ok(state) // Return the updated state
            }
            CounterEvent::Decrement => {
                state.count -= 1;
                Ok(state) // Return the updated state
            }
            CounterEvent::Print => {
                println!("Current count: {}", state.count); // Perform side-effect
                                                            // State doesn't change, return the current state
                Ok(state)
            }
        }
    }
}
