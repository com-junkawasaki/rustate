# RState

A Rust implementation of statecharts inspired by XState. RState provides a type-safe way to model and
implement finite state machines and statecharts in Rust.

## Features

- ✅ Finite state machines and statecharts
- ✅ Hierarchical states
- ✅ Parallel states
- ✅ Guards/conditions for transitions
- ✅ Actions/side effects
- ✅ Context (extended state)
- ✅ Typesafe API
- ✅ Serializable machines

## Usage Example

### Simple State Machine

```rust
use rstate::{Action, ActionType, Machine, MachineBuilder, State, Transition};

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

### Hierarchical State Machine

```rust
use rstate::{Action, ActionType, Context, Machine, MachineBuilder, State, Transition};

// Create hierarchical states
let power_off = State::new("powerOff");
let mut player = State::new_compound("player", "stopped");
player.parent = Some("root".to_string());

let mut stopped = State::new("stopped");
stopped.parent = Some("player".to_string());

let mut playing = State::new_compound("playing", "normal");
playing.parent = Some("player".to_string());

// Create transitions
let power_toggle = Transition::new("powerOff", "POWER", "player");
let play = Transition::new("stopped", "PLAY", "playing");

// Create context
let mut context = Context::new();
context.set("track", 0).unwrap();

// Create a machine with hierarchical states
let mut machine = MachineBuilder::new("musicPlayer")
    .initial("powerOff")
    .state(power_off)
    .state(player)
    .state(stopped)
    .state(playing)
    .transition(power_toggle)
    .transition(play)
    .context(context)
    .build()
    .unwrap();
```

See the `examples` directory for complete examples.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rustate = "0.1.0"
```

## Documentation

### Core Concepts

- **State**: Represents a node in the state chart
- **Transition**: Defines how the machine moves between states in response to events
- **Guard**: Conditional logic that determines if a transition should occur
- **Action**: Side effects that execute during state transitions
- **Context**: Stores extended state for the machine

### API Overview

- `State`: Create simple, compound, parallel, or history states
- `Transition`: Define transitions between states, including guards and actions
- `Guard`: Create guard conditions for transitions
- `Action`: Define actions/side effects for state transitions
- `Context`: Store and retrieve extended state data
- `Machine`: The runtime state machine instance
- `MachineBuilder`: Fluent API for creating state machines

## License

MIT 