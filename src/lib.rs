pub mod error;
pub mod state;
pub mod event;
pub mod context;
pub mod transition;
pub mod guard;
pub mod action;
pub mod machine;

pub use {
    action::{Action, ActionType, IntoAction},
    context::Context,
    error::Error,
    event::{Event, IntoEvent},
    guard::{Guard, IntoGuard},
    machine::{Machine, MachineBuilder},
    state::{State, StateType},
    transition::Transition,
};

/// Result type for operations that can fail
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
