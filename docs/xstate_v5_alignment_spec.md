# RuState - XState v5 Alignment Specification

## 1. Goal

This document outlines the plan to refactor the `RuState` library to align its core concepts and API patterns more closely with XState v5, leveraging Rust's type system for enhanced safety and performance. The primary focus is the adoption of the Actor Model.

## 2. Core Concept Alignment

| XState v5 Concept         | RuState Alignment Plan                                                                                                | Status        |
| :------------------------ | :-------------------------------------------------------------------------------------------------------------------- | :------------ |
| **Actor Model**           | Central paradigm shift. Introduce core traits and structs.                                                            | In Progress   |
| `Snapshot`                | `actor::Snapshot<C, O>` struct defined. Represents immutable state (value, context, status, output).                  | Defined       |
| `ActorLogic`              | `actor::ActorLogic<TSnapshot, TEvent, TInput>` trait defined. Defines actor behavior (initial snapshot, transitions). | Defined       |
| `ActorRef`                | `actor::ActorRef<TEvent, TSnapshot>` trait defined. Reference to a running actor for sending events/inspection.       | Defined       |
| `createActor`             | Introduce top-level `rustate::create_actor(logic, options)` function.                                                   | Planned       |
| Machine as `ActorLogic` | `Machine<C, E, O>` struct implements `ActorLogic<MachineSnapshot<C, O>, E>`.                                        | In Progress   |
| State Value (`value`)     | Use `serde_json::Value` within `Snapshot` for flexible state representation (atomic, parallel, hierarchical).         | Implemented   |
| Context (`context`)       | Generic `C` parameter within `Snapshot` and `Machine`.                                                                | Implemented   |
| Events (`event`)          | Generic `E: EventObject` parameter. Standardize event handling.                                                       | Defined       |
| Input (`input`)           | Generic `TInput` parameter for `ActorLogic::get_initial_snapshot`.                                                    | Defined       |
| Output (`output`)         | Generic `O` parameter within `Snapshot` for final state output.                                                     | Defined       |
| Status (`status`)         | `actor::ActorStatus` enum defined (Active, Done, Error, Stopped).                                                   | Defined       |

## 3. API Changes & Additions

### 3.1. New Traits/Structs (in `actor.rs`)

-   `pub struct Snapshot<C, O>`
-   `pub enum ActorStatus`
-   `pub trait ActorLogic<TSnapshot, TEvent: EventObject, TInput = ()>`
    -   `fn get_initial_snapshot(&self, input: Option<TInput>) -> TSnapshot;`
    -   `async fn transition(&self, snapshot: TSnapshot, event: TEvent) -> Result<TSnapshot, RuStateError>;` (Marked async)
    -   **Decision:** Use `#[async_trait::async_trait]`.
-   `pub trait ActorRef<TEvent: EventObject, TSnapshot>: Send + Sync + fmt::Debug`
    -   `fn send(&self, event: TEvent) -> Result<(), RuStateError>;`
    *   `fn id(&self) -> &str;`
    *   `fn get_snapshot(&self) -> TSnapshot;`
    -   *(Planned: `subscribe`, `stop`)*

### 3.2. `Machine` Modifications (`machine.rs`)

-   Change generics: `Machine<S, E>` -> `Machine<C, E, O>`
-   Implement `ActorLogic`: `impl<C, E, O> ActorLogic<MachineSnapshot<C, O>, E> for Machine<C, E, O>`
-   Internal Logic: Refactor state transition logic (`send`, `process_state_event`, `execute_transition`, etc.) into a non-mutating async function like `async fn step(&self, current_snapshot, event) -> Result<NextSnapshot>`.
-   Remove mutable state fields (`current_states`, `context`) - managed via Snapshots.

### 3.3. New Top-Level Functions (`lib.rs` / `actor.rs`)

-   `pub fn create_actor<L, S, E, I>(logic: L, options: ActorOptions<I>) -> impl ActorRef<E, S>` where `L: ActorLogic<S, E, I>`
    -   Needs a concrete `Actor` implementation to manage the logic execution and state.

### 3.4. `MachineBuilder` Modifications (`machine.rs`)

-   Change generics: `MachineBuilder<S, E>` -> `MachineBuilder<C, E, O>`
-   `build()` method now returns a `Result<Machine<C, E, O>>` without initializing the state.

## 4. Key Feature Alignment (Roadmap)

### Phase 1: Core Actor Model (Current Focus)

-   [x] Define core traits (`ActorLogic`, `ActorRef`, `Snapshot`).
-   [ ] Make `ActorLogic` async (`#[async_trait::async_trait]`).
-   [ ] Implement non-mutating `Machine::step` function correctly (handle hierarchy, actions, context updates).
-   [ ] Implement concrete `Actor` struct (holds logic, state, potentially mailbox).
-   [ ] Implement `create_actor` function.
-   [ ] Implement basic `ActorRef::send` mechanism (e.g., using async channels).
-   [ ] Update basic examples to use `create_actor` and `actorRef.send`.

### Phase 2: Actor Spawning & Communication

-   [ ] Implement `spawn` action logic (spawns a child actor).
-   [ ] Implement `sendTo` action logic (sends event to parent, child, specific ID).
-   [ ] Introduce concept of an `ActorSystem` (optional, for managing actor lifecycles and IDs).

### Phase 3: Invoking Actors & Async Services

-   [ ] Implement `invoke` within state definitions.
-   [ ] Implement `ActorLogic` for Futures (`from_future`).
-   [ ] Implement `ActorLogic` for Callbacks (`from_callback`).
-   *(Optional: `from_observable`)*

### Phase 4: Built-in Actions & Guards

-   [ ] Implement `assign` action.
-   [ ] Implement `log` action.
-   [ ] Implement `raise` action.
-   [ ] Implement `choose` action.
-   [ ] Implement `and`, `or`, `not` guards.
-   *(Others: `sendParent`, `cancel`, `stopChild`, `emit`)*

### Phase 5: Advanced State Features

-   [ ] Implement `after` / delayed transitions/events.
-   [ ] Implement `history` states (shallow and deep).

### Phase 6: API Refinement & Type Safety

-   [ ] Refine `MachineBuilder` API to better mirror XState config (e.g., embedding action/guard/actor definitions).
-   [ ] Enhance generic usage for stricter type checking across events, context, actions, guards, actors.
-   [ ] Implement `machine.provide(...)` mechanism for injecting action/guard/actor implementations.

## 5. Async Handling

-   The core `ActorLogic::transition` method **must** be asynchronous (`async fn`) to support async actions, guards, and invoked actors.
-   The `async-trait` crate will be used: `#[async_trait::async_trait]` on the `ActorLogic` trait definition.
-   Actor implementations (e.g., the concrete `Actor` struct managing a `Machine`) will likely use an async runtime (like `tokio`) internally to manage tasks and event processing.

## 6. Type Safety

-   Leverage Rust's generics extensively for `Context`, `Event`, `Input`, `Output`, `Snapshot`, `ActorLogic`, and `ActorRef`.
-   Use traits (`EventObject`, `ActorLogic`, `ActorRef`) to define clear interfaces.
-   Explore techniques similar to XState's `setup` function or `machine.provide` to link string-based definitions (in `MachineBuilder`) to concrete, type-safe implementations (actions, guards, actors).

## 7. Out of Scope (Initial Refactoring)

-   Advanced inspection tools (visualizers, debuggers).
-   Complex serialization/deserialization of actor state (beyond basic `Machine` serialization).
-   Domain Specific Language (DSL) for machine definition (beyond `MachineBuilder`).
-   Specific optimizations for extremely large state machines or embedded environments.
-   Full alignment with *all* XState v5 nuances and edge cases. Focus is on core concepts and major features. 