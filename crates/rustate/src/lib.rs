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

// クレート統合用モジュール
#[cfg(feature = "integration")]
pub mod integration;

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

// 統合パターンの機能をエクスポート
#[cfg(feature = "integration")]
pub use integration::{
    event_forwarding::SharedMachineRef,
    context_sharing::SharedContext,
    hierarchical::ChildMachine,
    Error as IntegrationError,
    Result as IntegrationResult,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
