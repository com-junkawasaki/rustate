//!
//! # RuState: A State Machine Framework for Rust
//! 
//! RuState provides a robust and flexible framework for building state machines in Rust.
//! It is inspired by concepts from XState and aims to offer type safety, composability,
//! and testability for managing complex state logic.
//! 
//! ## Core Concepts
//! 
//! - **Machine**: The main container for state machine definitions, including states, transitions, context, events, actions, and guards.
//! - **State**: Represents a specific condition or situation the machine can be in.
//! - **Event**: An occurrence that can trigger a state transition.
//! - **Transition**: Defines the move from one state to another based on an event, potentially guarded by conditions and executing actions.
//! - **Context**: Arbitrary data associated with the machine, which can be updated during transitions.
//! - **Action**: Side effects (e.g., I/O, state updates) executed during transitions or state entry/exit.
//! - **Guard**: Conditions that must be met for a transition to occur.
//! 
//! ## Features
//! 
//! - **Declarative Syntax**: Define state machines using a clear and concise builder pattern.
//! - **Type Safety**: Leverages Rust's type system to catch errors at compile time.
//! - **Async Support**: Built with asynchronous operations in mind (using `async-trait`).
//! - **Composability**: Design complex systems by composing smaller machines (requires `integration` feature).
//! - **Testing Utilities**: Includes features for Model-Based Testing (`mbt`) and Property-Based Testing (`property-testing`).
//! - **Codegen**: Generate state machine code from definitions (requires `codegen` feature).
//! - **WASM Support**: Compile state machines for WebAssembly environments (requires `wasm` feature).
//! 
//! ## Optional Features
//! 
//! - `integration`: Enables patterns for integrating multiple state machines (event forwarding, shared context, hierarchical machines).
//! - `codegen`: Provides tools for code generation based on machine definitions.
//! - `wasm`: Adds necessary bindings and utilities for compiling to WebAssembly.
//! - `mbt`: Enables Model-Based Testing tools.
//! - `property-testing`: Enables Property-Based Testing tools.
//! - `full`: Enables all optional features.
//! 
//! ## Example
//! 
//! ```rust
//! // (Requires adding rustate to Cargo.toml)
//! use rustate::prelude::*;
//! use serde::{Deserialize, Serialize};
//! use async_trait::async_trait;
//! 
//! // 1. Define State, Event, Context
//! #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
//! enum LightState { Green, Yellow, Red }
//! 
//! #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
//! enum LightEvent { Timer }
//! 
//! #[derive(Debug, Clone, Default, Serialize, Deserialize)]
//! struct LightContext { count: u32 }
//! 
//! // 2. Implement StateTrait, EventTrait (often derived)
//! impl StateTrait for LightState {}
//! impl EventTrait for LightEvent {}
//! 
//! // 3. Build the machine
//! #[tokio::main]
//! async fn main() -> Result<()> { // Use rustate::Result
//!     let mut machine = MachineBuilder::<LightState, LightEvent, LightContext>::new(LightState::Red)
//!         .state(LightState::Red, |s| s
//!             .on(LightEvent::Timer, |t| t.target(LightState::Green))
//!         )
//!         .state(LightState::Yellow, |s| s
//!             .on(LightEvent::Timer, |t| t.target(LightState::Red))
//!         )
//!         .state(LightState::Green, |s| s
//!             .on(LightEvent::Timer, |t| t.target(LightState::Yellow))
//!         )
//!         .build()?;
//! 
//!     println!("Initial state: {:?}", machine.current_state());
//!     machine.send(LightEvent::Timer).await?;
//!     println!("After Timer: {:?}", machine.current_state());
//!     machine.send(LightEvent::Timer).await?;
//!     println!("After Timer: {:?}", machine.current_state());
//! 
//!     Ok(())
//! }
//! 
//! 
//! pub mod prelude {
//!     pub use crate::{
//!         Action, Context, Error, Event, EventTrait, Guard, IntoAction, IntoEvent, IntoGuard,
//!         Machine, MachineBuilder, Result, State, StateTrait, StateType, Transition,
//!         // Optional features re-exported for convenience
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         CoverageReport,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         ModelChecker,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         Property,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         PropertyType,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         TestCase,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         TestGenerator,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         TestResult,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         TestResults,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         TestRunner,
//!         #[cfg(any(feature = "mbt", feature = "property-testing"))]
//!         VerificationResult,
//!         #[cfg(feature = "property-testing")]
//!         EventSequenceStrategyBuilder,
//!         #[cfg(feature = "property-testing")]
//!         PropertyTestResult,
//!         #[cfg(feature = "property-testing")]
//!         PropertyTestRunner,
//!         #[cfg(feature = "property-testing")]
//!         StateMachineProperty,
//!         #[cfg(feature = "integration")]
//!         integration::context_sharing::SharedContext,
//!         #[cfg(feature = "integration")]
//!         integration::event_forwarding::SharedMachineRef,
//!         #[cfg(feature = "integration")]
//!         integration::hierarchical::ChildMachine,
//!         #[cfg(feature = "integration")]
//!         IntegrationError,
//!         #[cfg(feature = "integration")]
//!         IntegrationResult,
//!         #[cfg(feature = "codegen")]
//!         codegen::*,
//!         #[cfg(feature = "wasm")]
//!         wasm::*,
//!         ActorLogic, ActorStatus, Snapshot, serde_json
//!     };
//!     pub use async_trait::async_trait;
//! }
//! ```

// Private modules
mod action;
mod actor;
mod context;
mod error;
mod event;
mod guard;
#[cfg(any(feature = "mbt", feature = "property-testing"))]
mod test;

// Public modules
pub mod machine;
pub mod state;
pub mod transition;

// Conditionally compiled public modules/re-exports
#[cfg(feature = "wasm")]
mod wasm;
#[cfg(feature = "wasm")]
pub use wasm::*; // Re-export WASM specific items

#[cfg(feature = "codegen")]
mod codegen;
#[cfg(feature = "codegen")]
pub use codegen::*; // Re-export codegen specific items

/// Integration patterns for combining multiple state machines.
///
/// Provides utilities for:
/// - **Event Forwarding**: Sending events between machines.
/// - **Context Sharing**: Allowing machines to access shared data.
/// - **Hierarchical Composition**: Defining parent-child relationships between machines.
///
/// Enable this module with the `integration` feature flag.
#[cfg(feature = "integration")]
pub mod integration;

// Core public re-exports

/// Represents a side effect to be executed.
/// See [`action::Action`] and [`action::IntoAction`].
pub use action::{Action, IntoAction};
/// Represents the data associated with a state machine.
/// See [`context::Context`].
pub use context::Context;
/// The standard Result type for RuState operations, using [`Error`].
pub use error::Result;
/// The standard error type for RuState operations.
/// See [`error::StateError`].
pub use error::StateError as Error;
/// Represents an occurrence that can trigger a state transition.
/// See [`event::Event`], [`event::EventTrait`], and [`event::IntoEvent`].
pub use event::{Event, EventTrait, IntoEvent};
/// Represents a condition that must be met for a transition to occur.
/// See [`guard::Guard`] and [`guard::IntoGuard`].
pub use guard::{Guard, IntoGuard};
/// The main state machine structure and its builder.
/// See [`machine::Machine`] and [`machine::MachineBuilder`].
pub use machine::{Machine, MachineBuilder};
/// Represents a state within the machine.
/// See [`state::State`], [`state::StateTrait`], and [`state::StateType`].
pub use state::{State, StateTrait, StateType};
/// Represents a transition between states.
/// See [`transition::Transition`].
pub use transition::Transition;

// Actor model related re-exports
/// Encapsulates the logic of an actor (often a state machine).
/// See [`actor::ActorLogic`].
pub use actor::ActorLogic;
/// Represents the possible statuses of an actor.
/// See [`actor::ActorStatus`].
pub use actor::ActorStatus;
/// A snapshot of an actor's state and context at a point in time.
/// See [`actor::Snapshot`].
pub use actor::Snapshot;

// Testing features re-exports
#[cfg(any(feature = "mbt", feature = "property-testing"))]
pub use test::{ // Re-export all test items under a single use statement
    CoverageReport, ModelChecker, Property, PropertyType, TestCase, TestGenerator, TestResult,
    TestResults, TestRunner, VerificationResult,
};

// Property-based testing specific re-exports
#[cfg(feature = "property-testing")]
pub use test::{ // Re-export specific property-testing items
    EventSequenceStrategyBuilder, PropertyTestResult, PropertyTestRunner, StateMachineProperty,
};

// Integration features re-exports
#[cfg(feature = "integration")]
pub use integration::{ // Re-export integration items under a single use statement
    context_sharing::SharedContext, event_forwarding::SharedMachineRef, hierarchical::ChildMachine,
    Error as IntegrationError, Result as IntegrationResult,
};

// Re-export serde_json for convenience, as it's often used with Context/Snapshots.
pub use serde_json;

/// A prelude module for easily importing the most common RuState types.
/// ```
/// use rustate::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{
        action::{Action, IntoAction},
        actor::{ActorLogic, ActorStatus, Snapshot},
        context::Context,
        error::{Result, StateError as Error},
        event::{Event, EventTrait, IntoEvent},
        guard::{Guard, IntoGuard},
        machine::{Machine, MachineBuilder},
        state::{State, StateTrait, StateType},
        transition::Transition,
        // Optional testing features
        #[cfg(any(feature = "mbt", feature = "property-testing"))]
        test::{ // Keep testing items grouped
            CoverageReport, ModelChecker, Property, PropertyType, TestCase, TestGenerator,
            TestResult, TestResults, TestRunner, VerificationResult,
        },
        #[cfg(feature = "property-testing")]
        test::{ // Keep property-testing items grouped
            EventSequenceStrategyBuilder, PropertyTestResult, PropertyTestRunner,
            StateMachineProperty,
        },
        // Optional integration features
        #[cfg(feature = "integration")]
        integration::{
            context_sharing::SharedContext, event_forwarding::SharedMachineRef,
            hierarchical::ChildMachine, Error as IntegrationError, Result as IntegrationResult,
        },
        // Optional codegen features
        #[cfg(feature = "codegen")]
        codegen::*,
        // Optional wasm features
        #[cfg(feature = "wasm")]
        wasm::*,
        serde_json, // Re-export common dependency
    };
    // Re-export async_trait as it's needed for defining actions/guards/etc.
    pub use async_trait::async_trait;
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
