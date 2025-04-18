mod action;
mod context;
mod error;
mod event;
mod guard;
mod machine;
mod state;
mod transition;

#[cfg(feature = "wasm")]
mod wasm;

#[cfg(feature = "wasm")]
pub use wasm::*;

pub use action::{Action, ActionType, IntoAction};
pub use context::Context;
pub use error::{StateError as Error, Result};
pub use event::{Event, IntoEvent};
pub use guard::{Guard, IntoGuard};
pub use machine::{Machine, MachineBuilder};
pub use state::{State, StateType};
pub use transition::Transition;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
