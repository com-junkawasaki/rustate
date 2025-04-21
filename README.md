# RuState

RuState is a type-safe state machine and statechart library implemented in Rust. Inspired by XState, it follows the principles of model-based testing (MBT).

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
- [ ] State machine coordination for distributed systems
- [ ] Enhanced WebAssembly (WASM) support
- [ ] Integration with visual state machine editors
- [ ] Automatic state machine model generation from real systems
- [ ] Support for more advanced concurrency models
- [ ] Domain-specific language (DSL) for state machine definition
- [ ] Optimized version for microcontrollers

## License

MIT 