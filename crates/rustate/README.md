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
rustate = { version = "0.2.1", features = ["integration"] }
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

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rustate = "0.2.1"
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