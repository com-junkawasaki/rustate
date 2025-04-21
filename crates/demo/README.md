# RuState Demo

A demonstration web application for RuState, a Rust implementation of statecharts inspired by XState.

## Features

- Interactive state machine visualization
- Traffic light state machine example
- Hierarchical state machine example
- Real-time state transition display
- Event history tracking

## Running the Demo

To run the demo, you need [Trunk](https://trunkrs.dev/), a WASM web application bundler for Rust.

If you don't have Trunk installed, you can install it with:

```bash
cargo install trunk
```

Then, from the `crates/demo` directory, run:

```bash
trunk serve
```

This will start a local development server, usually at http://127.0.0.1:8080.

## Building for Production

To build the demo for production, run:

```bash
trunk build --release
```

The output will be in the `dist` directory.

## Demo Examples

### Traffic Light State Machine

A simple state machine that simulates a traffic light with four states:
- Green
- Yellow
- Red
- Off

It responds to two events:
- TIMER: Cycles through Green -> Yellow -> Red -> Green
- POWER: Toggles between On (Green) and Off states

### Hierarchical State Machine

A demonstration of hierarchical state composition, featuring:
- A parent state containing two child states
- Transitions between child states

## Implementation Details

This demo is built with:
- [Yew](https://yew.rs/): A modern Rust framework for creating web applications
- [RuState](https://github.com/jun784/rustate): A Rust implementation of statecharts
- [Trunk](https://trunkrs.dev/): A WASM application bundler for Rust

The demo showcases how to:
- Create and use state machines in Rust
- Handle events and transitions
- Display machine state in a UI
- Track state machine history 