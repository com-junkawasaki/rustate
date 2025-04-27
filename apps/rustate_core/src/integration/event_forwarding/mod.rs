//! # Event Forwarding Integration Pattern
//!
//! ## Overview
//!
//! This module demonstrates the Event Forwarding pattern, a common integration strategy
//! for state machines. In this pattern, one state machine (`EventSource`) forwards
//! specific events to another state machine (`EventTarget`) for processing.
//!
//! ## Purpose
//!
//! - **Decoupling:** Allows state machines to react to events originating from other
//!   machines without direct dependencies on their internal logic.
//! - **Responsibility Segregation:** Enables specific state machines to handle particular
//!   types of events or tasks, promoting modularity.
//! - **Complex Workflows:** Facilitates building complex workflows where different parts
//!   of the system, represented by state machines, need to coordinate based on events.
//!
//! ## Components
//!
//! - **`EventSource` State Machine:** The machine that detects an event and decides
//!   to forward it. It typically uses an action associated with a transition or state
//!   to send the event.
//! - **`EventTarget` State Machine:** The machine that receives and processes the
//!   forwarded event. It has transitions defined to handle the specific event type.
//! - **Event:** The data or signal being forwarded between the machines.
//!
//! ## Implementation Notes
//!
//! - The `EventSource` machine needs a way to reference or access the `EventTarget`
//!   machine's `Sender` or `Context` to dispatch the event. This could be through
//!   shared context, dependency injection, or a dedicated communication channel.
//! - Ensure the event types are compatible or properly mapped between the source
//!   and target machines.
//! - **Warning:** Avoid circular dependencies where machines forward events back and
//!   forth indefinitely. This can lead to deadlocks or infinite loops. Design the
//!   forwarding logic carefully to ensure termination.
//!
//! ## Example
//!
//! The example below shows a simple scenario:
//!
//! 1.  `SourceMachine` transitions from `Idle` to `Processing` upon receiving `StartProcessing`.
//! 2.  In an `entry` action associated with the `Processing` state, it forwards a
//!     `DataReady` event to the `TargetMachine`.
//! 3.  `TargetMachine` receives `DataReady` and transitions from `Waiting` to `HandlingData`.

/// Example event enum used by both source and target machines.
/// In real-world scenarios, these might be distinct enums requiring mapping.
#[derive(Debug, Clone, PartialEq)]
// ... existing code ...

/// State machine that originates and forwards an event.
#[derive(StateMachine, Debug, Clone)]
#[state_machine(
    // ... existing code ...
    states(Idle, Processing),
    // ... existing code ...
)]
// ... existing code ...
struct SourceMachine {
    // ... existing code ...
}

/// Action to forward the DataReady event to the target machine.
// ... existing code ...

/// State machine that receives and processes the forwarded event.
#[derive(StateMachine, Debug, Clone)]
#[state_machine(
    // ... existing code ...
)]
// ... existing code ...
struct TargetMachine {
    // ... existing code ...
}

#[cfg(test)]
mod tests {
    // ... existing code ...
    /// Tests the event forwarding mechanism between SourceMachine and TargetMachine.
    #[tokio::test]
    // ... existing code ...
        // Create the target machine and get its sender
        // ... existing code ...

        // Create the source machine, providing the target's sender
        // ... existing code ...

        // Ensure initial states
        // ... existing code ...

        // Send the initial event to the source machine
        // ... existing code ...

        // Check that the source machine transitioned
        // ... existing code ...

        // Allow time for the forwarded event to be processed
        // ... existing code ...

        // Check that the target machine received the forwarded event and transitioned
        // ... existing code ...
    }
} 