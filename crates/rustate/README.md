# RuState

A Rust implementation of statecharts inspired by XState. RuState provides a type-safe way to model and
implement finite state machines and statecharts in Rust, with full support for model-based testing (MBT).

## Features

- ✅ Finite state machines and statecharts
- ✅ Hierarchical states
- ✅ Parallel states
- ✅ Guards/conditions for transitions
- ✅ Actions/side effects
- ✅ Context (extended state)
- ✅ Typesafe API
- ✅ Serializable machines
- ✅ Model-based testing support
- ✅ Cross-crate integration patterns

## Model-Based Testing Integration

RuState now includes comprehensive model-based testing features:

1. **Automated Test Generation**: Generate test cases from your state machine model
2. **Test Execution**: Run tests directly against your state machine or export them
3. **Coverage Analysis**: Measure state and transition coverage
4. **Model Checking**: Verify properties like reachability, safety, and liveness

### Key MBT Components:

- **TestGenerator**: Creates test cases for states, transitions, and loop coverage
- **TestRunner**: Executes test cases against your machine
- **ModelChecker**: Verifies model properties and detects deadlocks and unreachable states

## Cross-Crate Integration Patterns

RuState provides patterns for integrating state machines across crates with type safety:

1. **Event Forwarding Pattern**: Share state machine references and forward events between machines
2. **Context Sharing Pattern**: Share context data between multiple state machines
3. **Hierarchical Integration Pattern**: Connect parent-child state machines with traits

Enable with the `integration` feature:

```toml
[dependencies]
rustate = { version = "0.2.4", features = ["integration"] }
```

### Integration Example

```rust
use rustate::{
    Machine, MachineBuilder, State, Transition,
    integration::{
        SharedMachineRef,
        SharedContext,
        ChildMachine,
    },
};

// Create and share a state machine
let machine = create_machine();
let shared_machine = SharedMachineRef::new(machine);

// Forward events
shared_machine.send_event("EVENT")?;
```

See the `examples/integration` directory for complete integration examples.

## Integration Patterns in Detail

RuState provides three main integration patterns for connecting state machines across crate boundaries in a type-safe way:

### 1. Event Forwarding Pattern

This pattern allows state machines to communicate by sending events to each other. It's useful when you need to coordinate multiple state machines with minimal coupling.

```rust
use rustate::{Machine, MachineBuilder, State, Transition, Action, ActionType};
use rustate::integration::SharedMachineRef;

// Create a child state machine
let child_machine = MachineBuilder::new("child")
    .state(State::new("idle"))
    .state(State::new("active"))
    .initial("idle")
    .transition(Transition::new("idle", "ACTIVATE", "active"))
    .build()
    .unwrap();

// Create a shared reference
let shared_child = SharedMachineRef::new(child_machine);
let shared_child_clone = shared_child.clone();

// Parent machine action that forwards events to child
let forward_action = Action::new(
    "forwardToChild",
    ActionType::Transition,
    move |_ctx, evt| {
        if evt.event_type == "PARENT_EVENT" {
            let _ = shared_child_clone.send_event("ACTIVATE");
        }
    }
);
```

### 2. Context Sharing Pattern

This pattern allows multiple state machines to share data through a common context. It's ideal for scenarios where state machines need to access and modify shared state.

```rust
use rustate::{Machine, MachineBuilder, State, Action, ActionType};
use rustate::integration::SharedContext;

// Create shared context
let shared_context = SharedContext::new();
let context_for_a = shared_context.clone();
let context_for_b = shared_context.clone();

// Action that writes to shared context
let write_action = Action::new(
    "writeData",
    ActionType::Transition,
    move |_ctx, _evt| {
        let _ = context_for_a.set("status", "active");
    }
);

// Action that reads from shared context
let read_action = Action::new(
    "readData",
    ActionType::Transition,
    move |ctx, _evt| {
        if let Ok(Some(status)) = context_for_b.get::<String>("status") {
            let _ = ctx.set("localStatus", status);
        }
    }
);
```

### 3. Hierarchical Integration Pattern

This pattern establishes parent-child relationships between state machines using traits. It's powerful for complex systems where you need to model hierarchical relationships with high cohesion but low coupling.

```rust
use std::sync::{Arc, Mutex};
use rustate::{Machine, MachineBuilder, State, Transition};
use rustate::integration::hierarchical::{ChildMachine, DefaultChildMachine, coordination};

// Create a child state machine
let child_machine = MachineBuilder::new("child")
    .state(State::new("initial"))
    .state(State::new("running"))
    .state(State::new_final("complete"))
    .initial("initial")
    .transition(Transition::new("initial", "START", "running"))
    .transition(Transition::new("running", "COMPLETE", "complete"))
    .build()
    .unwrap();

// Wrap with trait implementation
let child = DefaultChildMachine::new(child_machine, "complete");
let child = Arc::new(Mutex::new(child));

// Create action that monitors child machine state
let monitor_action = coordination::create_child_monitor_action(
    "monitorChild",
    child.clone()
);

// Create action that forwards events to child machine
let forward_action = coordination::create_event_forwarder_action(
    "forwardToChild",
    child,
    "PARENT_START",  // Event received by parent
    "START"          // Event forwarded to child
);
```

### Combining Integration Patterns

For complex systems, these patterns can be combined to create powerful integration strategies. See the `examples/integration/combined_demo.rs` for a complete example that demonstrates all three patterns working together.

For asynchronous integration capabilities, use the `integration_async` feature:

```toml
[dependencies]
rustate = { version = "0.2.4", features = ["integration_async"] }
```

## Usage Example

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

// Send an event to the machine
machine.send("TIMER").unwrap();
```

### Model-Based Testing Example

```rust
use rustate::{Machine, TestGenerator, TestRunner, ModelChecker, Property, PropertyType};

// Assuming you have a state machine defined as above...
let machine = /* ... */;

// Generate test cases
let mut generator = TestGenerator::new(&machine);
let test_cases = generator.generate_all_transitions();

// Run tests
let mut runner = TestRunner::new(&machine);
let results = runner.run_tests(test_cases);
println!("Test success rate: {}%", results.success_rate());

// Coverage analysis
let coverage = results.get_coverage();
println!("State coverage: {}%", coverage.state_coverage());
println!("Transition coverage: {}%", coverage.transition_coverage());

// Model checking
let mut checker = ModelChecker::new(&machine);

// Define property to check
let property = Property {
    name: "Can reach red state".to_string(),
    property_type: PropertyType::Reachability,
    target_states: vec!["red".to_string()],
    description: None,
};

// Verify the property
let verification = checker.verify_property(&property);
if verification.satisfied {
    println!("Property satisfied: {}", property.name);
} else {
    println!("Property not satisfied: {}", property.name);
    if let Some(counterexample) = verification.counterexample {
        println!("Counterexample found with {} events", counterexample.len());
    }
}

// Detect deadlocks
let deadlocks = checker.detect_deadlocks();
println!("Deadlock states found: {}", deadlocks.len());
```

See the `examples` directory for complete examples.

## Network and Remote Integration

For network-based state machine control and monitoring, check out [rustate-grpc](https://github.com/jun784/rustate/tree/main/crates/grpc), which provides:

- Remote state machine creation and control via gRPC
- Real-time state change monitoring via streaming
- Type-safe client/server communication
- Cross-language support through protocol buffers

```toml
[dependencies]
rustate-grpc = { version = "0.1.0", features = ["full"] }
```

See the [rustate-grpc documentation](https://github.com/jun784/rustate/tree/main/crates/rustate-grpc) for detailed usage examples.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rustate = "0.2.4"
```

## Documentation

### Core Concepts

- **State**: Represents a node in the state chart
- **Transition**: Defines how the machine moves between states in response to events
- **Guard**: Conditional logic that determines if a transition should occur
- **Action**: Side effects that execute during state transitions
- **Context**: Stores extended state for the machine
- **TestGenerator**: Creates test cases from your state machine model
- **TestRunner**: Executes test cases against your machine
- **ModelChecker**: Verifies properties and analyzes your state machine model

### API Overview

- `State`: Create simple, compound, parallel, or history states
- `Transition`: Define transitions between states, including guards and actions
- `Guard`: Create guard conditions for transitions
- `Action`: Define actions/side effects for state transitions
- `Context`: Store and retrieve extended state data
- `Machine`: The runtime state machine instance
- `MachineBuilder`: Fluent API for creating state machines
- `TestGenerator`: Generate test cases from a state machine model
- `TestRunner`: Run tests against your state machine
- `ModelChecker`: Verify properties and analyze your state machine model

## Future Directions

- Advanced model checking algorithms
- Property-based testing integration
- Test visualization tools
- Fuzzing-based MBT
- Temporal logic (LTL/CTL) property specification and verification
- Performance optimizations for large state machines
- ✅ Distributed system state machine coordination
- Enhanced WebAssembly (WASM) support
- Integration with visual state machine editors
- Automatic state machine model generation from existing systems
- Advanced concurrency model support
- Domain-specific language (DSL) for state machine definition
- Microcontroller-optimized version

## License

MIT 

## テスト機能

RuStateには複数のテスト支援機能があります：

### モデルベーステスト (MBT)

状態マシンのモデルに基づいて自動的にテストケースを生成します。

```rust
let machine = create_test_machine();
let mut generator = TestGenerator::new(&machine);

// 状態カバレッジのテストケースを生成
let state_tests = generator.generate_all_states();

// 遷移カバレッジのテストケースを生成
let transition_tests = generator.generate_all_transitions();
```

### プロパティベーステスト

proptest との統合により、ランダムなイベントシーケンスでプロパティを検証します。

```rust
let property = Machine::property("state property")
    .given(|m| m.is_in("idle"))
    .when(|m| {
        let _ = m.send("START");
        Ok(m.current_state().clone())
    })
    .then(|m| m.is_in("running"));

let runner = PropertyTestRunner::new(machine);
let result = runner.verify_property(property, Config::default());
```

### XState互換モデルベーステスト (新機能)

XState v5互換のモデルベーステストインターフェースを提供します。

```rust
// ステートマシンからテストモデルを作成
let mut model = create_test_model(machine);

// アサーションを追加
model.assert("conditionName", |m| {
    // 条件をチェック
    true
});

// アクターの実装を提供
model.provide("actorName", |ctx, evt| {
    // アクターの実装
    Ok(())
});

// テストプランを作成（手動または自動生成）
let plan = XStateTestPlan {
    name: "Test Plan",
    paths: vec![
        XStateTestPath {
            name: "Path 1",
            segments: vec![
                XStatePathSegment {
                    state: "idle",
                    event: Some("START"),
                    assertions: None,
                },
                // ...
            ],
            description: Some("Test path description"),
        },
    ],
    // ...
};

// または自動生成
let generated_plan = model.generate_paths(max_depth);

// プランを実行
let results = execute_test_plan(&mut model, &plan)?;
```

### モデル検査

状態マシンの性質（到達可能性、安全性など）を検証します。

```rust
let mut checker = ModelChecker::new(&machine);

// 到達可能性プロパティの検証
let reachability = Property {
    name: "Can reach completed".to_string(),
    property_type: PropertyType::Reachability,
    target_states: vec!["completed".to_string()],
    description: None,
};

let result = checker.verify_property(&reachability);
``` 