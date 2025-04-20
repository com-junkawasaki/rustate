# RuState Project

This repository is organized as a Cargo Workspace for the RuState library and related tools.

## Workspace Structure

- `crates/rustate`: The core library for statecharts in Rust

## Getting Started

To build all crates in the workspace:

```bash
cargo build
```

To run tests for all crates:

```bash
cargo test
```

## Crates

### RuState

A Rust implementation of statecharts inspired by XState. RuState provides a type-safe way to model and
implement finite state machines and statecharts in Rust.

See the [RuState documentation](crates/rustate/README.md) for more details.

## License

MIT 