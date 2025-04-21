# RuState

RuState is a type-safe state machine and statechart library implemented in Rust. Inspired by XState, it follows the principles of model-based testing (MBT).

## Demo

Check out the live demo: [RuState Demo](https://jun784.github.io/rustate/)

The demo features:
- Interactive state machine visualization
- Traffic light state machine example
- Hierarchical state machine example
- Real-time state transition tracking

## Overview

RuState provides the following features:

- ✅ Finite state machines and statecharts
- ✅ Hierarchical states
- ✅ Parallel states
- ✅ Transition conditions (guards)
- ✅ Actions (side effects)
- ✅ Context (extended state)
- ✅ Type-safe API
- ✅ Serializable machines
- ✅ Cross-crate state machine integration
- ✅ Model-based testing (MBT) support

## Model-Based Testing (MBT) Integration

RuState incorporates the principles of model-based testing:

1. **Model Definition**: Define explicit models using states, transitions, guards, and actions
2. **Test Case Generation**: Automatically generate test cases from the model
3. **Test Execution**: Support for both online and offline testing
4. **Complete Coverage Verification**: Ensure tests cover all states and transitions

### Key Features

- **Test Generator**: Automatically generate test cases from state machines
- **Online Testing**: Directly test state machines at runtime
- **Offline Testing**: Export test cases to run later
- **State Coverage Report**: Verify which states and transitions have been tested

## Usage Examples

### Simple State Machine

```rust
use rustate::{Action, ActionType, Machine, MachineBuilder, State, Transition};

// Create states
let green = State::new("green");
let yellow = State::new("yellow");
let red = State::new("red");

// Create transitions
let green_to_yellow = Transition::new("green", "TIMER", "yellow");
let yellow_to_red = Transition::new("yellow", "TIMER", "red");
let red_to_green = Transition::new("red", "TIMER", "green");

// Define actions
let log_green = Action::new(
    "logGreen",
    ActionType::Entry,
    |_ctx, _evt| println!("Entering GREEN state - Go!"),
);

// Build the machine
let mut machine = MachineBuilder::new("trafficLight")
    .state(green)
    .state(yellow)
    .state(red)
    .initial("green")
    .transition(green_to_yellow)
    .transition(yellow_to_red)
    .transition(red_to_green)
    .on_entry("green", log_green)
    .build()
    .unwrap();

// Send events
machine.send("TIMER").unwrap();
```

### Model-Based Testing Example

```rust
use rustate::{Machine, TestGenerator, TestRunner};

// From an existing state machine definition...
let machine = /* ... */;

// Generate test cases
let test_generator = TestGenerator::new(&machine);
let test_cases = test_generator.generate_all_transitions();

// Run tests
let test_runner = TestRunner::new(&machine);
let results = test_runner.run_tests(test_cases);

// Coverage report
let coverage = results.get_coverage();
println!("State coverage: {}%", coverage.state_coverage());
println!("Transition coverage: {}%", coverage.transition_coverage());
```

### Cross-Crate State Machine Integration

RuState supports integrating multiple state machines across different crates in a type-safe manner, allowing you to build complex, modular state machines that communicate with each other.

#### Design Patterns for State Machine Integration

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
7. **Testing**: Test integrated machines as a whole system using MBT techniques

This approach allows you to build complex applications with modular, type-safe state management across multiple crates, perfect for large Rust applications with distinct domains.

## Installation

Add to your Cargo.toml:

```toml
[dependencies]
rustate = "0.2.0"
```

## Documentation

### Key Concepts

- **State**: Represents a node in the statechart
- **Transition**: Defines movement between states in response to events
- **Guard**: Logic that determines transition conditions
- **Action**: Side effects executed during state transitions
- **Context**: Stores the extended state of the machine
- **Cross-Crate Integration**: Patterns for connecting state machines across different crates
- **TestGenerator**: Generates test cases from the model
- **TestRunner**: Executes test cases
- **CoverageReport**: Analyzes test coverage

## Roadmap

- [x] Model checker integration
- [ ] Property-based testing
- [ ] Test visualization tools
- [ ] QuickCheck-style testing
- [ ] MBT with Fuzzing
- [ ] Property specification and verification with temporal logic (LTL/CTL)
- [ ] Performance optimization for large state machines
- [x] State machine coordination for distributed systems
- [ ] Enhanced WebAssembly (WASM) support
- [ ] Integration with visual state machine editors
- [ ] Automatic state machine model generation from real systems
- [ ] Support for more advanced concurrency models
- [ ] Domain-specific language (DSL) for state machine definition
- [ ] Optimized version for microcontrollers

## License

MIT 

## プロジェクト完成度評価 (Project Completion Status)

### 機能実装状況 (Implementation Status)

| モジュール (Module) | 進捗状況 (Progress) | 備考 (Notes) |
|-------------------|-------------------|-------------|
| rustate コア (Core) | 95% | 基本機能は実装済み、最適化と拡張機能を追加中 |
| テスト機能 (Testing) | 90% | MBT、プロパティベーステストなど実装済み |
| エディタ (Editor) | 80% | 基本的な編集機能は実装済み、UI改善の余地あり |
| デモ (Demo) | 85% | トラフィックライト、階層状態機械の例は完成 |
| Agent | 30% | 基本構造は定義済み、実装は初期段階 |
| ドキュメント (Documentation) | 75% | 主要機能の説明はあるが、APIドキュメントの拡充が必要 |
| CI/CD | 70% | GitHub Actionsで基本的なワークフロー設定済み |

### 次のステップ (Next Steps)

1. **Agent機能の実装完了**: LLMと統合したエージェントフレームワークの実装
2. **REST APIインターフェースの追加**: 型安全なREST API統合の実装
3. **ドキュメント拡充**: APIドキュメントの完成とxstateレベルの品質向上
4. **テスト可視化ツール**: テスト結果と状態遷移の視覚化機能の追加
5. **開発プロセス状態管理**: ビルド、デプロイなどの開発プロセスをrustateで制御する機能

### 優先タスク (Priority Tasks)

- [ ] Agent機能の実装を加速
- [ ] プロパティベーステストの機能強化
- [ ] エディタUIの改善
- [ ] デモページの拡充
- [ ] CI/CDパイプラインの強化

最終更新: 2024年5月7日 

## gRPC インターフェース

RuStateはgRPCを通じて型安全な状態機械APIを提供します。以下の機能があります：

- **型安全なステートマシン操作**: RuStateの全機能をgRPC経由で利用可能
- **リアルタイム状態変化監視**: ストリーミングでステートマシンの状態変化をリアルタイム監視
- **バッチ処理**: 複数イベントのトランザクション的処理
- **自動コード生成**: クライアント側の型安全コードを動的生成
- **クロスプラットフォーム**: 様々な言語・環境からアクセス可能

### サーバー側の使用例

```rust
use rustate_grpc::run_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // サーバーをポート50051で起動
    run_server("[::1]:50051").await?;
    
    Ok(())
}
```

### 型安全なクライアント

```rust
use rustate_grpc::client::typesafe::TypeSafeClient;
use std::fmt;

// イベント型を定義
#[derive(Clone)]
enum TrafficLightEvent {
    Timer,
    Emergency,
    Reset,
}

impl fmt::Display for TrafficLightEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timer => write!(f, "TIMER"),
            Self::Emergency => write!(f, "EMERGENCY"),
            Self::Reset => write!(f, "RESET"),
        }
    }
}

// コンテキスト型を定義
#[derive(serde::Serialize, serde::Deserialize, Default, Debug)]
struct TrafficLightContext {
    counter: u32,
    is_emergency: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 型安全なクライアントを作成
    let mut client = TypeSafeClient::<TrafficLightEvent, TrafficLightContext>::new(
        "http://[::1]:50051",
        "traffic_light",
    ).await?;
    
    // イベント送信
    let result = client.send_event(TrafficLightEvent::Timer).await?;
    println!("イベント処理結果: {:?}", result);
    
    Ok(())
}
```

詳細は[gRPCドキュメント](./crates/rustate-grpc/README.md)を参照してください。 