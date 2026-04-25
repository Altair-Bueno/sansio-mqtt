# MQTT v5 Client Stack Design

## Goal

Define a no-std-first MQTT v5 client architecture with clear crate boundaries
and a phased implementation path for:

1. `sansio-mqtt-v5-contract` (shared protocol contracts)
2. `sansio-mqtt-v5-state-machine` (pure session transitions)
3. `sansio-mqtt-v5-protocol` (orchestration implementing `sansio::Protocol`)
4. `sansio-mqtt-v5-tokio` (runtime/transport driver)

This design replaces/renames the existing scaffold crate `sansio-mqtt-v5-statig`
to `sansio-mqtt-v5-state-machine`.

## Constraints and Invariants

- MQTT v5.0 specification is authoritative for behavior.
- `sansio-mqtt-v5-contract`, `sansio-mqtt-v5-state-machine`, and
  `sansio-mqtt-v5-protocol` are `#![no_std]`.
- No `std`, `tokio`, or I/O primitives in `state-machine` or `protocol`.
- `sansio-mqtt-v5-types` remains the sole source of MQTT packet encode/decode
  types.
- `sansio-mqtt-v5-protocol` must implement `sansio::Protocol` from
  `sansio = "1.0.1"` as the primary public protocol contract.
- Every phase must compile before moving to the next.
- Validation commands:
  - `cargo fmt`
  - `cargo clippy`
  - `cargo build -p <crate>` after each task/phase gate

## Architecture

### Crates

1. `sansio-mqtt-v5-contract`
   - Shared boundary types used by both protocol orchestration and FSM.
   - Contains input/action enums, user requests, timer keys, and shared
     reason/error types.

2. `sansio-mqtt-v5-state-machine`
   - Stateless-by-interface transition engine plus shared mutable context.
   - Consumes internal events and emits `Action` values only.
   - No transport, clock, or async runtime details.

3. `sansio-mqtt-v5-protocol`
   - Owns protocol orchestration and implements `sansio::Protocol`.
   - Bridges raw bytes (or decoded packets) into state-machine events using
     `sansio-mqtt-v5-types`.
   - Owns packet-id allocation and timeout scheduling surfaced through
     `poll_timeout`/`handle_timeout`.

4. `sansio-mqtt-v5-tokio`
   - Tokio-specific adapter around protocol client.
   - Owns TCP split tasks, timer wheel/map integration, and user-facing async
     API.

### Dependency Graph

- `sansio-mqtt-v5-contract` -> `sansio-mqtt-v5-types`, `heapless`
- `sansio-mqtt-v5-state-machine` -> `sansio-mqtt-v5-contract`,
  `sansio-mqtt-v5-types`, `statig`, `heapless`
- `sansio-mqtt-v5-protocol` -> `sansio-mqtt-v5-contract`,
  `sansio-mqtt-v5-state-machine`, `sansio-mqtt-v5-types`, `sansio`
- `sansio-mqtt-v5-tokio` -> `sansio-mqtt-v5-protocol`, `tokio`

This avoids cyclic dependencies while keeping protocol/state-machine
interactions strongly typed.

## Module Boundaries

### `sansio-mqtt-v5-contract`

- `src/input.rs`: `Input<'a>`
- `src/action.rs`: `Action`, `SessionAction`
- `src/options.rs`: `ConnectOptions`, `PublishRequest`, `SubscribeRequest`
- `src/timer.rs`: `TimerKey`
- `src/error.rs`: shared disconnect/timeout reasons and boundary-level errors

### `sansio-mqtt-v5-state-machine`

- `src/context.rs`: shared context (`pending_acks`, QoS2 tracking, keepalive
  config)
- `src/states.rs`: state data structs only
- `src/transitions.rs`: transition logic grouped by behavior block
- `src/lib.rs`: state-machine type, event dispatch, public exports

### `sansio-mqtt-v5-protocol`

- `src/client.rs`: `MqttProtocol` implementation of `sansio::Protocol`
- `src/timer_queue.rs`: internal timeout scheduling abstraction used by
  `poll_timeout`
- `src/error.rs`: protocol-local errors (`DecodeError`, `UnexpectedPacket`,
  etc.)
- `src/lib.rs`: crate surface exports + trait type aliases for protocol I/O

### `sansio-mqtt-v5-tokio`

- `src/transport.rs`: read/write task split around `TcpStream`
- `src/timer.rs`: tokio timer map abstraction
- `src/driver.rs`: `TokioClient` event loop and action dispatcher
- `src/lib.rs`: public re-exports only

## End-to-End Data Flow

1. Tokio driver reads network data and calls `handle_read(Rin)` on
   `MqttProtocol`.
2. User API calls map to `UserCommand` and enter via `handle_event(Ein)`.
3. Protocol decodes/normalizes inbound messages via `sansio-mqtt-v5-types` and
   forwards typed inputs to the state machine.
4. State machine emits `Action` values.
5. Protocol maps actions into sansio pull queues:
   - outbound wire payloads in `poll_write()` (`Wout`)
   - session/app events in `poll_event()` (`Eout`)
   - deadlines via `poll_timeout()` (`Time = u32`)
6. Driver executes side effects by draining `poll_write`/`poll_event`; when a
   timeout deadline is reached, driver calls `handle_timeout(now)`.

## State-Machine Behavior Blocks

Transitions are implemented and tested in strict order:

1. Connection handshake
2. Keepalive ping cycle
3. QoS0 outbound publish
4. QoS1 outbound publish and retry
5. QoS2 outbound publish two-phase ack/retry
6. Subscribe/suback handling
7. Inbound publish (QoS0/1/2 + pubrel/pubcomp)
8. User-triggered disconnect from any connected substate

Each block ships with focused tests before moving to the next block.

## Rename and Migration Notes

- Rename crate directory: `crates/sansio-mqtt-v5-statig` ->
  `crates/sansio-mqtt-v5-state-machine`.
- Update package name in `Cargo.toml` accordingly.
- Update workspace dependencies and references from `sansio-mqtt-v5-statig` to
  `sansio-mqtt-v5-state-machine`.
- Keep current empty scaffold behavior-neutral during rename; behavior additions
  happen in subsequent tasks.
- Add `sansio = "1.0.1"` in workspace dependencies and consume it in
  `sansio-mqtt-v5-protocol`.

## Testing and Verification Strategy

### Per-task/phase compile gates

- `cargo build -p sansio-mqtt-v5-contract`
- `cargo build -p sansio-mqtt-v5-state-machine`
- `cargo build -p sansio-mqtt-v5-protocol`
- `cargo build -p sansio-mqtt-v5-tokio`

### Behavior tests

- `sansio-mqtt-v5-protocol`: decode bridge, timer queue operations, packet-id
  wrap/exhaustion
- `sansio-mqtt-v5-protocol`: `Protocol` trait conformance (`handle_read`,
  `handle_event`, `poll_write`, `poll_event`, `poll_timeout`, `handle_timeout`)
  plus packet-id wrap/exhaustion
- `sansio-mqtt-v5-state-machine`: transition tests per behavior block (A-H)
- `sansio-mqtt-v5-tokio`: integration test with mock broker and action dispatch
  assertions

### no_std compliance checks

- Verify no `use std` in:
  - `crates/sansio-mqtt-v5-protocol`
  - `crates/sansio-mqtt-v5-state-machine`

## Out of Scope

- Persistent session storage across process restarts
- Non-TCP transports (WebSocket, QUIC, serial)
- MQTT versions other than v5.0
- Runtime abstractions beyond tokio for this phase

## Success Criteria

- Cleanly separated crates with acyclic dependencies.
- `sansio-mqtt-v5-protocol` exposes the canonical protocol engine through
  `sansio::Protocol`.
- Protocol and state-machine remain `no_std` and transport-free.
- Tokio crate provides ergonomic end-user async API with re-exported
  request/session types.
- Each phase compiles independently and has focused tests for introduced
  behavior.
