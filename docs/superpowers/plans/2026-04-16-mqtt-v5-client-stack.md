# MQTT v5 Client Stack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a no_std-first MQTT v5 client stack with a shared contract
crate, a statig-based state machine, and a protocol layer centered on
`sansio::Protocol` consumed by a tokio runtime adapter.

**Architecture:** Rename the existing `sansio-mqtt-v5-statig` crate to
`sansio-mqtt-v5-state-machine`, add `sansio-mqtt-v5-contract` for shared types
to avoid cyclic dependencies, implement session logic in the state machine, and
make `sansio-mqtt-v5-protocol` the canonical `sansio::Protocol` implementation.
Reuse open-source crates wherever feasible, especially for containers, state
management, timers, and channels.

**Tech Stack:** Rust workspace crates, `sansio-mqtt-v5-types`, `sansio`
(`1.0.1`), `statig`, `heapless`, `thiserror` (no_std mode), `tokio`, `bytes`,
`tokio-util` (DelayQueue in tokio crate), `futures`.

---

## File Structure

- Modify: `Cargo.toml`
- Rename: `crates/sansio-mqtt-v5-statig` ->
  `crates/sansio-mqtt-v5-state-machine`
- Modify: `crates/sansio-mqtt-v5-state-machine/Cargo.toml`
- Create: `crates/sansio-mqtt-v5-contract/Cargo.toml`
- Create: `crates/sansio-mqtt-v5-contract/src/lib.rs`
- Create:
  `crates/sansio-mqtt-v5-contract/src/{action.rs,error.rs,input.rs,options.rs,timer.rs}`
- Create: `crates/sansio-mqtt-v5-contract/tests/contracts.rs`
- Create:
  `crates/sansio-mqtt-v5-state-machine/src/{context.rs,states.rs,transitions.rs}`
- Modify: `crates/sansio-mqtt-v5-state-machine/src/lib.rs`
- Create:
  `crates/sansio-mqtt-v5-state-machine/tests/{block_a_connection.rs,block_b_keepalive.rs,block_cd_publish.rs,block_ef_subscribe_qos2.rs,block_gh_inbound_disconnect.rs}`
- Create: `crates/sansio-mqtt-v5-protocol/Cargo.toml`
- Create:
  `crates/sansio-mqtt-v5-protocol/src/{client.rs,error.rs,lib.rs,timer_queue.rs}`
- Create:
  `crates/sansio-mqtt-v5-protocol/tests/{timer_queue.rs,packet_id.rs,protocol_trait.rs,poll_decode.rs}`
- Create: `crates/sansio-mqtt-v5-tokio/Cargo.toml`
- Create:
  `crates/sansio-mqtt-v5-tokio/src/{driver.rs,lib.rs,timer.rs,transport.rs}`
- Create: `crates/sansio-mqtt-v5-tokio/tests/integration_mock_broker.rs`

### Task 1: Workspace rename and dependency baseline

**Files:**

- Modify: `Cargo.toml`
- Modify: `crates/sansio-mqtt-v5-state-machine/Cargo.toml`

- [ ] **Step 1: Write failing workspace metadata check**

```bash
cargo metadata --no-deps --format-version 1
```

Expected: FAIL or missing package names for `sansio-mqtt-v5-state-machine`,
`sansio-mqtt-v5-contract`, `sansio-mqtt-v5-protocol`, `sansio-mqtt-v5-tokio`, or
missing `sansio` dependency.

- [ ] **Step 2: Rename crate and update workspace dependency keys**

```toml
# Cargo.toml (workspace dependencies section)
[workspace.dependencies]
sansio-mqtt-v5-types = { path = "crates/sansio-mqtt-v5-types", default-features = false }
sansio-mqtt-v5-contract = { path = "crates/sansio-mqtt-v5-contract", default-features = false }
sansio-mqtt-v5-state-machine = { path = "crates/sansio-mqtt-v5-state-machine", default-features = false }
sansio-mqtt-v5-protocol = { path = "crates/sansio-mqtt-v5-protocol", default-features = false }
sansio-mqtt-v5-tokio = { path = "crates/sansio-mqtt-v5-tokio", default-features = false }

heapless = { version = "0.8", default-features = false }
thiserror = { version = "2", default-features = false }
statig = { version = "0.4.1", default-features = false }
tokio = { version = "1", default-features = false }
tokio-util = { version = "0.7", default-features = false }
futures = { version = "0.3", default-features = false }
bytes = { version = "1", default-features = false }
sansio = { version = "1.0.1", default-features = false }
```

```toml
# crates/sansio-mqtt-v5-state-machine/Cargo.toml
[package]
name = "sansio-mqtt-v5-state-machine"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
```

- [ ] **Step 3: Run metadata to verify all target crates are visible**

```bash
cargo metadata --no-deps --format-version 1
```

Expected: PASS, JSON includes all five package names and resolves `sansio`.

- [ ] **Step 4: Run compile gate for renamed crate**

```bash
cargo build -p sansio-mqtt-v5-state-machine
```

Expected: PASS.

- [ ] **Step 5: Commit rename baseline**

```bash
git add Cargo.toml crates/sansio-mqtt-v5-state-machine/Cargo.toml
git commit -m "chore: rename statig crate and align workspace dependencies"
```

### Task 2: Create `sansio-mqtt-v5-contract` with shared boundary types

**Files:**

- Create: `crates/sansio-mqtt-v5-contract/Cargo.toml`
- Create: `crates/sansio-mqtt-v5-contract/src/lib.rs`
- Create:
  `crates/sansio-mqtt-v5-contract/src/{action.rs,error.rs,input.rs,options.rs,timer.rs}`
- Test: `crates/sansio-mqtt-v5-contract/tests/contracts.rs`

- [ ] **Step 1: Write failing contract tests**

```rust
// crates/sansio-mqtt-v5-contract/tests/contracts.rs
use sansio_mqtt_v5_contract::{Action, ConnectOptions, Input, TimerKey};

#[test]
fn constructs_core_contract_types() {
    let _opts = ConnectOptions::default();
    let _timer = TimerKey::Keepalive;
    let bytes: &[u8] = &[0x10, 0x00];
    let _input = Input::BytesReceived(bytes);
    let _action = Action::CancelTimer(TimerKey::ConnectTimeout);
}
```

- [ ] **Step 2: Implement minimal contract crate using OSS no_std dependencies**

```rust
// crates/sansio-mqtt-v5-contract/src/lib.rs
#![no_std]
#![forbid(unsafe_code)]

pub mod action;
pub mod error;
pub mod input;
pub mod options;
pub mod timer;

pub use action::{Action, SessionAction};
pub use error::{DisconnectReason, ProtocolError};
pub use input::Input;
pub use options::{ConnectOptions, PublishRequest, SubscribeRequest};
pub use timer::TimerKey;
```

```rust
// crates/sansio-mqtt-v5-contract/src/timer.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimerKey {
    Keepalive,
    PingRespTimeout,
    AckTimeout(u16),
    ConnectTimeout,
}
```

- [ ] **Step 3: Run contract tests**

```bash
cargo test -p sansio-mqtt-v5-contract
```

Expected: PASS.

- [ ] **Step 4: Run compile gate**

```bash
cargo build -p sansio-mqtt-v5-contract
```

Expected: PASS.

- [ ] **Step 5: Commit contract crate**

```bash
git add crates/sansio-mqtt-v5-contract Cargo.toml
git commit -m "feat: add shared mqtt v5 contract crate"
```

### Task 3: Scaffold state-machine internals and block A tests

**Files:**

- Modify: `crates/sansio-mqtt-v5-state-machine/Cargo.toml`
- Modify: `crates/sansio-mqtt-v5-state-machine/src/lib.rs`
- Create:
  `crates/sansio-mqtt-v5-state-machine/src/{context.rs,states.rs,transitions.rs}`
- Test: `crates/sansio-mqtt-v5-state-machine/tests/block_a_connection.rs`

- [ ] **Step 1: Write failing block A tests (connect lifecycle)**

```rust
// crates/sansio-mqtt-v5-state-machine/tests/block_a_connection.rs
use sansio_mqtt_v5_contract::{Action, Input, TimerKey};
use sansio_mqtt_v5_state_machine::StateMachine;

#[test]
fn user_connect_emits_send_and_connect_timeout() {
    let mut sm = StateMachine::new_default();
    let actions = sm.handle(Input::UserConnect(Default::default()));
    assert!(actions.iter().any(|a| matches!(a, Action::SendBytes(_))));
    assert!(actions.iter().any(|a| matches!(a, Action::ScheduleTimer { key: TimerKey::ConnectTimeout, .. })));
}
```

- [ ] **Step 2: Implement state skeleton (context + states + API)**

```rust
// crates/sansio-mqtt-v5-state-machine/src/lib.rs
#![no_std]
#![forbid(unsafe_code)]

mod context;
mod states;
mod transitions;

use heapless::Vec;
use sansio_mqtt_v5_contract::{Action, Input};

pub struct StateMachine {
    inner: states::MachineState,
    ctx: context::Context,
}

impl StateMachine {
    pub fn new_default() -> Self {
        Self { inner: states::MachineState::Disconnected, ctx: context::Context::default() }
    }

    pub fn handle(&mut self, input: Input<'_>) -> Vec<Action, 8> {
        transitions::dispatch(&mut self.inner, &mut self.ctx, input)
    }
}
```

- [ ] **Step 3: Run block A tests to verify minimum behavior**

```bash
cargo test -p sansio-mqtt-v5-state-machine --test block_a_connection
```

Expected: PASS.

- [ ] **Step 4: Run compile gate**

```bash
cargo build -p sansio-mqtt-v5-state-machine
```

Expected: PASS.

- [ ] **Step 5: Commit state-machine scaffold**

```bash
git add crates/sansio-mqtt-v5-state-machine
git commit -m "feat: scaffold no-std mqtt v5 state machine"
```

### Task 4: Implement blocks B, C, D (keepalive + QoS0/QoS1 outbound)

**Files:**

- Modify:
  `crates/sansio-mqtt-v5-state-machine/src/{context.rs,states.rs,transitions.rs,lib.rs}`
- Test: `crates/sansio-mqtt-v5-state-machine/tests/block_b_keepalive.rs`
- Test: `crates/sansio-mqtt-v5-state-machine/tests/block_cd_publish.rs`

- [ ] **Step 1: Write failing keepalive and QoS1 retry tests**

```rust
// block_b_keepalive.rs
use sansio_mqtt_v5_contract::{Action, Input, TimerKey};

#[test]
fn keepalive_timeout_sends_pingreq_then_waits_for_pingresp() {
    let mut sm = sansio_mqtt_v5_state_machine::StateMachine::new_default_connected();
    let actions = sm.handle(Input::TimerFired(TimerKey::Keepalive));
    assert!(actions.iter().any(|a| matches!(a, Action::SendBytes(_))));
    assert!(actions.iter().any(|a| matches!(a, Action::ScheduleTimer { key: TimerKey::PingRespTimeout, .. })));
}

// block_cd_publish.rs
#[test]
fn qos1_publish_schedules_ack_timeout_and_retries_on_timeout() {
    let mut sm = sansio_mqtt_v5_state_machine::StateMachine::new_default_connected();
    let req = sansio_mqtt_v5_contract::PublishRequest::qos1("sensor/temp", b"21");
    let first = sm.handle(Input::UserPublish(req.clone()));
    let packet_id = first
        .iter()
        .find_map(|a| match a {
            Action::ScheduleTimer { key: TimerKey::AckTimeout(id), .. } => Some(*id),
            _ => None,
        })
        .expect("missing ack timeout");
    let retry = sm.handle(Input::TimerFired(TimerKey::AckTimeout(packet_id)));
    assert!(retry.iter().any(|a| matches!(a, Action::SendBytes(_))));
    assert!(retry.iter().any(|a| matches!(a, Action::ScheduleTimer { key: TimerKey::AckTimeout(id), .. } if *id == packet_id)));
}
```

- [ ] **Step 2: Implement transition logic for B/C/D**

```rust
// transitions.rs
match (state, input) {
    (MachineState::Idle, Input::TimerFired(TimerKey::Keepalive)) => {
        push_pingreq(&mut actions);
        schedule(&mut actions, TimerKey::PingRespTimeout, ctx.ping_timeout_ms);
        *state = MachineState::WaitingForPingResp;
    }
    (MachineState::Idle, Input::UserPublish(req)) if req.qos == 0 => {
        push_publish(&mut actions, req, None, false);
    }
    (MachineState::Idle, Input::UserPublish(req)) if req.qos == 1 => {
        let packet_id = ctx.alloc_packet_id();
        ctx.store_pending_qos1(packet_id, &req);
        push_publish(&mut actions, req, Some(packet_id), false);
        schedule(&mut actions, TimerKey::AckTimeout(packet_id), ctx.ack_timeout_ms);
        *state = MachineState::WaitingForPubAck { packet_id };
    }
    _ => {}
}
```

- [ ] **Step 3: Run focused tests**

```bash
cargo test -p sansio-mqtt-v5-state-machine --test block_b_keepalive
cargo test -p sansio-mqtt-v5-state-machine --test block_cd_publish
```

Expected: PASS.

- [ ] **Step 4: Run compile gate**

```bash
cargo build -p sansio-mqtt-v5-state-machine
```

Expected: PASS.

- [ ] **Step 5: Commit B/C/D transitions**

```bash
git add crates/sansio-mqtt-v5-state-machine/src crates/sansio-mqtt-v5-state-machine/tests
git commit -m "feat: implement keepalive and outbound qos0/qos1 transitions"
```

### Task 5: Implement blocks E and F (QoS2 outbound + subscribe)

**Files:**

- Modify:
  `crates/sansio-mqtt-v5-state-machine/src/{context.rs,states.rs,transitions.rs}`
- Test: `crates/sansio-mqtt-v5-state-machine/tests/block_ef_subscribe_qos2.rs`

- [ ] **Step 1: Write failing tests for QoS2 phase flow and suback handling**

```rust
#[test]
fn qos2_publish_goes_pubrec_pubrel_pubcomp() {
    use sansio_mqtt_v5_contract::{Action, Input, TimerKey};
    let mut sm = sansio_mqtt_v5_state_machine::StateMachine::new_default_connected();
    let req = sansio_mqtt_v5_contract::PublishRequest::qos2("sensor/temp", b"21");
    let sent = sm.handle(Input::UserPublish(req));
    let pid = sent.iter().find_map(|a| match a {
        Action::ScheduleTimer { key: TimerKey::AckTimeout(id), .. } => Some(*id),
        _ => None,
    }).expect("ack timeout not scheduled");

    let on_pubrec = sm.handle(Input::PacketPubRec { packet_id: pid });
    assert!(on_pubrec.iter().any(|a| matches!(a, Action::SendBytes(_))));

    let on_pubcomp = sm.handle(Input::PacketPubComp { packet_id: pid });
    assert!(on_pubcomp.iter().any(|a| matches!(a, Action::CancelTimer(TimerKey::AckTimeout(id)) if *id == pid)));
}

#[test]
fn subscribe_emits_send_and_suback_session_action() {
    use sansio_mqtt_v5_contract::{Action, Input, SessionAction, TimerKey};
    let mut sm = sansio_mqtt_v5_state_machine::StateMachine::new_default_connected();
    let req = sansio_mqtt_v5_contract::SubscribeRequest::single("sensor/#", 1);
    let out = sm.handle(Input::UserSubscribe(req));
    assert!(out.iter().any(|a| matches!(a, Action::SendBytes(_))));
    let pid = out.iter().find_map(|a| match a {
        Action::ScheduleTimer { key: TimerKey::AckTimeout(id), .. } => Some(*id),
        _ => None,
    }).expect("sub ack timer missing");

    let ack = sm.handle(Input::PacketSubAck { packet_id: pid });
    assert!(ack.iter().any(|a| matches!(a, Action::SessionAction(SessionAction::SubscribeAck { packet_id }) if *packet_id == pid)));
}
```

- [ ] **Step 2: Implement minimal E/F transitions with `heapless::FnvIndexMap`**

```rust
// context.rs
pub struct Context {
    pub pending_qos2: heapless::FnvIndexMap<u16, Qos2Stage, 16>,
    pub pending_acks: heapless::FnvIndexMap<u16, PendingAck, 16>,
    pub ack_timeout_ms: u32,
}

pub enum Qos2Stage {
    WaitingForPubRec,
    WaitingForPubComp,
}
```

- [ ] **Step 3: Run E/F tests**

```bash
cargo test -p sansio-mqtt-v5-state-machine --test block_ef_subscribe_qos2
```

Expected: PASS.

- [ ] **Step 4: Compile gate**

```bash
cargo build -p sansio-mqtt-v5-state-machine
```

Expected: PASS.

- [ ] **Step 5: Commit E/F transitions**

```bash
git add crates/sansio-mqtt-v5-state-machine
git commit -m "feat: implement qos2 outbound and subscribe transitions"
```

### Task 6: Implement blocks G and H (inbound publish + disconnect)

**Files:**

- Modify: `crates/sansio-mqtt-v5-state-machine/src/transitions.rs`
- Test:
  `crates/sansio-mqtt-v5-state-machine/tests/block_gh_inbound_disconnect.rs`

- [ ] **Step 1: Write failing tests for inbound qos handling and user
      disconnect**

```rust
#[test]
fn inbound_qos1_publish_sends_puback_and_session_action() {
    use sansio_mqtt_v5_contract::{Action, Input, SessionAction};
    let mut sm = sansio_mqtt_v5_state_machine::StateMachine::new_default_connected();
    let publish = Input::PacketPublishQos1 {
        packet_id: 10,
        topic: "sensor/temp",
        payload: b"22",
    };
    let out = sm.handle(publish);
    assert!(out.iter().any(|a| matches!(a, Action::SendBytes(_))));
    assert!(out.iter().any(|a| matches!(a, Action::SessionAction(SessionAction::PublishReceived { .. }))));
}

#[test]
fn user_disconnect_cancels_timers_and_transitions_disconnected() {
    use sansio_mqtt_v5_contract::{Action, Input};
    let mut sm = sansio_mqtt_v5_state_machine::StateMachine::new_default_connected();
    let out = sm.handle(Input::UserDisconnect);
    assert!(out.iter().any(|a| matches!(a, Action::SendBytes(_))));
    assert!(out.iter().any(|a| matches!(a, Action::CancelTimer(_))));
}
```

- [ ] **Step 2: Implement G/H transitions and timer cancellation fan-out**

```rust
// transitions.rs
if let Input::UserDisconnect = input {
    push_disconnect(&mut actions);
    cancel_all_timers(&mut actions, ctx);
    emit_disconnected(&mut actions);
    *state = MachineState::Disconnected;
}
```

- [ ] **Step 3: Run G/H and full crate tests**

```bash
cargo test -p sansio-mqtt-v5-state-machine --test block_gh_inbound_disconnect
cargo test -p sansio-mqtt-v5-state-machine
```

Expected: PASS.

- [ ] **Step 4: Compile gate**

```bash
cargo build -p sansio-mqtt-v5-state-machine
```

Expected: PASS.

- [ ] **Step 5: Commit G/H transitions**

```bash
git add crates/sansio-mqtt-v5-state-machine
git commit -m "feat: implement inbound publish and disconnect transitions"
```

### Task 7: Create protocol crate skeleton implementing `sansio::Protocol`

**Files:**

- Create: `crates/sansio-mqtt-v5-protocol/Cargo.toml`
- Create:
  `crates/sansio-mqtt-v5-protocol/src/{lib.rs,error.rs,timer_queue.rs,client.rs}`
- Test:
  `crates/sansio-mqtt-v5-protocol/tests/{timer_queue.rs,packet_id.rs,protocol_trait.rs}`

- [ ] **Step 1: Write failing timer queue, packet-id, and trait conformance
      tests**

```rust
#[test]
fn timer_queue_returns_expired_key_at_or_after_now() {
    let mut q = sansio_mqtt_v5_protocol::TimerQueue::new();
    q.insert(sansio_mqtt_v5_contract::TimerKey::Keepalive, 100).unwrap();
    assert_eq!(q.expired(99), None);
    assert_eq!(q.expired(100), Some(sansio_mqtt_v5_contract::TimerKey::Keepalive));
}

#[test]
fn packet_id_skips_zero_and_wraps() {
    let mut p = sansio_mqtt_v5_protocol::MqttProtocol::new_default();
    p.set_packet_id_counter(u16::MAX - 1);
    assert_eq!(p.next_packet_id().unwrap(), u16::MAX);
    assert_eq!(p.next_packet_id().unwrap(), 1);
}

#[test]
fn protocol_exposes_sansio_push_pull_surfaces() {
    use sansio::Protocol;
    fn assert_protocol<T: Protocol<Vec<u8>, (), sansio_mqtt_v5_contract::UserCommand>>() {}
    assert_protocol::<sansio_mqtt_v5_protocol::MqttProtocol>();
}
```

- [ ] **Step 2: Implement `MqttProtocol` skeleton and no_std timer queue**

```rust
// timer_queue.rs
use heapless::Vec;
use sansio_mqtt_v5_contract::TimerKey;

#[derive(Clone, Copy)]
struct Entry { key: TimerKey, deadline_ms: u32 }

pub struct TimerQueue {
    entries: Vec<Entry, 32>,
}
```

```rust
// client.rs
impl sansio::Protocol<InboundRead, InboundWrite, UserCommand> for MqttProtocol {
    type Rout = SessionAction;
    type Wout = OutboundBytes;
    type Eout = SessionAction;
    type Error = ProtocolError;
    type Time = u32;

    fn handle_read(&mut self, msg: InboundRead) -> Result<(), Self::Error> {
        let actions = self.drive_read(msg)?;
        self.enqueue_actions(actions)
    }
    fn poll_read(&mut self) -> Option<Self::Rout> { None }
    fn handle_write(&mut self, msg: InboundWrite) -> Result<(), Self::Error> {
        let actions = self.drive_write(msg)?;
        self.enqueue_actions(actions)
    }
    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.write_queue.pop_front()
    }
    fn handle_event(&mut self, evt: UserCommand) -> Result<(), Self::Error> {
        let actions = self.drive_input(Input::from(evt))?;
        self.enqueue_actions(actions)
    }
    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.event_queue.pop_front()
    }
    fn handle_timeout(&mut self, now: u32) -> Result<(), Self::Error> {
        while let Some(key) = self.timers.expired(now) {
            let actions = self.drive_input(Input::TimerFired(key))?;
            self.enqueue_actions(actions)?;
        }
        Ok(())
    }
    fn poll_timeout(&mut self) -> Option<u32> {
        self.timers.next_deadline()
    }
}
```

- [ ] **Step 3: Implement `next_packet_id` exhaustion-safe logic**

```rust
pub fn next_packet_id(&mut self) -> Result<u16, ProtocolError> {
    for _ in 0..u16::MAX {
        self.packet_id_counter = self.packet_id_counter.wrapping_add(1);
        if self.packet_id_counter != 0 {
            return Ok(self.packet_id_counter);
        }
    }
    Err(ProtocolError::PacketIdExhausted)
}
```

- [ ] **Step 4: Run tests and compile gate**

```bash
cargo test -p sansio-mqtt-v5-protocol --test timer_queue
cargo test -p sansio-mqtt-v5-protocol --test packet_id
cargo test -p sansio-mqtt-v5-protocol --test protocol_trait
cargo build -p sansio-mqtt-v5-protocol
```

Expected: PASS.

- [ ] **Step 5: Commit protocol core utilities**

```bash
git add crates/sansio-mqtt-v5-protocol Cargo.toml
git commit -m "feat: add protocol timer queue and packet id allocator"
```

### Task 8: Implement protocol decode/dispatch through sansio push-pull methods

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/client.rs`
- Test: `crates/sansio-mqtt-v5-protocol/tests/poll_decode.rs`

- [ ] **Step 1: Write failing poll bridge tests**

```rust
#[test]
fn bytes_received_decodes_and_drives_state_machine() {
    use sansio::Protocol;
    let mut p = sansio_mqtt_v5_protocol::MqttProtocol::new_default();
    p.handle_read(vec![0x20, 0x03, 0x00, 0x00, 0x00]).unwrap(); // minimal CONNACK frame
    assert!(p.poll_event().is_some());
}

#[test]
fn schedule_and_cancel_timer_actions_update_internal_queue() {
    let mut p = sansio_mqtt_v5_protocol::MqttProtocol::new_default();
    p.schedule_timer_for_test(sansio_mqtt_v5_contract::TimerKey::ConnectTimeout, 250);
    assert_eq!(p.poll_timeout(), Some(250));
    p.cancel_timer_for_test(sansio_mqtt_v5_contract::TimerKey::ConnectTimeout);
    assert_eq!(p.poll_timeout(), None);
}

#[test]
fn user_event_enters_via_handle_event_and_emits_poll_write() {
    use sansio::Protocol;
    let mut p = sansio_mqtt_v5_protocol::MqttProtocol::new_default();
    p.handle_event(sansio_mqtt_v5_contract::UserCommand::Connect(Default::default())).unwrap();
    assert!(p.poll_write().is_some());
}
```

- [ ] **Step 2: Implement `handle_read`/`handle_event` to drive state machine**

```rust
fn handle_event(&mut self, evt: UserCommand) -> Result<(), Self::Error> {
    let actions = self.drive_input(Input::from(evt))?;
    self.enqueue_actions(actions)?;
    Ok(())
}
```

- [ ] **Step 3: Implement timeout mapping via `poll_timeout` +
      `handle_timeout`**

```rust
fn handle_timeout(&mut self, now: u32) -> Result<(), Self::Error> {
    while let Some(key) = self.timers.expired(now) {
        let actions = self.drive_input(Input::TimerFired(key))?;
        self.enqueue_actions(actions)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests and compile gate**

```bash
cargo test -p sansio-mqtt-v5-protocol --test poll_decode
cargo test -p sansio-mqtt-v5-protocol
cargo build -p sansio-mqtt-v5-protocol
```

Expected: PASS.

- [ ] **Step 5: Commit protocol poll implementation**

```bash
git add crates/sansio-mqtt-v5-protocol
git commit -m "feat: implement sansio protocol decode event and timeout orchestration"
```

### Task 9: Create tokio runtime adapter using existing async crates

**Files:**

- Create: `crates/sansio-mqtt-v5-tokio/Cargo.toml`
- Create:
  `crates/sansio-mqtt-v5-tokio/src/{lib.rs,timer.rs,transport.rs,driver.rs}`
- Test: `crates/sansio-mqtt-v5-tokio/tests/integration_mock_broker.rs`

- [ ] **Step 1: Write failing integration test with mock broker**

```rust
#[tokio::test]
async fn connect_publish_subscribe_flow_emits_session_actions() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut buf = [0u8; 1024];
        let _n = socket.read(&mut buf).await.unwrap();
        socket.write_all(&[0x20, 0x03, 0x00, 0x00, 0x00]).await.unwrap(); // CONNACK
    });
    let (_client, mut session_rx) = sansio_mqtt_v5_tokio::TokioClient::connect(addr, Default::default()).await.unwrap();
    let evt = session_rx.recv().await;
    assert!(evt.is_some());
}
```

- [ ] **Step 2: Implement transport split and channels with `tokio` + `bytes`**

```rust
// transport.rs
let (read_half, write_half) = stream.into_split();
let (read_tx, read_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(32);
let (write_tx, write_rx) = tokio::sync::mpsc::channel::<bytes::Bytes>(32);
```

- [ ] **Step 3: Implement timer abstraction with
      `tokio_util::time::DelayQueue`**

```rust
// timer.rs
pub struct TimerMap {
    queue: tokio_util::time::DelayQueue<sansio_mqtt_v5_contract::TimerKey>,
    slots: std::collections::HashMap<sansio_mqtt_v5_contract::TimerKey, tokio_util::time::delay_queue::Key>,
}
```

- [ ] **Step 4: Implement driver select loop around sansio trait methods**

```rust
tokio::select! {
    Some(bytes) = read_rx.recv() => { protocol.handle_read(bytes)?; }
    Some(req) = user_rx.recv() => { protocol.handle_event(req)?; }
    _ = next_timeout_sleep => { protocol.handle_timeout(now_ticks())?; }
}
while let Some(out) = protocol.poll_write() { write_tx.send(out).await?; }
while let Some(evt) = protocol.poll_event() { session_tx.send(evt).await?; }
```

- [ ] **Step 5: Run tests and compile gate**

```bash
cargo test -p sansio-mqtt-v5-tokio
cargo build -p sansio-mqtt-v5-tokio
```

Expected: PASS.

- [ ] **Step 6: Commit tokio adapter**

```bash
git add crates/sansio-mqtt-v5-tokio Cargo.toml
git commit -m "feat: add tokio driver for mqtt v5 protocol client"
```

### Task 10: Final verification and no_std compliance

**Files:**

- Modify: any touched files from previous tasks

- [ ] **Step 1: Run formatting and linting gates**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
```

Expected: PASS.

- [ ] **Step 2: Run full build matrix**

```bash
cargo build -p sansio-mqtt-v5-contract
cargo build -p sansio-mqtt-v5-state-machine
cargo build -p sansio-mqtt-v5-protocol
cargo build -p sansio-mqtt-v5-tokio
```

Expected: PASS.

- [ ] **Step 3: Run full test matrix**

```bash
cargo test -p sansio-mqtt-v5-contract
cargo test -p sansio-mqtt-v5-state-machine
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
```

Expected: PASS.

- [ ] **Step 4: Verify `std` is not imported in no_std crates**

```bash
rg "use std" crates/sansio-mqtt-v5-protocol crates/sansio-mqtt-v5-state-machine crates/sansio-mqtt-v5-contract
```

Expected: no matches.

- [ ] **Step 5: Commit final verification fixes**

```bash
git add -A
git commit -m "chore: pass formatting linting and verification gates"
```

## Dependency-First Rules for Implementation

- Prefer existing crates before writing custom infrastructure:
  - `heapless`: fixed-capacity collections and maps in no_std crates
  - `sansio`: canonical protocol push/pull trait and timeout/event interface
  - `statig`: state-machine orchestration
  - `thiserror`: error enums without manual boilerplate
  - `tokio-util::time::DelayQueue`: runtime timer scheduling in tokio crate
  - `bytes`: byte buffer ownership/transfer in async channels
- Only write custom code when:
  - behavior is MQTT-domain-specific, or
  - no suitable no_std-compatible crate exists.
- Any custom utility added must be justified in PR notes with "why existing
  crate is insufficient".

## Notes on MQTT Spec Traceability

- During implementation, annotate key transition/test assertions with MQTT
  conformance tags (for example `[MQTT-3.1.2-24]`) where relevant.
- Keep tags in tests close to assertions so behavior and spec references stay
  aligned.
