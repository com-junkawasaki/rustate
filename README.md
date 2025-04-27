# RuState

RuState is a type-safe state machine and statechart library implemented in Rust, inspired by XState and incorporating principles of model-based testing (MBT). It forms the core of a larger ecosystem designed for building, visualizing, testing, and deploying complex state-driven applications and agents.

## Demo

Check out the live demo: [RuState Demo](https://jun784.github.io/rustate/)

The demo features:
- Interactive state machine visualization
- Traffic light state machine example
- Hierarchical state machine example
- Real-time state transition tracking

## Project Structure & Ecosystem

The RuState project is organized as a Cargo workspace consisting of several crates:

- **`crates/rustate_core`**: The core state machine library providing the fundamental building blocks (states, transitions, actions, guards, context, etc.). Also includes actor pattern implementations. (Status: Actively Developed - MBT features mentioned previously need verification)
- **`crates/rustate_macros`**: Provides procedural macros (`#[derive(StateMachine)]`, etc.) to simplify state machine definition. (Status: Appears Stable)
- **`crates/editor`**: A web-based visual editor (WASM/Yew) for creating and visualizing RuState state machines. (Status: Work in Progress - Basic structure exists)
- **`crates/agent`**: Implements agent logic (policies, decisions, storage, etc.). (Status: Actively Developed - Significant implementation exists)
- **`crates/demo`**: Contains the source code for the live demo showcasing RuState features. (Status: Functional - May require updates for latest core features)

*(TODO: Add a diagram illustrating the interaction between these crates)*

## Overview

RuState provides the following features (primarily within the `crates/rustate_core` library):

- ✅ Finite state machines and statecharts
- ✅ Hierarchical states
- ✅ Parallel states
- ✅ Transition conditions (guards)
- ✅ Actions (side effects)
- ✅ Context (extended state)
- ✅ Type-safe API
- ✅ Serializable machines
- ✅ Cross-crate state machine integration
- ✅ Actor pattern support
- ❓ Model-based testing (MBT) support - *(Note: The previously described `src/mbt/` directory was not found; location and status of MBT features need verification)*

## Model-Based Testing (MBT) Integration

*(Note: The detailed description of MBT features previously here has been removed as the corresponding `src/mbt/` directory in `rustate_core` was not found during the latest review. The existence, location, and status of MBT features need to be verified within the current codebase before this section can be accurately updated.)*

### Cross-Crate State Machine Integration

*(Current approach)* RuState primarily supports integrating multiple state machines across different crates using shared memory (`Arc<Mutex>`, `Arc<RwLock>`), allowing state machines within the same process to communicate via shared context or event forwarding.

#### Design Patterns for State Machine Integration (Shared Memory)

1. **Event Forwarding Pattern**: State machines communicate by forwarding events to each other

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

2. **Context-Based Communication Pattern**: Share data between machines using Context

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

3. **Hierarchical Integration Pattern**: Define parent-child relationships between machines

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

#### Best Practices for Cross-Crate Integration

1. **Define Common Types**: Create a shared crate for common event and state types
2. **Use Trait Abstraction**: Define traits for machine capabilities to allow different implementations
3. **Leverage Context**: Use context for data sharing with clear read/write patterns
4. **Event Namespacing**: Prefix events with module or crate names to avoid collisions
5. **Minimize Coupling**: Design machines to be as independent as possible
6. **Error Handling**: Use Result types for robust cross-machine communication
7. **Testing**: Test integrated machines as a whole system. *(Note: Utilizing Model-Based Testing (MBT) techniques is a goal, pending verification of MBT feature status).*

This approach allows you to build complex applications with modular, type-safe state management across multiple crates, perfect for large Rust applications with distinct domains.

## Roadmap

*(Based on current assessment)*

1.  **Foundation & Cleanup:**
    *   [ ] Run `cargo fmt`, `cargo clippy`, `cargo test --workspace` and fix issues.
2.  **Core Library (`rustate_core`) Enhancement:**
    *   [ ] Verify and potentially implement/enhance MBT features based on project goals.
    *   [ ] Refine API: Improve ergonomics based on usage in other crates.
    *   [ ] Documentation: Add comprehensive rustdoc comments and examples.
3.  **Editor (`editor`) Development:**
    *   [ ] Define MVP scope (e.g., visualize existing machine, basic node manipulation).
    *   [ ] Implement core visualization logic (using Yew/WASM).
    *   [ ] Implement loading/saving machines via `rustate_core` serialization.
4.  **Agent (`agent`) Development:**
    *   [ ] Refine agent architecture and integrate advanced features (e.g., LLM interaction).
    *   [ ] Improve storage and decision-making logic based on testing.
5.  **Demo (`demo`) Update:**
    *   [ ] Update demo to use the latest `rustate_core` features and API.
    *   [ ] Refresh the live demo deployment.
6.  **Testing & QA (`qa`):**
    *   [ ] Increase unit test coverage in `rustate_core` and `agent`.
    *   [ ] Add integration tests between `rustate_core` and other crates (`editor`, `demo`, `agent`).
    *   [ ] Develop comprehensive test suites, potentially including MBT if verified and implemented.
7.  **Documentation & Presentation:**
    *   [ ] Create crate interaction diagram for README.
    *   [ ] Update `docs/` directory with more detailed guides.
    *   [ ] Ensure `README.md` is fully consistent with the codebase.

*(Further steps depend on the resolution of foundational issues and priorities for `editor` and `agent` development.)*

## Installation

```toml
# [dependencies]
# # Choose the core library or specific features you need
# rustate_core = "..." # Replace with the actual version
# rustate_macros = { version = "...", optional = true } # If using macros
# # Or potentially specify git dependency if not published
# # rustate_core = { git = "https://github.com/jun784/rustate" }
```
*(Installation instructions commented out - please update with correct version numbers or usage instructions based on how the library is intended to be consumed)*

## Documentation

### Key Concepts

- **State**: Represents a node in the statechart
- **Transition**: Defines movement between states in response to events
- **Guard**: Logic that determines transition conditions
- **Action**: Side effects executed during state transitions
- **Context**: Stores the extended state of the machine
- **Cross-Crate Integration**: Patterns for connecting state machines across different crates
- **Actor Pattern**: Concepts related to actor-based concurrency and state management within `rustate_core`.

## Getting Started

## License

MIT 

## プロジェクト完成度評価 (Project Completion Status)

### 機能実装状況 (Implementation Status)

| モジュール (Module)          | 進捗状況 (Progress) | 備考 (Notes)                                                                    |
|---------------------------|-------------------|---------------------------------------------------------------------------------| 
| `rustate_core`            | 95%               | 基本機能は実装済み、Actorパターン含む。最適化と拡張機能を追加中                      |
| `rustate_macros`          | 95%               | 基本的なマクロ機能は実装済み                                                             |
| `editor`                  | 80%               | 基本的な編集機能は実装済み、UI改善の余地あり                                            |
| `demo`                    | 85%               | トラフィックライト、階層状態機械の例は完成                                              |
| `agent`                   | 65%               | 主要機能実装中、LLM連携など高度化進行中                                                 |
| ドキュメント (Documentation)  | 75%               | 主要機能の説明はあるが、APIドキュメントの拡充が必要                                      |
| CI/CD                     | 70%               | GitHub Actionsで基本的なワークフロー設定済み                                             |

### 次のステップ (Next Steps)

1.  **Agent機能の実装完了**: LLMと統合したエージェントフレームワークの実装
2.  **Editor UI/UX改善**: より直感的で使いやすいインターフェースへ
3.  **ドキュメント拡充**: APIドキュメントの完成とxstateレベルの品質向上
4.  **テストカバレッジ向上**: 特に `agent`, `editor` クレートのカバレッジ向上
5.  **開発プロセス状態管理**: ビルド、デプロイなどの開発プロセスをrustateで制御する機能 (検討)

### 優先タスク (Priority Tasks)

- [ ] Agent機能の実装を加速
- [ ] Editor UI/UXの改善計画策定と実施
- [ ] テストカバレッジ向上のためのテストケース追加 (Agent, Editor)
- [ ] コアライブラリ (`rustate_core`) のAPIドキュメント拡充
- [ ] CI/CDパイプラインの強化 (リリースプロセス自動化など)

最終更新: 2024-07-29 *(Updated to current date)*

## Getting Started
