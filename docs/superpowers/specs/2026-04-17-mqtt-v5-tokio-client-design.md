# MQTT v5 Tokio Client Design (sansio-backed)

## Goal

Design and implement `crates/sansio-mqtt-v5-tokio` as an async Tokio-facing client API built on top of `sansio-mqtt-v5-protocol`, preserving the sansio protocol engine as the single source of MQTT state behavior.

The Tokio crate must provide a nicer API than manual protocol driving, while keeping explicit event-loop control (no hidden background task and no convenience managed mode).

## Scope

### In scope

- A split API with explicit ownership boundaries:
  - `Client`: cloneable command handle
  - `EventLoop`: single owner that drives network/protocol progress
- Tokio TCP transport integration (`tokio::net::TcpStream`)
- Timer integration for keep-alive via Tokio time
- Command ingress (publish/subscribe/unsubscribe/disconnect)
- Event egress (messages, connection state, publish ack lifecycle)
- Error model for user-command rejection vs event-loop failures
- Minimal examples and tests for expected runtime behavior

### Out of scope (initial implementation)

- Automatic reconnect policies/backoff
- TLS/WebSocket transports
- Hidden managed background-task API
- Persistent offline queueing beyond what protocol state already covers
- Additional runtime support beyond Tokio

## Design Principles

- Keep sansio as authority: all protocol decisions remain in `sansio-mqtt-v5-protocol`.
- Explicit progress model: user must drive the event loop.
- Predictable concurrency: commands are async send operations; all protocol/socket state lives in one loop task/context.
- Backpressure by default: bounded command and event channels.
- Clear separation between command-plane errors and connection/protocol failures.

## Public API

## Crate root exports

- `pub struct ConnectOptions`
- `pub struct Client`
- `pub struct EventLoop`
- `pub enum Event`
- `pub enum ClientError`
- `pub enum ConnectError`
- `pub enum EventLoopError`
- `pub async fn connect(options: ConnectOptions) -> Result<(Client, EventLoop), ConnectError>`

No `connect_managed` or similar helper is provided.

## Connect options

`ConnectOptions` is Tokio-transport-focused and maps to protocol options:

- `addr: std::net::SocketAddr` or host/port tuple equivalent
- `connection: sansio_mqtt_v5_protocol::ConnectionOptions`
- `protocol_config: sansio_mqtt_v5_protocol::Config` (optional override; default if omitted)
- `command_channel_capacity: usize` (bounded, default small and non-zero)

Rationale: keep protocol-native types visible rather than inventing duplicate data models.

## Client API

`Client` is cloneable and sends commands to `EventLoop` over a bounded channel.

Methods:

- `pub async fn publish(&self, msg: ClientMessage) -> Result<(), ClientError>`
- `pub async fn subscribe(&self, options: SubscribeOptions) -> Result<(), ClientError>`
- `pub async fn unsubscribe(&self, options: UnsubscribeOptions) -> Result<(), ClientError>`
- `pub async fn disconnect(&self) -> Result<(), ClientError>`

`ClientError` covers:

- channel closed (loop not running or terminated)
- channel full only if non-async/try variants are added later (not required now)

Note: command acceptance means queued for processing, not broker-level success.

## EventLoop API

`EventLoop` owns:

- `TcpStream`
- `sansio_mqtt_v5_protocol::Client<tokio::time::Instant>`
- command receiver
- read buffer
- timer state required to align with protocol `poll_timeout`

Method:

- `pub async fn poll(&mut self) -> Result<Event, EventLoopError>`

Behavior:

- `poll()` executes progress and returns one emitted event.
- Users drive `poll()` in their own loop/task.

## Event model

`Event` maps protocol outputs to Tokio-facing domain events:

- `Connected`
- `Disconnected`
- `Message(BrokerMessage)`
- `PublishAcknowledged { packet_id, reason_code }`
- `PublishCompleted { packet_id, reason_code }`
- `PublishDropped { packet_id, reason }`

This is a direct, low-surprise mapping from `UserWriteOut`.

## Internal architecture

## Components

1. `client.rs`
   - `Client` handle
   - command channel sender
   - async command methods

2. `event_loop.rs`
   - `EventLoop` state machine orchestrator
   - Tokio `select!` over socket reads, command rx, and timeout
   - drains protocol output queues (`poll_read`, `poll_write`, `poll_event`, `poll_timeout`)

3. `connect.rs`
   - `connect()` function
   - TCP connect + protocol initialization path
   - wiring channels and constructing `(Client, EventLoop)`

4. `error.rs`
   - connect/client/loop error enums
   - conversion mappings from io/protocol/channel errors

5. `event.rs`
   - public `Event` type
   - internal mapping helpers from protocol outputs

## Data flow

### Startup

1. `connect(options)` opens TCP socket.
2. Build protocol client (with provided/default protocol config).
3. Feed `UserWriteIn::Connect(connection_options)` into protocol.
4. Drain initial protocol effects:
   - handle `DriverEventOut::OpenSocket` expectation as already satisfied (socket already open)
   - feed `DriverEventIn::SocketConnected`
   - flush pending protocol writes to socket
5. Return `(Client, EventLoop)`.

### Steady-state loop

Loop iteration drives these phases:

1. Drain pending protocol writes to socket.
2. Emit any protocol read outputs as `Event`.
3. Evaluate protocol requested driver actions:
   - `CloseSocket` => close socket path and state update
   - `Quit` => terminal event-loop return
   - `OpenSocket` => unsupported in v1 (treated as loop error)
4. Wait on next input source (`tokio::select!`):
   - socket bytes read
   - command from `Client`
   - timeout instant from protocol `poll_timeout`
5. Feed corresponding protocol handler:
   - bytes -> `handle_read`
   - command -> `handle_write`
   - timeout -> `handle_timeout(now)`

### Shutdown

- If user issues `disconnect`, loop processes graceful DISCONNECT sequence.
- If command channel closes and no clones remain, the loop can continue to process socket-driven protocol events until disconnection.

## Error handling model

## ConnectError

- TCP connect failure
- Protocol bootstrap rejection (invalid initial state/options)
- Immediate IO failure during initial frame flush

## ClientError

- loop dropped / channel closed

## EventLoopError

- unrecoverable IO errors
- unrecoverable protocol errors from `sansio-mqtt-v5-protocol`
- unsupported driver event request for this version (`OpenSocket` after startup)

Error boundaries:

- command methods return `ClientError` only (transport/protocol failure details are surfaced by loop termination)
- loop failures are explicit and terminal in v1

## Reconnect policy

No automatic reconnect in initial implementation.

Rationale:

- keeps behavior explicit and composable
- avoids guessing policy knobs
- keeps first iteration aligned with sansio-driven deterministic behavior

Future reconnect support can be added as opt-in strategy API.

## Backpressure and buffering

- Command channel must be bounded and configurable.
- Socket writes are immediate flush attempts from protocol queue.
- No unbounded in-memory buffering introduced by Tokio layer.

## Testing strategy

## Unit tests (tokio crate)

- `connect` bootstrap drives protocol connect prelude correctly.
- command -> protocol routing (publish/subscribe/unsubscribe/disconnect).
- protocol output mapping to public `Event` is exact.
- loop handles socket closure by feeding `DriverEventIn::SocketClosed`.
- keep-alive timeout path calls protocol timeout with `Instant`.

## Integration tests

- local broker or controlled harness test:
  - connect -> subscribe -> publish -> receive message
  - disconnect emits expected lifecycle events
- publish QoS1/QoS2 lifecycle events observed when broker supports flows

## Regression tests

- ensure no hidden background task API appears
- ensure `EventLoop` remains explicit-driving model

## Example(s)

- `examples/connect_and_poll.rs`:
  - `let (client, mut event_loop) = connect(opts).await?;`
  - spawn a task issuing publishes via cloned `client`
  - main task loops on `event_loop.poll().await?` and handles `Event`

This example replaces manual protocol wiring from current prototype with the new API.

## Compatibility and migration notes

- Existing users of `sansio-mqtt-v5-protocol` keep full manual control unchanged.
- Tokio crate provides ergonomic orchestration only; protocol API remains source-compatible.
- Tokio crate should avoid wrapping protocol types unless necessary to preserve low cognitive overhead.

## Open decisions resolved by this spec

- Use split model (`Client` + explicit `EventLoop`) only.
- Do not provide managed/background convenience API in v1.
- No reconnect automation in v1.
- Keep protocol-native message/option types in public API to minimize translation layers.

## Implementation checklist

1. Add public types/modules in `crates/sansio-mqtt-v5-tokio/src/`.
2. Implement `connect()` and startup handshake bridging.
3. Implement `Client` command channel API.
4. Implement `EventLoop::poll` orchestration and `run` helper.
5. Add event mapping and error enums.
6. Add unit/integration tests for lifecycle and message paths.
7. Update examples to new API.
8. Run `cargo fmt`, `cargo clippy`, and relevant tests.
