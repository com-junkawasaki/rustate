mod action;
mod context;
mod error;
mod event;
mod guard;
pub mod machine;
pub mod state;
mod test;
pub mod transition;

#[cfg(feature = "wasm")]
mod wasm;

#[cfg(feature = "wasm")]
pub use wasm::*;

pub use action::{Action, ActionType, IntoAction};
pub use context::Context;
pub use error::{Result, StateError as Error};
pub use event::{Event, EventTrait, IntoEvent};
pub use guard::{Guard, IntoGuard};
pub use machine::{Machine, MachineBuilder};
pub use state::{State, StateTrait, StateType};
pub use transition::Transition;

// モデルベーステストの機能をエクスポート
pub use test::{
    CoverageReport, ModelChecker, Property, PropertyType, TestCase, TestGenerator, TestResult,
    TestResults, TestRunner, VerificationResult,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
