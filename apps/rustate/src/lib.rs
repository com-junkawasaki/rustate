//!
//! Core components for the rustate actor framework.
//! This crate provides the fundamental traits, structs, and functions
//! for defining, spawning, and interacting with actors.
//! It forms the basis for building concurrent applications using the actor model.

// Module declarations
//
// The crate is split into two independent halves, each behind its own Cargo
// feature so users can compile only what they need (see the `state` / `actor`
// features in `Cargo.toml`):
//   * `actor` — the actor-model runtime (actor system, mailboxes, spawning).
//   * `state` — the XState-style state machine / statechart layer.
// Both are enabled by default; the `simple_counter` demo needs both.

// --- Actor model modules (feature = "actor") ---
#[cfg(feature = "actor")]
pub mod actor;
#[cfg(feature = "actor")]
pub mod actor_ref;
#[cfg(feature = "actor")]
pub mod logic;
#[cfg(feature = "actor")]
pub mod spawn;
#[cfg(feature = "actor")]
pub mod system;

// --- State machine modules (feature = "state") ---
#[cfg(feature = "state")]
pub mod action;
#[cfg(feature = "state")]
pub mod context;
#[cfg(feature = "state")]
pub mod error;
#[cfg(feature = "state")]
pub mod event;
#[cfg(feature = "state")]
pub mod guard;
#[cfg(feature = "state")]
pub mod machine;
#[cfg(feature = "state")]
pub mod state;
// pub mod state_registry; // Removed - seems unused/missing
#[cfg(feature = "state")]
pub mod transition;

// Demo actor that wires a state machine into an actor — needs both halves.
#[cfg(all(feature = "state", feature = "actor"))]
pub mod simple_counter;

#[cfg(feature = "codegen")]
pub mod codegen;
#[cfg(feature = "wasm")]
pub mod wasm;

// Integration patterns build on top of the state machine layer.
#[cfg(feature = "state")]
pub mod integration;

// Public re-exports for easier access by users of the crate.

/// The core trait defining actor behavior, state, events, and outputs.
/// See [`actor::Actor`] for details.
#[cfg(feature = "actor")]
pub use actor::Actor;

/// Enum representing errors that can occur within the actor system or during actor processing.
/// See [`actor::ActorError`] for variants.
#[cfg(feature = "actor")]
pub use actor::ActorError;

/// A reference (handle) to a spawned actor, used for sending events.
/// See [`actor_ref::ActorRef`] for details.
#[cfg(feature = "actor")]
pub use actor_ref::ActorRef;

/// A trait encapsulating the state transition logic (state machine behavior) of an actor.
/// Often implemented by code generated via macros.
/// See [`logic::ActorLogic`] for details.
#[cfg(feature = "actor")]
pub use logic::ActorLogic;

/// Spawns an actor with the default mailbox buffer size.
/// See [`spawn::spawn`] for details.
#[cfg(feature = "actor")]
pub use spawn::spawn;

/// Represents the actor system, the entry point for creating top-level actors.
/// See [`system::ActorSystem`] for details.
#[cfg(feature = "actor")]
pub use system::ActorSystem;

// Add re-exports for types needed by machine.rs or others
// pub use actor::{ActorStatus, Snapshot as ActorSnapshot}; // Added for MachineSnapshot - COMMENTED OUT
#[cfg(feature = "state")]
pub use event::{Event, IntoEvent}; // Removed duplicate EventTrait

// --- Add re-exports from obsolete crate (Review and merge carefully) ---

// Re-export core types from moved modules
#[cfg(feature = "state")]
pub use action::{Action, ActionType, IntoAction};
#[cfg(feature = "state")]
pub use context::Context;
#[cfg(feature = "state")]
pub use error::Result;
#[cfg(feature = "state")]
pub use error::StateError as Error;
#[cfg(feature = "state")]
pub use event::EventTrait;
#[cfg(feature = "state")]
pub use guard::{Guard, IntoGuard};
#[cfg(feature = "state")]
pub use machine::{Machine, MachineBuilder, MachineSnapshot};
#[cfg(feature = "state")]
pub use state::{HistoryType, State, StateCollection, StateTrait, StateType};
#[cfg(feature = "state")]
pub use transition::{Transition, TransitionType};

// Actor model related re-exports from obsolete's actor.rs (may conflict/need merging)
// Consider prefixing or carefully choosing which ones to expose
// pub use actor::{create_actor, ActorLogic, ActorRefImpl, ActorStatus, Snapshot as ActorSnapshot};

// Conditionally re-export based on features
#[cfg(feature = "wasm")]
pub use crate::wasm::*; // Re-export WASM specific items

#[cfg(feature = "codegen")]
pub use crate::codegen::*; // Re-export codegen specific items

// Re-export integration items (part of the state machine layer)
#[cfg(feature = "state")]
pub use crate::integration::{
    context_sharing::SharedContext,
    event_forwarding::SharedMachineRef, // Assuming SharedMachineRef exists
    hierarchical::{ChildMachine, DefaultChildMachine}, // Assuming these exist
    Error as IntegrationError,
    Result as IntegrationResult,
};

// Re-export serde_json for convenience (enabled together with the `state` feature)
#[cfg(feature = "serde_json")]
pub use serde_json;

// --- Consider creating or merging a `prelude` module ---
/*
pub mod prelude {
    // Combine re-exports from both original lib.rs files
    pub use crate::actor::Actor; // From core
    pub use crate::actor::ActorError; // From core
    pub use crate::actor_ref::ActorRef; // From core
    pub use crate::logic::ActorLogic; // From core
    pub use crate::spawn::spawn; // From core
    pub use crate::system::ActorSystem; // From core

    // From obsolete
    pub use crate::{action::Action, action::IntoAction};
    pub use crate::context::Context;
    pub use crate::error::Result;
    pub use crate::error::StateError as Error;
    pub use crate::event::{Event, EventTrait, IntoEvent};
    pub use crate::guard::{Guard, IntoGuard};
    pub use crate::machine::{Machine, MachineBuilder};
    pub use crate::state::{State, StateTrait, StateType};
    pub use crate::transition::Transition;

    // ... add conditional exports for features (wasm, codegen, integration, test) ...

    pub use async_trait::async_trait;
    pub use serde_json;
}
*/

// Tests module (exercises the counter actor, which needs both halves)
#[cfg(all(test, feature = "state", feature = "actor"))]
mod tests {
    // Re-import necessary items for tests
    use super::*;
    use simple_counter::{CounterActor, CounterEvent};
    use tokio::time::{sleep, Duration};

    // --- Test for original counter actor using ActorSystem ---
    #[tokio::test]
    async fn test_counter_actor_with_system() {
        println!("Creating ActorSystem...");
        let system = ActorSystem::new("test-system");
        println!("ActorSystem created: {:?}", system);

        println!("Spawning CounterActor using system...");
        // Use spawn and the async constructor, provide buffer size, remove incorrect await
        let counter_ref = system.spawn(CounterActor::new().await, 100); // Add buffer size, remove trailing .await
        println!("CounterActor spawned with ref: {:?}", counter_ref);

        // Allow time for the actor to start
        sleep(Duration::from_millis(10)).await;

        println!("Sending Increment event...");
        let res1 = counter_ref.send(CounterEvent::Increment).await;
        assert!(res1.is_ok(), "Failed to send Increment (1)");
        println!("Increment event sent.");

        // Allow time for processing
        sleep(Duration::from_millis(10)).await;

        println!("Sending Increment event again...");
        let res2 = counter_ref.send(CounterEvent::Increment).await;
        assert!(res2.is_ok(), "Failed to send Increment (2)");
        println!("Increment event sent.");

        // Allow time for processing
        sleep(Duration::from_millis(10)).await;

        println!("Sending Print event...");
        let res3 = counter_ref.send(CounterEvent::Print).await;
        assert!(res3.is_ok(), "Failed to send Print");
        println!("Print event sent.");

        // Allow more time for the print action to complete
        sleep(Duration::from_millis(50)).await;

        // NOTE: Verification of the final state currently relies on checking logs.
        // A proper test would use an "ask" pattern or other mechanism to query state.
        // For example, modify spawn to return a way to query state or add an Ask message.
        println!("Original counter test finished. Verify logs for state changes (e.g., count should be 2).");
    }

    // --- Tests for the create_machine macro (commented out as it depends on rustate_macros) ---
    /*
    mod machine_macro_tests {
        use crate::logic::ActorLogic;
        use crate::ActorError;
        use rustate_macros::create_machine; // Requires rustate_macros crate
        use serde::{Deserialize, Serialize};
        use async_trait::async_trait; // Required by generated code
        use std::fmt::Debug;

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        enum TestState {
            Idle,
            Running,
            Finished,
        }

        #[derive(Debug, Clone, Serialize, Deserialize)] // Event doesn't strictly need PartialEq/Eq
        enum TestEvent {
            Start,
            Finish,
            Reset,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
        struct TestContext {
            count: i32,
            // Add another field for testing initial context
            name: String,
        }

        // Define the machine using the macro
        create_machine!(
            MyTestMachine,
            Context: TestContext,
            Event: TestEvent,
            State: TestState,
            initial: TestState::Idle { // Provide initial context values here
                count: 0,
                name: "Initial Name".to_string()
            },
            states: {
                Idle { on: { Start: "Running" } },
                Running { on: { Finish: "Finished", Reset: "Idle" } },
                Finished { on: { Reset: "Idle" } }
            }
        );

        #[test]
        fn test_initial_state_and_context() {
            let machine = MyTestMachine::default(); // Logic struct itself is stateless
            let (initial_state, initial_context) = machine.initial();
            assert_eq!(initial_state, TestState::Idle);
            assert_eq!(initial_context, TestContext { count: 0, name: "Initial Name".to_string() });
        }

        #[tokio::test]
        async fn test_transitions() {
            let machine = MyTestMachine::default();
            let initial_context = machine.initial().1; // Get initial context from the logic

            // Test Idle -> Running
            let (state1, context1) = machine
                .transition(TestState::Idle, initial_context.clone(), TestEvent::Start)
                .await
                .expect("Transition Idle -> Running failed");
            assert_eq!(state1, TestState::Running);
            assert_eq!(context1, initial_context); // Context unchanged by default

            // Test Running -> Finished
            let (state2, context2) = machine
                .transition(state1, context1.clone(), TestEvent::Finish)
                .await
                .expect("Transition Running -> Finished failed");
            assert_eq!(state2, TestState::Finished);
            assert_eq!(context2, context1);

            // Test Finished -> Idle (Reset)
            let (state3, context3) = machine
                .transition(state2, context2.clone(), TestEvent::Reset)
                .await
                .expect("Transition Finished -> Idle failed");
            assert_eq!(state3, TestState::Idle);
            assert_eq!(context3, context2);

            // Test Running -> Idle (Reset) - starting from Running state
            let (state4, context4) = machine
                .transition(TestState::Running, initial_context.clone(), TestEvent::Reset)
                .await
                .expect("Transition Running -> Idle failed");
            assert_eq!(state4, TestState::Idle);
            assert_eq!(context4, initial_context);

            // Test no transition (e.g., Start event in Finished state)
            let (state5, context5) = machine
                .transition(TestState::Finished, initial_context.clone(), TestEvent::Start)
                .await
                .expect("Transition Finished -> Start (no op) failed");
            assert_eq!(state5, TestState::Finished); // Should remain in Finished
            assert_eq!(context5, initial_context);

            // Test no transition (e.g., Reset event in Idle state)
             let (state6, context6) = machine
                .transition(TestState::Idle, initial_context.clone(), TestEvent::Reset)
                .await
                .expect("Transition Idle -> Reset (no op) failed");
            assert_eq!(state6, TestState::Idle); // Should remain in Idle
            assert_eq!(context6, initial_context);
        }
    }
    */
} // End of tests module
