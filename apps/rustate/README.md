# rustate Crate

This crate contains the core RuState state machine library.

## Overview

RuState provides the following features:

- ✅ Finite state machines and statecharts
- ✅ Hierarchical states
- ✅ Parallel states
- ✅ Transition conditions (guards)
- ✅ Actions (side effects)
- ✅ Context (extended state)
- ✅ Type-safe API
- ✅ Serializable machines (State snapshot only; actions/guards are not fully serialized)
- ✅ Cross-crate state machine integration (via shared memory patterns)
- ✅ Actor pattern support (Basic: Actor trait, ActorRef, spawn, system)
- ❌ Model-based testing (MBT) support (Planned, not implemented)

## Cargo Features

The state machine and the actor model are split into two independent features so
you only compile (and depend on) the part you actually use. Both are enabled by
default, so existing dependencies keep working unchanged.

| Feature | Enables | Notes |
| --- | --- | --- |
| `state` | Statecharts, guards, actions, context, transitions, integration patterns, code generation target | Only needs Tokio's `sync` primitives. |
| `actor` | Actor trait, `ActorRef`, `spawn`, `ActorSystem` | Pulls in the full Tokio runtime (`rt-multi-thread`, `macros`). |

```toml
# Everything (default)
rustate = "0.3"

# Just the state machine — no actor runtime / lighter Tokio footprint
rustate = { version = "0.3", default-features = false, features = ["state"] }

# Just the actor model
rustate = { version = "0.3", default-features = false, features = ["actor"] }
```

Other features (`codegen`, `xstate-compat`, `wasm`, `integration`, `proto`) build on
top of `state` and enable it automatically.

## Model-Based Testing (MBT) Integration

TODO: Implement MBT integration. (Currently not implemented)

## Cross-Crate State Machine Integration

*(Current approach)* RuState primarily supports integrating multiple state machines across different crates using shared memory (`Arc<Mutex>`, `Arc<RwLock>`), allowing state machines within the same process to communicate via shared context or event forwarding.

### Design Patterns for State Machine Integration (Shared Memory)

1.  **Event Forwarding Pattern**: State machines communicate by forwarding events to each other

    ```rust
    use rustate::{Action, Context, Event, Machine, MachineBuilder, State, Transition};
    use std::sync::{Arc, Mutex};

    // Define a shared state machine in a common crate
    pub struct SharedMachineRef {
        machine: Arc<Mutex<Machine>>,
    }

    impl SharedMachineRef {
        pub fn new(machine: Machine) -> Self {
            Self {
                machine: Arc::new(Mutex::new(machine)),
            }
        }
        
        pub fn send_event(&self, event: &str) -> rustate::Result<bool> {
            let mut machine = self.machine.lock().unwrap();
            machine.send(event)
        }
    }

    // In crate A: Create a parent machine that forwards events to child
    fn setup_parent_machine(child_machine: SharedMachineRef) -> Machine {
        let parent_state = State::new("parent");
        
        // Define action that forwards events to child machine
        let forward_to_child = Action::new(
            "forwardToChild",
            ActionType::Transition,
            move |_ctx, evt| {
                if evt.event_type == "CHILD_EVENT" {
                    let _ = child_machine.send_event("HANDLE_EVENT");
                }
            },
        );
        
        MachineBuilder::new("parentMachine")
            .state(parent_state)
            .initial("parent")
            .on_entry("parent", forward_to_child)
            .build()
            .unwrap()
    }
    ```

2.  **Context-Based Communication Pattern**: Share data between machines using Context

    ```rust
    use rustate::{Context, Machine, MachineBuilder, State, Transition};
    use std::sync::{Arc, RwLock};

    // Define shared context type in a common crate
    #[derive(Clone, Default)]
    pub struct SharedContext {
        data: Arc<RwLock<serde_json::Value>>,
    }

    impl SharedContext {
        pub fn new() -> Self {
            Self {
                data: Arc::new(RwLock::new(serde_json::json!({}))),
            }
        }
        
        pub fn set<T: serde::Serialize>(&self, key: &str, value: T) -> Result<(), serde_json::Error> {
            let mut data = self.data.write().unwrap();
            match &mut *data {
                serde_json::Value::Object(map) => {
                    map.insert(key.to_string(), serde_json::to_value(value)?);
                    Ok(())
                }
                _ => {
                    *data = serde_json::json!({ key: value });
                    Ok(())
                }
            }
        }
        
        pub fn get<T: for<'de> serde::Deserialize<'de>>(&self, key: &str) -> Option<T> {
            let data = self.data.read().unwrap();
            match &*data {
                serde_json::Value::Object(map) => map
                    .get(key)
                    .and_then(|val| serde_json::from_value(val.clone()).ok()),
                _ => None,
            }
        }
    }

    // Use in machine actions across different crates
    fn create_machines(shared_context: SharedContext) -> (Machine, Machine) {
        // Machine in crate A
        let machine_a = MachineBuilder::new("machineA")
            // ...setup states and transitions...
            .on_entry("someState", move |ctx, _evt| {
                // Read shared context data
                if let Some(value) = shared_context.get::<String>("status") {
                    ctx.set("localStatus", value).unwrap();
                }
            })
            .build()
            .unwrap();
            
        // Machine in crate B
        let machine_b = MachineBuilder::new("machineB")
            // ...setup states and transitions...
            .on_entry("anotherState", move |_ctx, _evt| {
                // Update shared context
                shared_context.set("status", "active").unwrap();
            })
            .build()
            .unwrap();
            
        (machine_a, machine_b)
    }
    ```

3.  **Hierarchical Integration Pattern**: Define parent-child relationships between machines

    ```rust
    use rustate::{Action, Machine, MachineBuilder, State, Transition};

    // In a common crate: Define a trait for child machines
    trait ChildMachine {
        fn handle_parent_event(&mut self, event: &str) -> rustate::Result<bool>;
        fn is_in_final_state(&self) -> bool;
    }

    // In child crate: Implement child machine
    struct ConcreteChildMachine {
        machine: Machine,
    }

    impl ConcreteChildMachine {
        fn new() -> Self {
            let final_state = State::new_final("final");
            let initial = State::new("initial");
            let machine = MachineBuilder::new("childMachine")
                .state(initial)
                .state(final_state)
                .initial("initial")
                .transition(Transition::new("initial", "COMPLETE", "final"))
                .build()
                .unwrap();
                
            Self { machine }
        }
    }

    impl ChildMachine for ConcreteChildMachine {
        fn handle_parent_event(&mut self, event: &str) -> rustate::Result<bool> {
            self.machine.send(event)
        }
        
        fn is_in_final_state(&self) -> bool {
            self.machine.is_in("final")
        }
    }

    // In parent crate: Create parent machine that coordinates with child
    fn setup_parent_machine(mut child: impl ChildMachine + 'static) -> Machine {
        let check_child_status = Action::new(
            "checkChildStatus",
            ActionType::Transition,
            move |ctx, _evt| {
                if child.is_in_final_state() {
                    let _ = ctx.set("childComplete", true);
                }
            },
        );
        
        MachineBuilder::new("parentMachine")
            // ...setup states and transitions...
            .on_entry("monitoring", check_child_status)
            .build()
            .unwrap()
    }
    ```

### Best Practices for Cross-Crate Integration

1.  **Define Common Types**: Create a shared crate for common event and state types
2.  **Use Trait Abstraction**: Define traits for machine capabilities to allow different implementations
3.  **Leverage Context**: Use context for data sharing with clear read/write patterns
4.  **Event Namespacing**: Prefix events with module or crate names to avoid collisions
5.  **Minimize Coupling**: Design machines to be as independent as possible
6.  **Error Handling**: Use Result types for robust cross-machine communication
7.  **Testing**: Test integrated machines as a whole system. *(Note: Utilizing Model-Based Testing (MBT) techniques is a goal, pending verification of MBT feature status).*

This approach allows you to build complex applications with modular, type-safe state management across multiple crates, perfect for large Rust applications with distinct domains.

## Key Concepts

- **State**: Represents a node in the statechart
- **Transition**: Defines movement between states in response to events
- **Guard**: Logic that determines transition conditions
- **Action**: Side effects executed during state transitions
- **Context**: Stores the extended state of the machine
- **Cross-Crate Integration**: Patterns for connecting state machines across different crates
- **Actor Pattern**: Concepts related to actor-based concurrency and state management within `rustate_core`. 

## Roadmap

*(Current status as of v0.3.0)*

- [ ] **Implement Model-Based Testing (MBT) Support:** Integrate MBT capabilities for automated test generation and validation based on the state machine models. *(Status: Not started)*
- [ ] **Explore Alternative Cross-Crate Communication:** Investigate and potentially implement/document alternative communication patterns beyond shared memory (e.g., message passing, event bus). *(Status: Planning/Research)*
- [ ] **Enhance Actor Pattern Support:** Further develop and document the actor pattern features within `rustate` (e.g., ask pattern, supervision). *(Status: Basic implementation exists, enhancements planned)*
- [ ] **Add Comprehensive Examples:** Provide more diverse and complex usage examples. *(Status: Ongoing)*
- [ ] **Benchmarking & Performance Optimization:** Conduct performance analysis and optimize critical paths. *(Status: Basic benchmark setup exists, optimization not started)*
- [ ] **Formalize `.ssot` Integration:** Define and potentially implement a workflow for using `.ssot` files to define/generate `rustate` machines. *(Status: Planned)*
- [ ] **Improve CI/CD and Testing:** Enhance the testing suite (unit, integration, MBT) and automate builds/releases. *(Status: Basic setup exists, enhancements planned)*
- [ ] **Implement Deep History:** Complete the implementation for deep history states. *(Status: Partially implemented/needs review)*
- [ ] **Stabilize Codegen Macros:** Finalize and document the `create_machine` macro (potentially in a separate `rustate-macros` crate). *(Status: Basic macro exists but needs separate crate and testing)*
- [ ] **Full Serialization Support:** Investigate methods for serializing/deserializing actions and guards (e.g., using identifiers). *(Status: Planning/Research)* 