use crate::actor::ActorError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Defines the core logic for state transitions, actions, and guards within an actor.
///
/// This trait encapsulates the behavior of a state machine. It determines how the actor
/// moves between states and updates its context based on incoming events.
///
/// Typically, the `create_machine` macro (planned for Phase 2) will automatically generate
/// a struct implementing this trait based on a declarative state machine definition.
/// Manual implementation is possible but the macro approach is generally preferred for
/// consistency and reduced boilerplate.
#[async_trait]
pub trait ActorLogic: Send + Sync + 'static {
    /// The type representing the actor's context (extended state).
    /// This is the data the state machine holds and potentially modifies during transitions.
    /// Context must be serializable, deserializable, cloneable, debuggable, sendable, and syncable.
    type Context: Send + Sync + Clone + Debug + Serialize + for<'de> Deserialize<'de>;

    /// The type representing the events that the actor logic processes.
    /// This should typically match the `Event` type defined in the corresponding `Actor` trait implementation.
    /// Events must be debuggable, sendable, syncable, serializable, and deserializable.
    type Event: Send + Sync + Debug + Serialize + for<'de> Deserialize<'de>;

    /// The type representing the possible states of the actor (usually an enum).
    /// State must be serializable, deserializable, cloneable, debuggable, equatable, sendable, and syncable.
    type State: Send + Sync + Clone + Debug + PartialEq + Eq + Serialize + for<'de> Deserialize<'de>;

    /// Returns the initial state and initial context for the state machine.
    ///
    /// This is called when the actor associated with this logic is started.
    fn initial(&self) -> (Self::State, Self::Context);

    /// Executes a state transition based on the current state, context, and a received event.
    ///
    /// This is the core method where the state machine's transition rules are applied.
    /// It may involve evaluating guards, executing actions, and determining the next state.
    ///
    /// # Arguments
    ///
    /// * `state` - The current state of the machine.
    /// * `context` - The current context (data) associated with the state.
    /// * `event` - The event that triggered the potential transition.
    ///
    /// # Returns
    ///
    /// A `Result` containing:
    /// * `Ok((Self::State, Self::Context))` - The new state and potentially updated context after the transition.
    /// * `Err(ActorError)` - If an error occurred during the transition logic (e.g., an action failed).
    ///
    /// If no transition is defined for the given event in the current state (or if a guard condition fails),
    /// this method should typically return `Ok((state, context))` (the current state and context unchanged).
    /// A specific `ActorError` variant could potentially be used to indicate "no transition occurred",
    /// but returning the current state is often sufficient.
    async fn transition(
        &self,
        state: Self::State,
        context: Self::Context,
        event: Self::Event,
    ) -> Result<(Self::State, Self::Context), ActorError>;

    // --- Potential Future Enhancements ---

    /// Executes actions upon entering a state.
    // async fn on_entry(&self, state: &Self::State, context: &mut Self::Context) -> Result<(), ActorError> { Ok(()) }

    /// Executes actions upon exiting a state.
    // async fn on_exit(&self, state: &Self::State, context: &mut Self::Context) -> Result<(), ActorError> { Ok(()) }

    /// Evaluates guard conditions before executing a transition.
    // async fn check_guard(&self, state: &Self::State, context: &Self::Context, event: &Self::Event) -> bool { true }

    /// Executes actions associated with a specific transition.
    // async fn execute_action(&self, state: &Self::State, context: &mut Self::Context, event: &Self::Event) -> Result<(), ActorError> { Ok(()) }
}

// --- Example Dummy Implementation ---
/*
struct MyMachineLogic;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum MyState { Idle, Processing }

#[async_trait]
impl ActorLogic for MyMachineLogic {
    type Context = i32;
    type Event = String;
    type State = MyState;

    fn initial(&self) -> (Self::State, Self::Context) {
        (MyState::Idle, 0)
    }

    async fn transition(&self, state: Self::State, context: Self::Context, event: Self::Event)
        -> Result<(Self::State, Self::Context), ActorError>
    {
        println!("Transitioning from {:?} with context {} on event \"{}\"", state, context, event);
        match (state, event.as_str()) {
            (MyState::Idle, "START") => Ok((MyState::Processing, context + 1)),
            (MyState::Processing, "STOP") => Ok((MyState::Idle, context)),
            (s, _) => Ok((s, context)), // No change for other events
        }
    }
}
*/
