# MQTT v5 Tokio Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a Tokio-native MQTT v5 client crate over the sansio protocol engine, and migrate the interactive example to an env-driven echo program with reconnect behavior that does not drain stdin while disconnected.

**Architecture:** The Tokio crate exposes a split API with `Client` (command handle) and `EventLoop` (single network/protocol owner). `connect()` wires channels, socket, and protocol bootstrap. The event loop drives read/write/event/timeout integration via Tokio `select!`, and the example uses a reconnect-aware driver loop built on `poll()`.

**Tech Stack:** Rust, Tokio (`net`, `sync`, `time`, `io-util`, `io-std`, `signal`), sansio, `sansio-mqtt-v5-protocol`, Cargo tests/examples.

---

### Task 1: Implement Tokio crate public API and runtime loop

**Files:**
- Create: `crates/sansio-mqtt-v5-tokio/src/client.rs`
- Create: `crates/sansio-mqtt-v5-tokio/src/connect.rs`
- Create: `crates/sansio-mqtt-v5-tokio/src/error.rs`
- Create: `crates/sansio-mqtt-v5-tokio/src/event.rs`
- Create: `crates/sansio-mqtt-v5-tokio/src/event_loop.rs`
- Modify: `crates/sansio-mqtt-v5-tokio/src/lib.rs`
- Create: `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs`
- Modify: `crates/sansio-mqtt-v5-tokio/Cargo.toml`

- [ ] **Step 1: Write failing API-shape tests first**

```rust
use sansio_mqtt_v5_tokio::{Client, ConnectOptions, Event, EventLoop};

#[tokio::test]
async fn connect_api_exposes_split_handles() {
    let _ = core::any::TypeId::of::<Client>();
    let _ = core::any::TypeId::of::<EventLoop>();
    let _ = core::any::TypeId::of::<Event>();
    let _ = core::any::TypeId::of::<ConnectOptions>();
}
```

- [ ] **Step 2: Run test and verify failure**

Run: `cargo test -p sansio-mqtt-v5-tokio connect_api_exposes_split_handles -- --nocapture`

Expected: FAIL due to missing exported types/functions.

- [ ] **Step 3: Add minimal public types and exports to compile**

```rust
// src/lib.rs
mod client;
mod connect;
mod error;
mod event;
mod event_loop;

pub use client::Client;
pub use connect::{connect, ConnectOptions};
pub use error::{ClientError, ConnectError, EventLoopError};
pub use event::Event;
pub use event_loop::EventLoop;
```

- [ ] **Step 4: Add failing command-routing test**

```rust
#[tokio::test]
async fn client_publish_enqueues_command() {
    // Build a test-only client around mpsc sender and assert publish() sends command.
    // Assert queue receives a Publish command variant.
}
```

- [ ] **Step 5: Run test and verify failure**

Run: `cargo test -p sansio-mqtt-v5-tokio client_publish_enqueues_command -- --nocapture`

Expected: FAIL due to unimplemented command channel plumbing.

- [ ] **Step 6: Implement Client command handle minimally**

```rust
#[derive(Clone)]
pub struct Client {
    tx: tokio::sync::mpsc::Sender<Command>,
}

impl Client {
    pub async fn publish(&self, msg: sansio_mqtt_v5_protocol::ClientMessage) -> Result<(), ClientError> {
        self.tx.send(Command::Publish(msg)).await.map_err(|_| ClientError::Closed)
    }
    // subscribe/unsubscribe/disconnect same pattern
}
```

- [ ] **Step 7: Add failing event-mapping tests**

```rust
#[test]
fn maps_protocol_outputs_to_public_events() {
    // feed sample UserWriteOut variants and assert Event mapping.
}
```

- [ ] **Step 8: Run test and verify failure**

Run: `cargo test -p sansio-mqtt-v5-tokio maps_protocol_outputs_to_public_events -- --nocapture`

Expected: FAIL due to missing mapper.

- [ ] **Step 9: Implement Event type and mapper**

```rust
pub enum Event {
    Connected,
    Disconnected,
    Message(sansio_mqtt_v5_protocol::BrokerMessage),
    PublishAcknowledged { packet_id: core::num::NonZero<u16>, reason_code: sansio_mqtt_v5_types::PubAckReasonCode },
    PublishCompleted { packet_id: core::num::NonZero<u16>, reason_code: sansio_mqtt_v5_types::PubCompReasonCode },
    PublishDropped { packet_id: core::num::NonZero<u16>, reason: sansio_mqtt_v5_protocol::PublishDroppedReason },
}
```

- [ ] **Step 10: Add failing loop poll behavior test**

```rust
#[tokio::test]
async fn poll_emits_connected_after_connack_flow() {
    // Use local TcpListener harness: accept connection, emit CONNACK bytes.
    // Assert EventLoop::poll returns Event::Connected.
}
```

- [ ] **Step 11: Run test and verify failure**

Run: `cargo test -p sansio-mqtt-v5-tokio poll_emits_connected_after_connack_flow -- --nocapture`

Expected: FAIL due to missing event loop orchestration.

- [ ] **Step 12: Implement EventLoop + connect bootstrap with minimal passing behavior**

```rust
pub async fn connect(options: ConnectOptions) -> Result<(Client, EventLoop), ConnectError> {
    let stream = tokio::net::TcpStream::connect(options.broker).await?;
    let mut protocol = sansio_mqtt_v5_protocol::Client::<tokio::time::Instant>::with_config(options.protocol_config);
    protocol.handle_write(sansio_mqtt_v5_protocol::UserWriteIn::Connect(options.connection))?;
    protocol.handle_event(sansio_mqtt_v5_protocol::DriverEventIn::SocketConnected)?;
    // flush poll_write frames to stream
    // create channels and return handles
}
```

- [ ] **Step 13: Run focused tests until green**

Run: `cargo test -p sansio-mqtt-v5-tokio --tests -- --nocapture`

Expected: PASS for new tests.

- [ ] **Step 14: Refactor and keep tests green**

Run: `cargo fmt && cargo test -p sansio-mqtt-v5-tokio --tests`

Expected: fmt clean + tests PASS.

- [ ] **Step 15: Commit Task 1**

```bash
git add crates/sansio-mqtt-v5-tokio/src crates/sansio-mqtt-v5-tokio/tests crates/sansio-mqtt-v5-tokio/Cargo.toml
git commit -m "feat(tokio): add split client and event loop API"
```

### Task 2: Migrate example to reconnecting echo mode

**Files:**
- Delete: `crates/sansio-mqtt-v5-tokio/examples/connect_test_mosquitto.rs`
- Create: `crates/sansio-mqtt-v5-tokio/examples/echo.rs`
- Create: `crates/sansio-mqtt-v5-tokio/tests/echo_example_contract.rs`

- [ ] **Step 1: Add failing contract test for env var requirements**

```rust
#[test]
fn echo_example_uses_broker_and_topic_env_vars() {
    let src = std::fs::read_to_string("crates/sansio-mqtt-v5-tokio/examples/echo.rs").unwrap();
    assert!(src.contains("BROKER"));
    assert!(src.contains("TOPIC"));
}
```

- [ ] **Step 2: Run test and verify failure**

Run: `cargo test -p sansio-mqtt-v5-tokio echo_example_uses_broker_and_topic_env_vars -- --nocapture`

Expected: FAIL because `echo.rs` does not exist yet.

- [ ] **Step 3: Add minimal `echo.rs` that reads env vars and compiles**

```rust
let broker = std::env::var("BROKER")?;
let topic = std::env::var("TOPIC")?;
```

- [ ] **Step 4: Add failing behavior test for stdin gating while disconnected**

```rust
#[test]
fn echo_driver_gates_stdin_on_connection_state() {
    // Assert source contains condition that only polls stdin when connected.
}
```

- [ ] **Step 5: Run test and verify failure**

Run: `cargo test -p sansio-mqtt-v5-tokio echo_driver_gates_stdin_on_connection_state -- --nocapture`

Expected: FAIL until reconnect-aware loop exists.

- [ ] **Step 6: Implement reconnecting event-loop driver in `echo.rs`**

```rust
loop {
    match sansio_mqtt_v5_tokio::connect(opts.clone()).await {
        Ok((client, mut event_loop)) => {
            connected = false;
            loop {
                tokio::select! {
                    evt = event_loop.poll() => { /* update connected, print received to stdout */ }
                    line = stdin_lines.next_line(), if connected => { /* publish */ }
                }
            }
        }
        Err(_) => tokio::time::sleep(retry_backoff).await,
    }
}
```

Constraints enforced by code:
- read stdin only when connected (`if connected` guard in `select!`)
- print broker payloads to stdout
- reconnect forever with delay after disconnect/error

- [ ] **Step 7: Remove old example and wire new one**

```bash
git rm crates/sansio-mqtt-v5-tokio/examples/connect_test_mosquitto.rs
```

- [ ] **Step 8: Run example compile and tests**

Run: `cargo test -p sansio-mqtt-v5-tokio --tests && cargo check -p sansio-mqtt-v5-tokio --examples`

Expected: PASS.

- [ ] **Step 9: Commit Task 2**

```bash
git add crates/sansio-mqtt-v5-tokio/examples crates/sansio-mqtt-v5-tokio/tests
git commit -m "feat(tokio): add reconnecting env-driven echo example"
```

### Task 3: Workspace verification and docs alignment

**Files:**
- Modify: `docs/superpowers/specs/2026-04-17-mqtt-v5-tokio-client-design.md` (only if implementation-driven clarifications are needed)

- [ ] **Step 1: Run required repo checks**

Run:

```bash
cargo fmt
cargo clippy -p sansio-mqtt-v5-tokio --all-targets --all-features
cargo test -p sansio-mqtt-v5-tokio
```

Expected: all commands succeed.

- [ ] **Step 2: Update spec wording only if implementation diverged intentionally**

```markdown
Record exact divergence and rationale with no TODO placeholders.
```

- [ ] **Step 3: Final commit (if needed)**

```bash
git add docs/superpowers/specs/2026-04-17-mqtt-v5-tokio-client-design.md
git commit -m "docs: align tokio design spec with implemented behavior"
```

## Spec coverage self-check

- Split explicit API (`Client` + `EventLoop`): covered by Task 1.
- `connect()` bootstrap and loop orchestration: covered by Task 1.
- Event mapping and error boundaries: covered by Task 1.
- Migration to `echo.rs` with `BROKER` and `TOPIC`: covered by Task 2.
- No stdin draining while disconnected: covered by Task 2.
- Reconnect mechanism in new driver: covered by Task 2.
- Required formatting/lint/tests: covered by Task 3.
