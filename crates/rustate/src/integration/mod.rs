//!
//! # RuState Integration Patterns
//!
//! This module provides patterns and utilities for integrating multiple RuState
//! state machines, potentially across different crates or modules, in a type-safe manner.
//!
//! ## Core Integration Patterns
//!
//! 1.  **Event Forwarding (`event_forwarding`)**: Enables loosely coupled communication
//!     by allowing one state machine to send events to another via a shared reference
//!     ([`SharedMachineRef`]). This is useful when machines need to react to each other's
//!     milestones without sharing internal state.
//!
//! 2.  **Context Sharing (`context_sharing`)**: Allows multiple state machines to access
//!     and potentially modify a shared data structure ([`SharedContext`]). This facilitates
//!     tighter coordination where machines operate on common data.
//!
//! 3.  **Hierarchical Composition (`hierarchical`)**: Manages parent-child relationships
//!     between state machines. The parent can spawn, monitor, and interact with children
//!     through a defined trait ([`ChildMachine`]), promoting encapsulation.
//!
//! ## Usage
//!
//! Enable this module and its features by adding the `integration` feature
//! flag to your `rustate` dependency in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! rustate = { version = "0.x.y", features = ["integration"] }
//! ```
//!
//! ### Example: Event Forwarding
//!
//! ```rust
//! # #[cfg(feature = "integration")]
//! # {
//! use rustate::prelude::*;
//! use rustate::integration::SharedMachineRef;
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//!
//! // Define state, event, context (can be simple String/()) or custom types
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Child machine that activates
//!     let child_machine = MachineBuilder::<String, String, ()>::new("idle".to_string())
//!         .state("idle".to_string(), |s| s.on("ACTIVATE".to_string(), |t| t.target("active".to_string())))
//!         .state("active".to_string(), |_| {})
//!         .build()?;
//!
//!     // Create a shareable reference to the child
//!     let shared_child = SharedMachineRef::new(child_machine);
//!     let child_ref_for_action = shared_child.clone();
//!
//!     // Parent machine with an action to forward an event
//!     let mut parent_machine = MachineBuilder::<String, String, ()>::new("ready".to_string())
//!         .state("ready".to_string(), |s| s
//!             .on_entry(|action| action
//!                 .name("forwardActivate")
//!                 .call(move |_ctx: Arc<RwLock<()>>, _evt: &String| {
//!                     let child_ref = child_ref_for_action.clone(); // Clone Arc for async block
//!                     async move {
//!                         println!("Parent: Telling child to activate...");
//!                         if let Err(e) = child_ref.send("ACTIVATE".to_string()).await {
//!                             eprintln!("Parent: Failed to send activate to child: {}", e);
//!                             return Err(e); // Propagate error if needed
//!                         }
//!                         Ok(())
//!                     }
//!                 })
//!             )
//!         )
//!         .build()?;
//!
//!     println!("Child state before: {:?}", shared_child.get_snapshot().await?.value);
//!     // Entering the parent's "ready" state triggers the on_entry action
//!     parent_machine.start().await?; // Ensure machine starts and executes entry actions
//!
//!     tokio::time::sleep(std::time::Duration::from_millis(10)).await; // Allow time for event processing
//!
//!     println!("Child state after: {:?}", shared_child.get_snapshot().await?.value);
//!     assert_eq!(shared_child.get_snapshot().await?.value, serde_json::json!("active"));
//!
//!     Ok(())
//! }
//! # }
//! ```
//!
//! See the specific submodules (`context_sharing`, `event_forwarding`, `hierarchical`)
//! for more detailed examples and API documentation.

// Declare submodules
pub mod context_sharing;
pub mod event_forwarding;
pub mod hierarchical;

/// Error types specific to integration patterns.
pub mod error;
pub use error::{Error, LockResultExt, Result};

// Re-export key types for convenience

/// A reference-counted, thread-safe handle for accessing shared context data.
/// See [`context_sharing::SharedContext`].
pub use context_sharing::SharedContext;

/// A reference-counted, thread-safe handle for sending events to another machine.
/// See [`event_forwarding::SharedMachineRef`].
pub use event_forwarding::SharedMachineRef;

/// A trait defining the interface for a child state machine in hierarchical compositions.
/// See [`hierarchical::ChildMachine`].
pub use hierarchical::ChildMachine;
