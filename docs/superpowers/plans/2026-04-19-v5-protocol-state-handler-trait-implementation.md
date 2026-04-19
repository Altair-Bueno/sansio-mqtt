# v5-Protocol `StateHandler` Trait Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Break `proto.rs` (1815 lines) into focused modules using a `StateHandler<Time>` trait — one struct per MQTT lifecycle state — while keeping all existing tests and spec compliance intact.

**Architecture:** Incremental extraction: pull helpers into sibling modules (`limits`, `queues`, `session_ops`, `scratchpad`, `session`) while `proto.rs` remains the live dispatcher; then introduce `state/` (FSM enum + trait + four state structs); finally cut `Client`'s `Protocol` impl over to a `dispatch()` helper and delete `proto.rs`. Every intermediate commit keeps `cargo test --workspace` green.

**Tech Stack:** Rust 2021 nightly, `sansio_mqtt_v5_types`, `winnow`, `bytes`, `encode`, `tracing` — no new dependencies.

---

## Non-negotiable constraints

- `#![forbid(unsafe_code)]` preserved in `lib.rs`.
- `no_std` preserved; no `std::` imports outside `cfg(test)`.
- `Client<Time>` and `sansio::Protocol for Client<Time>` exported with unchanged method signatures.
- All `[MQTT-x.x.x-y]` spec citations preserved at every call site through all tasks.
- `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` green after every task.

## Files Map

| Path | Action | Notes |
|---|---|---|
| `src/proto.rs` | Delete in Task 13 | Replaced by modules below |
| `src/lib.rs` | Modify across tasks | Re-exports updated |
| `src/client.rs` | Create in Task 13 | `Client<Time>`, constructors, `dispatch`, `Protocol` impl |
| `src/scratchpad.rs` | Create in Task 6 | `ClientScratchpad<Time>` + `ClientLifecycleState` + `Default` |
| `src/session.rs` | Create in Task 7 | `ClientSession`, `OutboundInflightState`, `InboundInflightState` |
| `src/limits.rs` | Create in Task 3 | limit computation + outbound validation free functions |
| `src/queues.rs` | Create in Task 4 | encode/enqueue helpers + `fail_protocol_and_disconnect` |
| `src/session_ops.rs` | Create in Task 5 | packet-id alloc, replay, inflight reset, keepalive reset |
| `src/state/mod.rs` | Create in Task 8 | `enum ClientState`, `trait StateHandler<Time>`, dispatch impl |
| `src/state/start.rs` | Create in Task 9 | `struct Start` + `impl StateHandler<Time>` |
| `src/state/disconnected.rs` | Create in Task 9 | `struct Disconnected` + `impl StateHandler<Time>` |
| `src/state/connecting.rs` | Create in Task 10 | `struct Connecting { pending_connect_options }` + impl |
| `src/state/connected.rs` | Create in Task 11 | `struct Connected` + impl (bulk of packet dispatch) |

---

### Task 1: Verify baseline

**Files:** read-only

- [ ] **Step 1: Confirm rust-analyzer is available**

Run: `rust-analyzer --version`
Expected: a version string. If missing, run `rustup component add rust-analyzer` and confirm before continuing.

- [ ] **Step 2: Confirm clean baseline**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
All must pass. If any fail, stop and report before touching code.

---

### Task 2: Rename `ClientState` → `ClientSession`

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

Rename the persistent struct and constructor, and rename the `state` field on `Client` to `session`. This frees the name `ClientState` for the FSM enum introduced in Task 8.

- [ ] **Step 1: Rename struct, impls, and field in `proto.rs`**

Apply these replacements in `proto.rs`:

```
pub struct ClientState {          →  pub struct ClientSession {
impl ClientState {                →  impl ClientSession {
impl Default for ClientState      →  impl Default for ClientSession
ClientState::default()            →  ClientSession::default()
state: ClientState,               →  session: ClientSession,    (field on Client<Time>)
with_settings_and_state(          →  with_settings_and_session(
state: ClientState)               →  session: ClientSession)    (constructor param)
Self::with_settings_and_state(    →  Self::with_settings_and_session(
```

Also rename the local parameter inside `with_settings_and_session`:
```rust
// before
pub fn with_settings_and_state(settings: ClientSettings, state: ClientState) -> Self {
    let mut client = Self { settings, state, scratchpad: ClientScratchpad::default() };

// after
pub fn with_settings_and_session(settings: ClientSettings, session: ClientSession) -> Self {
    let mut client = Self { settings, session, scratchpad: ClientScratchpad::default() };
```

Rename every `self.state.` access that targets the session fields to `self.session.`:

```
self.state.on_flight_sent        →  self.session.on_flight_sent
self.state.on_flight_received    →  self.session.on_flight_received
self.state.pending_subscribe     →  self.session.pending_subscribe
self.state.pending_unsubscribe   →  self.session.pending_unsubscribe
self.state.inbound_topic_aliases →  self.session.inbound_topic_aliases
self.state.next_packet_id        →  self.session.next_packet_id
```

- [ ] **Step 2: Update `lib.rs` re-export**

```rust
// before
pub use proto::ClientState;
// after
pub use proto::ClientSession;
```

- [ ] **Step 3: Update `tests/client_protocol.rs`**

```
sansio_mqtt_v5_protocol::ClientState  →  sansio_mqtt_v5_protocol::ClientSession
Client::<u64>::with_settings_and_state(  →  Client::<u64>::with_settings_and_session(
```

- [ ] **Step 4: Run tests**

```bash
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs \
        crates/sansio-mqtt-v5-protocol/src/lib.rs \
        crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "refactor(v5-protocol): rename ClientState → ClientSession, state field → session"
```

---

### Task 3: Extract `limits.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/limits.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`

Move all limit computation and outbound validation methods to free functions. These are called by `enqueue_packet` (extracted in Task 4) and inline in `proto.rs`.

- [ ] **Step 1: Create `src/limits.rs`**

```rust
use core::num::NonZero;

use sansio_mqtt_v5_types::{MaximumQoS, Publish};

use crate::proto::{ClientScratchpad, ClientSession};
use crate::types::{ClientMessage, ClientSettings, Error};

pub(crate) fn min_option_nonzero_u16(
    a: Option<NonZero<u16>>,
    b: Option<NonZero<u16>>,
) -> Option<NonZero<u16>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a.get() <= b.get() { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(crate) fn min_option_nonzero_u32(
    a: Option<NonZero<u32>>,
    b: Option<NonZero<u32>>,
) -> Option<NonZero<u32>> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a.get() <= b.get() { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn min_option_maximum_qos(a: Option<MaximumQoS>, b: Option<MaximumQoS>) -> Option<MaximumQoS> {
    match (a, b) {
        (Some(MaximumQoS::AtMostOnce), _) | (_, Some(MaximumQoS::AtMostOnce)) => {
            Some(MaximumQoS::AtMostOnce)
        }
        (Some(MaximumQoS::AtLeastOnce), Some(MaximumQoS::AtLeastOnce)) => {
            Some(MaximumQoS::AtLeastOnce)
        }
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

pub(crate) fn recompute_effective_limits<Time: 'static>(
    settings: &ClientSettings,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    scratchpad.effective_client_max_bytes_string = settings.max_bytes_string;
    scratchpad.effective_client_max_bytes_binary_data = settings.max_bytes_binary_data;
    scratchpad.effective_client_max_remaining_bytes = settings.max_remaining_bytes.min(
        scratchpad
            .effective_client_maximum_packet_size
            .map(|x| u64::from(x.get()))
            .unwrap_or(u64::MAX),
    );
    scratchpad.effective_client_max_subscriptions_len = settings.max_subscriptions_len;
    scratchpad.effective_client_max_user_properties_len = settings.max_user_properties_len;
    scratchpad.effective_client_max_subscription_identifiers_len =
        settings.max_subscription_identifiers_len;

    scratchpad.effective_client_receive_maximum = min_option_nonzero_u16(
        settings.max_incoming_receive_maximum,
        scratchpad.pending_connect_options.receive_maximum,
    )
    .unwrap_or(NonZero::new(u16::MAX).expect("u16::MAX is always non-zero"));

    scratchpad.effective_client_maximum_packet_size = min_option_nonzero_u32(
        settings.max_incoming_packet_size,
        scratchpad.pending_connect_options.maximum_packet_size,
    );

    scratchpad.effective_client_topic_alias_maximum = settings
        .max_incoming_topic_alias_maximum
        .unwrap_or(u16::MAX)
        .min(
            scratchpad
                .pending_connect_options
                .topic_alias_maximum
                .or(settings.max_incoming_topic_alias_maximum)
                .unwrap_or(0),
        );

    scratchpad.effective_broker_receive_maximum = scratchpad.negotiated_receive_maximum;
    scratchpad.effective_broker_maximum_packet_size = scratchpad.negotiated_maximum_packet_size;
    scratchpad.effective_broker_topic_alias_maximum = scratchpad.negotiated_topic_alias_maximum;
    scratchpad.effective_broker_maximum_qos = min_option_maximum_qos(
        settings.max_outgoing_qos,
        scratchpad.negotiated_maximum_qos,
    );
    scratchpad.effective_retain_available =
        settings.allow_retain && scratchpad.negotiated_retain_available;
    scratchpad.effective_wildcard_subscription_available =
        settings.allow_wildcard_subscriptions
            && scratchpad.negotiated_wildcard_subscription_available;
    scratchpad.effective_shared_subscription_available =
        settings.allow_shared_subscriptions && scratchpad.negotiated_shared_subscription_available;
    scratchpad.effective_subscription_identifiers_available =
        settings.allow_subscription_identifiers
            && scratchpad.negotiated_subscription_identifiers_available;
}

pub(crate) fn reset_negotiated_limits<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    scratchpad.negotiated_receive_maximum =
        NonZero::new(u16::MAX).expect("u16::MAX is always non-zero for receive_maximum");
    scratchpad.negotiated_maximum_packet_size = None;
    scratchpad.negotiated_topic_alias_maximum = 0;
    scratchpad.negotiated_server_keep_alive = None;
    scratchpad.negotiated_maximum_qos = None;
    scratchpad.negotiated_retain_available = true;
    scratchpad.negotiated_wildcard_subscription_available = true;
    scratchpad.negotiated_shared_subscription_available = true;
    scratchpad.negotiated_subscription_identifiers_available = true;
    session.inbound_topic_aliases.clear();
    recompute_effective_limits(settings, scratchpad);
}

pub(crate) fn ensure_outbound_receive_maximum_capacity<Time: 'static>(
    session: &ClientSession,
    scratchpad: &ClientScratchpad<Time>,
) -> Result<(), Error> {
    // [MQTT-4.9.0-2] [MQTT-4.9.0-3] Sender enforces peer Receive Maximum by limiting concurrent QoS>0 in-flight PUBLISH packets.
    if session.on_flight_sent.len()
        >= usize::from(scratchpad.effective_broker_receive_maximum.get())
    {
        return Err(Error::ReceiveMaximumExceeded);
    }
    Ok(())
}

pub(crate) fn validate_outbound_topic_alias<Time: 'static>(
    scratchpad: &ClientScratchpad<Time>,
    topic_alias: Option<NonZero<u16>>,
) -> Result<(), Error> {
    if let Some(alias) = topic_alias {
        let topic_alias_maximum = scratchpad.effective_broker_topic_alias_maximum;
        if topic_alias_maximum == 0 || alias.get() > topic_alias_maximum {
            return Err(Error::ProtocolError);
        }
    }
    Ok(())
}

pub(crate) fn validate_outbound_packet_size<Time: 'static>(
    scratchpad: &ClientScratchpad<Time>,
    packet_size_bytes: usize,
) -> Result<(), Error> {
    if let Some(maximum_packet_size) = scratchpad.effective_broker_maximum_packet_size {
        if packet_size_bytes > maximum_packet_size.get() as usize {
            return Err(Error::PacketTooLarge);
        }
    }
    Ok(())
}

pub(crate) fn validate_outbound_publish_capabilities<Time: 'static>(
    scratchpad: &ClientScratchpad<Time>,
    msg: &ClientMessage,
) -> Result<(), Error> {
    use crate::types::Qos;
    if let Some(maximum_qos) = scratchpad.effective_broker_maximum_qos {
        let exceeds = match maximum_qos {
            MaximumQoS::AtMostOnce => !matches!(msg.qos, Qos::AtMostOnce),
            MaximumQoS::AtLeastOnce => matches!(msg.qos, Qos::ExactlyOnce),
        };
        if exceeds {
            return Err(Error::ProtocolError);
        }
    }
    if msg.retain && !scratchpad.effective_retain_available {
        return Err(Error::ProtocolError);
    }
    Ok(())
}

pub(crate) fn apply_inbound_publish_topic_alias<Time: 'static>(
    session: &mut ClientSession,
    scratchpad: &ClientScratchpad<Time>,
    publish: &mut Publish,
) -> Result<(), Error> {
    let topic: &str = publish.topic.as_ref().as_ref();
    if topic.is_empty() && publish.properties.topic_alias.is_none() {
        return Err(Error::ProtocolError);
    }
    let Some(topic_alias) = publish.properties.topic_alias else {
        return Ok(());
    };
    let topic_alias_maximum = scratchpad.effective_client_topic_alias_maximum;
    if topic_alias.get() > topic_alias_maximum {
        return Err(Error::ProtocolError);
    }
    if topic.is_empty() {
        publish.topic = session
            .inbound_topic_aliases
            .get(&topic_alias)
            .cloned()
            .ok_or(Error::ProtocolError)?;
    } else {
        session
            .inbound_topic_aliases
            .insert(topic_alias, publish.topic.clone());
    }
    Ok(())
}
```

- [ ] **Step 2: Add module declaration to `lib.rs`**

Add `mod limits;` to `src/lib.rs`.

- [ ] **Step 3: Update `proto.rs` call sites**

Replace every method call with the corresponding free function call:

```
self.recompute_effective_limits()
  → limits::recompute_effective_limits(&self.settings, &mut self.scratchpad)

self.reset_negotiated_limits()
  → limits::reset_negotiated_limits(&self.settings, &mut self.session, &mut self.scratchpad)

self.ensure_outbound_receive_maximum_capacity()?
  → limits::ensure_outbound_receive_maximum_capacity(&self.session, &self.scratchpad)?

self.validate_outbound_topic_alias(msg.topic_alias)?
  → limits::validate_outbound_topic_alias(&self.scratchpad, msg.topic_alias)?

self.validate_outbound_packet_size(encoded.len())?
  → limits::validate_outbound_packet_size(&self.scratchpad, encoded.len())?

self.validate_outbound_publish_capabilities(&msg)?
  → limits::validate_outbound_publish_capabilities(&self.scratchpad, &msg)?

self.apply_inbound_publish_topic_alias(&mut publish)
  → limits::apply_inbound_publish_topic_alias(&mut self.session, &self.scratchpad, &mut publish)

Self::min_option_nonzero_u16(a, b)
  → limits::min_option_nonzero_u16(a, b)

Self::min_option_nonzero_u32(a, b)
  → limits::min_option_nonzero_u32(a, b)
```

Delete the moved methods from the `impl<Time> Client<Time>` block in `proto.rs`:
`min_option_nonzero_u16`, `min_option_nonzero_u32`, `recompute_effective_limits`,
`reset_negotiated_limits`, `ensure_outbound_receive_maximum_capacity`,
`validate_outbound_topic_alias`, `validate_outbound_packet_size`,
`validate_outbound_publish_capabilities`, `apply_inbound_publish_topic_alias`.

- [ ] **Step 4: Run tests**

```bash
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/limits.rs \
        crates/sansio-mqtt-v5-protocol/src/proto.rs \
        crates/sansio-mqtt-v5-protocol/src/lib.rs
git commit -m "refactor(v5-protocol): extract limits and validation helpers into limits module"
```

---

### Task 4: Extract `queues.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/queues.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`

`fail_protocol_and_disconnect` inlines the keepalive/session reset bodies during this task to avoid a circular dependency with `session_ops` (not yet extracted). These are cleaned up in Task 12 after `session_ops` exists.

- [ ] **Step 1: Create `src/queues.rs`**

```rust
use alloc::vec::Vec;
use bytes::Bytes;
use core::num::NonZero;
use encode::Encodable;
use sansio_mqtt_v5_types::{
    ControlPacket, Disconnect, DisconnectProperties, DisconnectReasonCode, EncodeError, PubAck,
    PubAckProperties, PubAckReasonCode, PubComp, PubCompProperties, PubCompReasonCode, PubRec,
    PubRecProperties, PubRecReasonCode, PubRel, PubRelProperties, PubRelReasonCode,
};

use crate::proto::{ClientLifecycleState, ClientScratchpad, ClientSession};
use crate::types::{ClientSettings, DriverEventOut, Error};

pub(crate) fn encode_control_packet(packet: &ControlPacket) -> Result<Bytes, Error> {
    let mut encoded = Vec::new();
    packet.encode(&mut encoded).map_err(|err| match err {
        EncodeError::PacketTooLarge(_) => Error::PacketTooLarge,
        _ => Error::EncodeFailure,
    })?;
    Ok(Bytes::from(encoded))
}

pub(crate) fn enqueue_packet<Time: 'static>(
    scratchpad: &mut ClientScratchpad<Time>,
    packet: &ControlPacket,
) -> Result<(), Error> {
    let encoded = encode_control_packet(packet)?;
    crate::limits::validate_outbound_packet_size(scratchpad, encoded.len())?;
    scratchpad.write_queue.push_back(encoded);
    scratchpad.keep_alive_saw_network_activity = true;
    Ok(())
}

/// Enqueues DISCONNECT best-effort, closes socket, transitions lifecycle to Disconnected,
/// and resets keepalive + negotiated limits + session state.
///
/// [MQTT-4.13.1-1] Protocol violations and malformed frames force DISCONNECT and connection close.
///
/// NOTE: During Tasks 4–11 this function still sets scratchpad.lifecycle_state directly.
/// In Task 12 (FSM cutover) that line is removed and callers return ClientState::Disconnected.
pub(crate) fn fail_protocol_and_disconnect<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    reason: DisconnectReasonCode,
) -> Result<(), Error> {
    let _ = enqueue_packet(
        scratchpad,
        &ControlPacket::Disconnect(Disconnect {
            reason_code: reason,
            properties: DisconnectProperties::default(),
        }),
    );
    scratchpad.action_queue.push_back(DriverEventOut::CloseSocket);
    scratchpad.lifecycle_state = ClientLifecycleState::Disconnected; // removed in Task 12
    scratchpad.read_buffer.clear();
    // [MQTT-3.1.2-22] [MQTT-3.1.2-23] reset keepalive — inline until session_ops exists
    scratchpad.keep_alive_interval_secs = None;
    scratchpad.keep_alive_saw_network_activity = false;
    scratchpad.keep_alive_ping_outstanding = false;
    scratchpad.next_timeout = None;
    // reset negotiated limits (also clears inbound topic aliases)
    crate::limits::reset_negotiated_limits(settings, session, scratchpad);
    // [MQTT-3.1.2-4] maybe reset session — inline until session_ops exists
    if !scratchpad.session_should_persist {
        session.on_flight_sent.clear();
        session.on_flight_received.clear();
        session.pending_subscribe.clear();
        session.pending_unsubscribe.clear();
    }
    Ok(())
}

pub(crate) fn enqueue_pubrel_or_fail_protocol<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
) -> Result<(), Error> {
    match enqueue_packet(
        scratchpad,
        &ControlPacket::PubRel(PubRel {
            packet_id,
            reason_code: PubRelReasonCode::Success,
            properties: PubRelProperties::default(),
        }),
    ) {
        Ok(()) => Ok(()),
        Err(_) => {
            fail_protocol_and_disconnect(settings, session, scratchpad, DisconnectReasonCode::ProtocolError)?;
            Err(Error::ProtocolError)
        }
    }
}

pub(crate) fn enqueue_puback_or_fail_protocol<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    reason_code: PubAckReasonCode,
) -> Result<(), Error> {
    match enqueue_packet(
        scratchpad,
        &ControlPacket::PubAck(PubAck {
            packet_id,
            reason_code,
            properties: PubAckProperties::default(),
        }),
    ) {
        Ok(()) => Ok(()),
        Err(_) => {
            fail_protocol_and_disconnect(settings, session, scratchpad, DisconnectReasonCode::ProtocolError)?;
            Err(Error::ProtocolError)
        }
    }
}

pub(crate) fn enqueue_pubrec_or_fail_protocol<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    reason_code: PubRecReasonCode,
) -> Result<(), Error> {
    match enqueue_packet(
        scratchpad,
        &ControlPacket::PubRec(PubRec {
            packet_id,
            reason_code,
            properties: PubRecProperties::default(),
        }),
    ) {
        Ok(()) => Ok(()),
        Err(_) => {
            fail_protocol_and_disconnect(settings, session, scratchpad, DisconnectReasonCode::ProtocolError)?;
            Err(Error::ProtocolError)
        }
    }
}

pub(crate) fn enqueue_pubcomp_or_fail_protocol<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    reason_code: PubCompReasonCode,
) -> Result<(), Error> {
    match enqueue_packet(
        scratchpad,
        &ControlPacket::PubComp(PubComp {
            packet_id,
            reason_code,
            properties: PubCompProperties::default(),
        }),
    ) {
        Ok(()) => Ok(()),
        Err(_) => {
            fail_protocol_and_disconnect(settings, session, scratchpad, DisconnectReasonCode::ProtocolError)?;
            Err(Error::ProtocolError)
        }
    }
}
```

- [ ] **Step 2: Add module declaration to `lib.rs`**

Add `mod queues;` to `src/lib.rs`.

- [ ] **Step 3: Update `proto.rs` call sites**

```
Self::encode_control_packet(&packet)       →  queues::encode_control_packet(&packet)
self.enqueue_packet(packet)                →  queues::enqueue_packet(&mut self.scratchpad, &packet)
self.fail_protocol_and_disconnect(r)       →  queues::fail_protocol_and_disconnect(&self.settings, &mut self.session, &mut self.scratchpad, r)
self.enqueue_pubrel_or_fail_protocol(id)   →  queues::enqueue_pubrel_or_fail_protocol(&self.settings, &mut self.session, &mut self.scratchpad, id)
self.enqueue_puback_or_fail_protocol(id,r) →  queues::enqueue_puback_or_fail_protocol(&self.settings, &mut self.session, &mut self.scratchpad, id, r)
self.enqueue_pubrec_or_fail_protocol(id,r) →  queues::enqueue_pubrec_or_fail_protocol(&self.settings, &mut self.session, &mut self.scratchpad, id, r)
self.enqueue_pubcomp_or_fail_protocol(id,r)→  queues::enqueue_pubcomp_or_fail_protocol(&self.settings, &mut self.session, &mut self.scratchpad, id, r)
```

Delete the moved methods from `proto.rs`.

- [ ] **Step 4: Run tests**

```bash
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/queues.rs \
        crates/sansio-mqtt-v5-protocol/src/proto.rs \
        crates/sansio-mqtt-v5-protocol/src/lib.rs
git commit -m "refactor(v5-protocol): extract queue and encode helpers into queues module"
```

---

### Task 5: Extract `session_ops.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/session_ops.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`

- [ ] **Step 1: Create `src/session_ops.rs`**

```rust
use alloc::collections::btree_map::BTreeMap;
use core::num::NonZero;

use sansio_mqtt_v5_types::{
    ControlPacket, PubRel, PubRelProperties, PubRelReasonCode, PublishKind,
};

use crate::proto::{ClientScratchpad, ClientSession, OutboundInflightState};
use crate::types::{Error, UserWriteOut};

pub(crate) fn reset_keepalive<Time: 'static>(scratchpad: &mut ClientScratchpad<Time>) {
    // [MQTT-3.1.2-22] [MQTT-3.1.2-23] Keep Alive tracking resets on connection lifecycle boundaries.
    scratchpad.keep_alive_interval_secs = None;
    scratchpad.keep_alive_saw_network_activity = false;
    scratchpad.keep_alive_ping_outstanding = false;
    scratchpad.next_timeout = None;
}

pub(crate) fn maybe_reset_session_state<Time: 'static>(
    session: &mut ClientSession,
    scratchpad: &ClientScratchpad<Time>,
) {
    // [MQTT-3.1.2-4] Clean Start controls whether prior session state is discarded.
    if !scratchpad.session_should_persist {
        reset_session_state(session);
    }
}

pub(crate) fn reset_session_state(session: &mut ClientSession) {
    session.on_flight_sent.clear();
    session.on_flight_received.clear();
    session.pending_subscribe.clear();
    session.pending_unsubscribe.clear();
}

pub(crate) fn next_packet_id(session: &mut ClientSession) -> NonZero<u16> {
    let packet_id = session.next_packet_id;
    session.next_packet_id = if packet_id == u16::MAX { 1 } else { packet_id + 1 };
    NonZero::new(packet_id).expect("packet identifier is always non-zero")
}

pub(crate) fn next_packet_id_checked(session: &mut ClientSession) -> Result<NonZero<u16>, Error> {
    // [MQTT-2.2.1-2] Packet Identifier MUST be unused while an exchange is in-flight.
    for _ in 0..u16::MAX {
        let packet_id = next_packet_id(session);
        if !session.on_flight_sent.contains_key(&packet_id)
            && !session.pending_subscribe.contains_key(&packet_id)
            && !session.pending_unsubscribe.contains_key(&packet_id)
        {
            return Ok(packet_id);
        }
    }
    Err(Error::ReceiveMaximumExceeded)
}

pub(crate) fn next_outbound_publish_packet_id(
    session: &mut ClientSession,
) -> Result<NonZero<u16>, Error> {
    for _ in 0..u16::MAX {
        let packet_id = next_packet_id(session);
        if !session.on_flight_sent.contains_key(&packet_id)
            && !session.pending_subscribe.contains_key(&packet_id)
            && !session.pending_unsubscribe.contains_key(&packet_id)
        {
            return Ok(packet_id);
        }
    }
    Err(Error::ReceiveMaximumExceeded)
}

pub(crate) fn emit_publish_dropped_for_all_inflight<Time: 'static>(
    session: &ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    for packet_id in session.on_flight_sent.keys().copied() {
        scratchpad
            .read_queue
            .push_back(UserWriteOut::PublishDroppedDueToSessionNotResumed(packet_id));
    }
}

pub(crate) fn replay_outbound_inflight_with_dup<Time: 'static>(
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) -> Result<(), Error> {
    // [MQTT-4.4.0-1] [MQTT-4.4.0-2] On session resume, retransmit unacknowledged QoS1/QoS2 PUBLISH with DUP=1.
    for (packet_id, state) in session.on_flight_sent.clone() {
        let publish = match state {
            OutboundInflightState::Qos1AwaitPubAck { mut publish }
            | OutboundInflightState::Qos2AwaitPubRec { mut publish } => {
                if let PublishKind::Repetible { dup, .. } = &mut publish.kind {
                    *dup = true;
                }
                publish
            }
            OutboundInflightState::Qos2AwaitPubComp => {
                crate::queues::enqueue_packet(
                    scratchpad,
                    &ControlPacket::PubRel(PubRel {
                        packet_id,
                        reason_code: PubRelReasonCode::Success,
                        properties: PubRelProperties::default(),
                    }),
                )?;
                continue;
            }
        };

        crate::queues::enqueue_packet(scratchpad, &ControlPacket::Publish(publish.clone()))?;

        match session.on_flight_sent.get_mut(&packet_id) {
            Some(
                OutboundInflightState::Qos1AwaitPubAck {
                    publish: stored_publish,
                }
                | OutboundInflightState::Qos2AwaitPubRec {
                    publish: stored_publish,
                },
            ) => {
                *stored_publish = publish;
            }
            _ => {}
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Add module declaration to `lib.rs`**

Add `mod session_ops;` to `src/lib.rs`.

- [ ] **Step 3: Update `proto.rs` call sites**

```
self.reset_keepalive()                          →  session_ops::reset_keepalive(&mut self.scratchpad)
self.maybe_reset_session_state()                →  session_ops::maybe_reset_session_state(&mut self.session, &self.scratchpad)
self.reset_session_state()                      →  session_ops::reset_session_state(&mut self.session)
self.next_packet_id()                           →  session_ops::next_packet_id(&mut self.session)
self.next_packet_id_checked()                   →  session_ops::next_packet_id_checked(&mut self.session)
self.next_outbound_publish_packet_id()          →  session_ops::next_outbound_publish_packet_id(&mut self.session)
self.emit_publish_dropped_for_all_inflight()    →  session_ops::emit_publish_dropped_for_all_inflight(&self.session, &mut self.scratchpad)
self.replay_outbound_inflight_with_dup()        →  session_ops::replay_outbound_inflight_with_dup(&mut self.session, &mut self.scratchpad)
```

Delete the moved methods from `proto.rs`.

Also update `queues::fail_protocol_and_disconnect` in `queues.rs` to call the extracted helpers instead of inlining them:

```rust
// in queues.rs — replace the inline keepalive reset and maybe_reset_session_state bodies with:
crate::session_ops::reset_keepalive(scratchpad);
crate::limits::reset_negotiated_limits(settings, session, scratchpad);
crate::session_ops::maybe_reset_session_state(session, scratchpad);
```

Remove the duplicated inline code from `fail_protocol_and_disconnect`.

- [ ] **Step 4: Run tests**

```bash
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/session_ops.rs \
        crates/sansio-mqtt-v5-protocol/src/queues.rs \
        crates/sansio-mqtt-v5-protocol/src/proto.rs \
        crates/sansio-mqtt-v5-protocol/src/lib.rs
git commit -m "refactor(v5-protocol): extract session operation helpers into session_ops module"
```

---

### Task 6: Extract `scratchpad.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/scratchpad.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/limits.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/queues.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/session_ops.rs`

- [ ] **Step 1: Create `src/scratchpad.rs`**

Cut `ClientLifecycleState`, `ConnectingPhase`, and `ClientScratchpad<Time>` verbatim from `proto.rs` into `scratchpad.rs`. Add the necessary imports (those currently at the top of `proto.rs` that these types depend on). Make `ClientLifecycleState` and `ConnectingPhase` `pub(crate)`.

```rust
// scratchpad.rs — structs and enums cut from proto.rs

use core::num::NonZero;
use alloc::collections::vec_deque::VecDeque;
use bytes::{Bytes, BytesMut};
use sansio_mqtt_v5_types::MaximumQoS;

use crate::types::{ConnectionOptions, DriverEventOut, UserWriteOut};

#[derive(Debug, PartialEq, Default)]
pub(crate) enum ClientLifecycleState {
    #[default]
    Start,
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum ConnectingPhase {
    AwaitConnAck,
    AuthInProgress,
}

#[derive(Debug)]
pub struct ClientScratchpad<Time>
where
    Time: 'static,
{
    // ... all fields verbatim from proto.rs ...
}

impl<Time> Default for ClientScratchpad<Time>
where
    Time: 'static,
{
    // ... verbatim from proto.rs ...
}
```

- [ ] **Step 2: Update imports in `limits.rs`, `queues.rs`, `session_ops.rs`**

In each of these files, replace:
```rust
use crate::proto::ClientScratchpad;
```
with:
```rust
use crate::scratchpad::ClientScratchpad;
```

Also in `queues.rs`, replace:
```rust
use crate::proto::ClientLifecycleState;
```
with:
```rust
use crate::scratchpad::ClientLifecycleState;
```

- [ ] **Step 3: Add module declaration to `lib.rs`**

Add `mod scratchpad;` and update any re-exports if `ClientScratchpad` is public-facing.

- [ ] **Step 4: Update `proto.rs`**

Remove the `ClientLifecycleState`, `ConnectingPhase`, and `ClientScratchpad<Time>` definitions from `proto.rs`. Add `use crate::scratchpad::{ClientLifecycleState, ConnectingPhase, ClientScratchpad};` at the top of `proto.rs`.

- [ ] **Step 5: Run tests**

```bash
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/scratchpad.rs \
        crates/sansio-mqtt-v5-protocol/src/proto.rs \
        crates/sansio-mqtt-v5-protocol/src/lib.rs \
        crates/sansio-mqtt-v5-protocol/src/limits.rs \
        crates/sansio-mqtt-v5-protocol/src/queues.rs \
        crates/sansio-mqtt-v5-protocol/src/session_ops.rs
git commit -m "refactor(v5-protocol): extract ClientScratchpad into scratchpad module"
```

---

### Task 7: Extract `session.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/session.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/limits.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/queues.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/session_ops.rs`

- [ ] **Step 1: Create `src/session.rs`**

Cut `OutboundInflightState`, `InboundInflightState`, `ClientSession`, and its impls verbatim from `proto.rs` into `session.rs`:

```rust
// session.rs
use alloc::collections::btree_map::BTreeMap;
use core::num::NonZero;

use sansio_mqtt_v5_types::{Publish, PubRecReasonCode, Topic};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OutboundInflightState {
    Qos1AwaitPubAck { publish: Publish },
    Qos2AwaitPubRec { publish: Publish },
    Qos2AwaitPubComp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InboundInflightState {
    Qos1AwaitAppDecision,
    Qos2AwaitAppDecision,
    Qos2AwaitPubRel,
    Qos2Rejected(PubRecReasonCode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientSession {
    pub(crate) on_flight_sent: BTreeMap<NonZero<u16>, OutboundInflightState>,
    pub(crate) on_flight_received: BTreeMap<NonZero<u16>, InboundInflightState>,
    pub(crate) pending_subscribe: BTreeMap<NonZero<u16>, ()>,
    pub(crate) pending_unsubscribe: BTreeMap<NonZero<u16>, ()>,
    pub(crate) inbound_topic_aliases: BTreeMap<NonZero<u16>, Topic>,
    pub(crate) next_packet_id: u16,
}

impl ClientSession {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

impl Default for ClientSession {
    fn default() -> Self {
        Self {
            on_flight_sent: BTreeMap::new(),
            on_flight_received: BTreeMap::new(),
            pending_subscribe: BTreeMap::new(),
            pending_unsubscribe: BTreeMap::new(),
            inbound_topic_aliases: BTreeMap::new(),
            next_packet_id: 1,
        }
    }
}
```

- [ ] **Step 2: Update imports in all modules**

In `limits.rs`, `queues.rs`, `session_ops.rs`, replace:
```rust
use crate::proto::ClientSession;
```
with:
```rust
use crate::session::ClientSession;
```

Also in `session_ops.rs`, replace:
```rust
use crate::proto::OutboundInflightState;
```
with:
```rust
use crate::session::OutboundInflightState;
```

- [ ] **Step 3: Update `lib.rs`**

Add `mod session;` and change `pub use proto::ClientSession;` to `pub use session::ClientSession;`.

- [ ] **Step 4: Update `proto.rs`**

Remove `OutboundInflightState`, `InboundInflightState`, `ClientSession` and their impls.
Add at the top:
```rust
use crate::session::{ClientSession, InboundInflightState, OutboundInflightState};
```

- [ ] **Step 5: Run tests**

```bash
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/session.rs \
        crates/sansio-mqtt-v5-protocol/src/proto.rs \
        crates/sansio-mqtt-v5-protocol/src/lib.rs \
        crates/sansio-mqtt-v5-protocol/src/limits.rs \
        crates/sansio-mqtt-v5-protocol/src/queues.rs \
        crates/sansio-mqtt-v5-protocol/src/session_ops.rs
git commit -m "refactor(v5-protocol): extract ClientSession and inflight enums into session module"
```

---

### Task 8: Create `state/mod.rs` — FSM enum and trait

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/state/mod.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`

At this point `proto.rs` is still the live dispatcher. The `state/` module is built alongside it. The trait impls (Tasks 9–11) are not wired up to `Client` until Task 12.

- [ ] **Step 1: Create `src/state/mod.rs`**

```rust
pub(crate) mod connected;
pub(crate) mod connecting;
pub(crate) mod disconnected;
pub(crate) mod start;

pub(crate) use connected::Connected;
pub(crate) use connecting::Connecting;
pub(crate) use disconnected::Disconnected;
pub(crate) use start::Start;

use sansio_mqtt_v5_types::ControlPacket;

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::types::{ClientSettings, DriverEventIn, Error, UserWriteIn};

/// The MQTT client lifecycle as a type-state FSM.
///
/// `Transitioning` is a zero-size default used as a `core::mem::take` sentinel.
/// It is never observable in stable code — the `unreachable!` in its trait impl
/// fires only if a bug leaves the FSM without a next state after `dispatch`.
#[derive(Default)]
pub(crate) enum ClientState {
    #[default]
    Transitioning,
    Start(Start),
    Disconnected(Disconnected),
    Connecting(Connecting),
    Connected(Connected),
}

pub(crate) trait StateHandler<Time>: Sized {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>);

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>);

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>);

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        now: Time,
    ) -> (ClientState, Result<(), Error>);

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>);
}

impl<Time: Copy + Ord + 'static> StateHandler<Time> for ClientState {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.handle_control_packet(settings, session, scratchpad, packet),
            ClientState::Disconnected(x) => x.handle_control_packet(settings, session, scratchpad, packet),
            ClientState::Connecting(x) => x.handle_control_packet(settings, session, scratchpad, packet),
            ClientState::Connected(x) => x.handle_control_packet(settings, session, scratchpad, packet),
        }
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.handle_write(settings, session, scratchpad, msg),
            ClientState::Disconnected(x) => x.handle_write(settings, session, scratchpad, msg),
            ClientState::Connecting(x) => x.handle_write(settings, session, scratchpad, msg),
            ClientState::Connected(x) => x.handle_write(settings, session, scratchpad, msg),
        }
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.handle_event(settings, session, scratchpad, evt),
            ClientState::Disconnected(x) => x.handle_event(settings, session, scratchpad, evt),
            ClientState::Connecting(x) => x.handle_event(settings, session, scratchpad, evt),
            ClientState::Connected(x) => x.handle_event(settings, session, scratchpad, evt),
        }
    }

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        now: Time,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.handle_timeout(settings, session, scratchpad, now),
            ClientState::Disconnected(x) => x.handle_timeout(settings, session, scratchpad, now),
            ClientState::Connecting(x) => x.handle_timeout(settings, session, scratchpad, now),
            ClientState::Connected(x) => x.handle_timeout(settings, session, scratchpad, now),
        }
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.close(settings, session, scratchpad),
            ClientState::Disconnected(x) => x.close(settings, session, scratchpad),
            ClientState::Connecting(x) => x.close(settings, session, scratchpad),
            ClientState::Connected(x) => x.close(settings, session, scratchpad),
        }
    }
}
```

- [ ] **Step 2: Add module declaration to `lib.rs`**

Add `mod state;` to `src/lib.rs`. Do NOT re-export `ClientState` yet — it is `pub(crate)` and only used internally until Task 12.

- [ ] **Step 3: Run tests**

```bash
cargo test --workspace
```
Expected: PASS (new module compiles but is unreachable; no behaviour change).

- [ ] **Step 4: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/state/ \
        crates/sansio-mqtt-v5-protocol/src/lib.rs
git commit -m "refactor(v5-protocol): introduce ClientState FSM enum and StateHandler trait"
```

---

### Task 9: Implement `state/start.rs` + `state/disconnected.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/state/start.rs`
- Create: `crates/sansio-mqtt-v5-protocol/src/state/disconnected.rs`

These two states share nearly identical behaviour — both accept `Connect` and reject everything else.

- [ ] **Step 1: Create `src/state/start.rs`**

```rust
use sansio_mqtt_v5_types::ControlPacket;

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::{ClientState, Disconnected, StateHandler};
use crate::types::{ClientSettings, ConnectionOptions, DriverEventIn, DriverEventOut, Error, UserWriteIn};
use crate::{limits, session_ops};

pub(crate) struct Start;

fn handle_connect<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    options: ConnectionOptions,
) -> (ClientState, Result<(), Error>) {
    scratchpad.pending_connect_options = options;
    limits::recompute_effective_limits(settings, scratchpad);
    if scratchpad.pending_connect_options.clean_start {
        // [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
        session.clear();
    }
    scratchpad.session_should_persist = scratchpad
        .pending_connect_options
        .session_expiry_interval
        .unwrap_or(0)
        > 0;
    if !scratchpad
        .action_queue
        .iter()
        .any(|e| matches!(e, DriverEventOut::OpenSocket))
    {
        scratchpad.action_queue.push_back(DriverEventOut::OpenSocket);
    }
    use crate::state::Connecting;
    (
        ClientState::Connecting(Connecting {
            pending_connect_options: core::mem::take(&mut scratchpad.pending_connect_options),
        }),
        Ok(()),
    )
}

impl<Time: Copy + Ord + 'static> StateHandler<Time> for Start {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        _packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        let _ = crate::queues::fail_protocol_and_disconnect(
            settings, session, scratchpad,
            sansio_mqtt_v5_types::DisconnectReasonCode::ProtocolError,
        );
        (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError))
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            UserWriteIn::Connect(options) => {
                handle_connect(settings, session, scratchpad, options)
            }
            _ => (ClientState::Start(self), Err(Error::InvalidStateTransition)),
        }
    }

    fn handle_event(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Start(self), Err(Error::InvalidStateTransition))
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _now: Time,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Start(self), Ok(()))
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        session_ops::reset_keepalive(scratchpad);
        limits::reset_negotiated_limits(settings, session, scratchpad);
        session_ops::maybe_reset_session_state(session, scratchpad);
        (ClientState::Disconnected(Disconnected), Ok(()))
    }
}
```

- [ ] **Step 2: Create `src/state/disconnected.rs`**

`Disconnected` is identical to `Start` except `close()` is idempotent and `handle_event` also errors:

```rust
use sansio_mqtt_v5_types::ControlPacket;

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::{ClientState, StateHandler};
use crate::types::{ClientSettings, DriverEventIn, Error, UserWriteIn};

pub(crate) struct Disconnected;

impl<Time: Copy + Ord + 'static> StateHandler<Time> for Disconnected {
    fn handle_control_packet(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        // Packet arrival while disconnected is a protocol error but we are already
        // disconnected — no further action needed.
        (ClientState::Disconnected(self), Err(Error::ProtocolError))
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            UserWriteIn::Connect(options) => {
                crate::state::start::handle_connect(settings, session, scratchpad, options)
            }
            _ => (ClientState::Disconnected(self), Err(Error::InvalidStateTransition)),
        }
    }

    fn handle_event(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Disconnected(self), Err(Error::InvalidStateTransition))
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _now: Time,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Disconnected(self), Ok(()))
    }

    fn close(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Disconnected(self), Ok(()))
    }
}
```

Make `handle_connect` in `start.rs` `pub(crate)` so `disconnected.rs` can call it.

- [ ] **Step 3: Run tests**

```bash
cargo test --workspace
```
Expected: PASS (still unreachable; proto.rs is still live).

- [ ] **Step 4: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/state/start.rs \
        crates/sansio-mqtt-v5-protocol/src/state/disconnected.rs \
        crates/sansio-mqtt-v5-protocol/src/state/mod.rs
git commit -m "refactor(v5-protocol): implement StateHandler for Start and Disconnected states"
```

---

### Task 10: Implement `state/connecting.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/state/connecting.rs`

The CONNECT packet is built inline in `handle_event(SocketConnected)`. The three `build_connect_packet` unit tests in `proto.rs` become dead after the cutover in Task 12 — delete them then.

- [ ] **Step 1: Create `src/state/connecting.rs`**

```rust
use core::num::NonZero;

use bytes::Bytes;
use sansio_mqtt_v5_types::{
    AuthReasonCode, BinaryData, ConnAckKind, ConnackReasonCode, Connect, ConnectProperties,
    ControlPacket, Disconnect, DisconnectProperties, DisconnectReasonCode, Utf8String, Will as ConnectWill,
    WillProperties,
};

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::session_ops;
use crate::state::{ClientState, Connected, Disconnected, StateHandler};
use crate::types::{
    ClientSettings, ConnectionOptions, DriverEventIn, DriverEventOut, Error, UserWriteIn,
    UserWriteOut,
};
use crate::{limits, queues, session_ops as sops};

pub(crate) struct Connecting {
    pub(crate) pending_connect_options: ConnectionOptions,
}

fn build_connect(
    settings: &ClientSettings,
    options: &ConnectionOptions,
) -> Result<Connect, Error> {
    let will = options
        .will
        .as_ref()
        .map(|will| {
            let payload =
                BinaryData::try_new(will.payload.clone()).map_err(|_| Error::ProtocolError)?;
            let message_expiry_interval = will
                .message_expiry_interval
                .map(|interval| {
                    u32::try_from(interval.as_secs()).map_err(|_| Error::ProtocolError)
                })
                .transpose()?;
            Ok(ConnectWill {
                topic: will.topic.clone(),
                payload,
                qos: will.qos,
                retain: will.retain,
                properties: WillProperties {
                    will_delay_interval: will.will_delay_interval,
                    payload_format_indicator: will.payload_format_indicator,
                    message_expiry_interval,
                    content_type: will.content_type.clone(),
                    response_topic: will.response_topic.clone(),
                    correlation_data: will.correlation_data.clone(),
                    user_properties: will.user_properties.clone(),
                },
            })
        })
        .transpose()?;

    Ok(Connect {
        protocol_name: Utf8String::try_from("MQTT")
            .expect("MQTT protocol name is always valid UTF-8 string"),
        protocol_version: 5,
        clean_start: options.clean_start,
        client_identifier: options.client_identifier.clone(),
        will,
        user_name: options.user_name.clone(),
        password: options.password.clone(),
        keep_alive: options.keep_alive.or(settings.default_keep_alive),
        properties: ConnectProperties {
            session_expiry_interval: options.session_expiry_interval,
            receive_maximum: limits::min_option_nonzero_u16(
                options.receive_maximum,
                settings.max_incoming_receive_maximum,
            ),
            maximum_packet_size: limits::min_option_nonzero_u32(
                options.maximum_packet_size,
                settings.max_incoming_packet_size,
            ),
            topic_alias_maximum: options
                .topic_alias_maximum
                .or(settings.max_incoming_topic_alias_maximum)
                .map(|v| v.min(settings.max_incoming_topic_alias_maximum.unwrap_or(u16::MAX))),
            request_response_information: options
                .request_response_information
                .or(settings.default_request_response_information),
            request_problem_information: options
                .request_problem_information
                .or(settings.default_request_problem_information),
            authentication: options.authentication.clone(),
            user_properties: options.user_properties.clone(),
        },
    })
}

fn on_socket_connected<Time: 'static>(
    connecting: Connecting,
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) -> (ClientState, Result<(), Error>) {
    limits::reset_negotiated_limits(settings, session, scratchpad);
    let connect = match build_connect(settings, &connecting.pending_connect_options) {
        Ok(c) => c,
        Err(e) => return (ClientState::Connecting(connecting), Err(e)),
    };
    match queues::enqueue_packet(scratchpad, &ControlPacket::Connect(connect)) {
        Ok(()) => {
            scratchpad.keep_alive_saw_network_activity = false;
            scratchpad.keep_alive_ping_outstanding = false;
            (ClientState::Connecting(connecting), Ok(()))
        }
        Err(e) => (ClientState::Connecting(connecting), Err(e)),
    }
}

fn on_socket_closed_or_error<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    is_error: bool,
) -> (ClientState, Result<(), Error>) {
    scratchpad.read_buffer.clear();
    sops::reset_keepalive(scratchpad);
    limits::reset_negotiated_limits(settings, session, scratchpad);
    sops::maybe_reset_session_state(session, scratchpad);
    scratchpad.read_queue.push_back(UserWriteOut::Disconnected);
    if is_error {
        scratchpad.action_queue.push_back(DriverEventOut::CloseSocket);
        return (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError));
    }
    (ClientState::Disconnected(Disconnected), Ok(()))
}

fn on_connack_success<Time: 'static>(
    connecting: Connecting,
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    connack: sansio_mqtt_v5_types::ConnAck,
) -> (ClientState, Result<(), Error>) {
    // Populate negotiated server capabilities
    scratchpad.negotiated_receive_maximum = connack
        .properties
        .receive_maximum
        .unwrap_or(NonZero::new(u16::MAX).expect("u16::MAX is always non-zero"));
    scratchpad.negotiated_maximum_packet_size = connack.properties.maximum_packet_size;
    scratchpad.negotiated_topic_alias_maximum =
        connack.properties.topic_alias_maximum.unwrap_or(0);
    scratchpad.negotiated_server_keep_alive = connack.properties.server_keep_alive;
    scratchpad.negotiated_maximum_qos = connack.properties.maximum_qos;
    scratchpad.negotiated_retain_available =
        connack.properties.retain_available.unwrap_or(true);
    scratchpad.negotiated_wildcard_subscription_available = connack
        .properties
        .wildcard_subscription_available
        .unwrap_or(true);
    scratchpad.negotiated_subscription_identifiers_available = connack
        .properties
        .subscription_identifiers_available
        .unwrap_or(true);
    scratchpad.negotiated_shared_subscription_available = connack
        .properties
        .shared_subscription_available
        .unwrap_or(true);
    limits::recompute_effective_limits(settings, scratchpad);
    scratchpad.keep_alive_interval_secs =
        match scratchpad.negotiated_server_keep_alive {
            Some(server_keep_alive) => NonZero::new(server_keep_alive),
            None => connecting.pending_connect_options.keep_alive,
        };
    scratchpad.keep_alive_saw_network_activity = false;
    scratchpad.keep_alive_ping_outstanding = false;

    match connack.kind {
        ConnAckKind::ResumePreviousSession => {
            // [MQTT-3.2.2-2] Session Present=1 is only valid when CONNECT had Clean Start=0.
            if connecting.pending_connect_options.clean_start {
                let _ = queues::fail_protocol_and_disconnect(
                    settings, session, scratchpad,
                    DisconnectReasonCode::ProtocolError,
                );
                return (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError));
            }
            // [MQTT-4.4.0-1] [MQTT-4.4.0-2] Session Present=1 resumes in-flight QoS transactions.
            if session_ops::replay_outbound_inflight_with_dup(session, scratchpad).is_err() {
                let _ = queues::fail_protocol_and_disconnect(
                    settings, session, scratchpad,
                    DisconnectReasonCode::ProtocolError,
                );
                return (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError));
            }
            scratchpad.read_queue.push_back(UserWriteOut::Connected);
        }
        ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        } => {
            scratchpad.read_queue.push_back(UserWriteOut::Connected);
            session_ops::emit_publish_dropped_for_all_inflight(session, scratchpad);
            session_ops::reset_session_state(session);
        }
        _ => unreachable!("successful CONNACK kind already matched"),
    }

    (ClientState::Connected(Connected), Ok(()))
}

impl<Time: Copy + Ord + 'static> StateHandler<Time> for Connecting {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        match packet {
            ControlPacket::ConnAck(connack) => {
                let is_success = matches!(
                    connack.kind,
                    ConnAckKind::ResumePreviousSession
                        | ConnAckKind::Other {
                            reason_code: ConnackReasonCode::Success
                        }
                );
                if is_success {
                    on_connack_success(self, settings, session, scratchpad, connack)
                } else {
                    scratchpad.action_queue.push_back(DriverEventOut::CloseSocket);
                    limits::reset_negotiated_limits(settings, session, scratchpad);
                    (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError))
                }
            }
            ControlPacket::Auth(auth) => {
                if self.pending_connect_options.authentication.is_none() {
                    let _ = queues::fail_protocol_and_disconnect(
                        settings, session, scratchpad,
                        DisconnectReasonCode::ProtocolError,
                    );
                    return (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError));
                }
                if !matches!(auth.reason_code, AuthReasonCode::ContinueAuthentication) {
                    let _ = queues::fail_protocol_and_disconnect(
                        settings, session, scratchpad,
                        DisconnectReasonCode::ProtocolError,
                    );
                    return (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError));
                }
                // Remain Connecting; ConnectingPhase tracking removed (was write-only dead state).
                (ClientState::Connecting(self), Ok(()))
            }
            _ => {
                let _ = queues::fail_protocol_and_disconnect(
                    settings, session, scratchpad,
                    DisconnectReasonCode::ProtocolError,
                );
                (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError))
            }
        }
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            UserWriteIn::Disconnect => {
                let _ = queues::enqueue_packet(
                    scratchpad,
                    &ControlPacket::Disconnect(Disconnect {
                        reason_code: DisconnectReasonCode::NormalDisconnection,
                        properties: DisconnectProperties::default(),
                    }),
                );
                scratchpad.action_queue.push_back(DriverEventOut::CloseSocket);
                scratchpad.read_queue.push_back(UserWriteOut::Disconnected);
                sops::reset_keepalive(scratchpad);
                limits::reset_negotiated_limits(settings, session, scratchpad);
                sops::maybe_reset_session_state(session, scratchpad);
                (ClientState::Disconnected(Disconnected), Ok(()))
            }
            _ => (ClientState::Connecting(self), Err(Error::InvalidStateTransition)),
        }
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        match evt {
            DriverEventIn::SocketConnected => {
                on_socket_connected(self, settings, session, scratchpad)
            }
            DriverEventIn::SocketClosed => {
                on_socket_closed_or_error(settings, session, scratchpad, false)
            }
            DriverEventIn::SocketError => {
                on_socket_closed_or_error(settings, session, scratchpad, true)
            }
        }
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _now: Time,
    ) -> (ClientState, Result<(), Error>) {
        // Keep-alive timer only runs while Connected.
        (ClientState::Connecting(self), Ok(()))
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        let _ = queues::enqueue_packet(
            scratchpad,
            &ControlPacket::Disconnect(Disconnect {
                reason_code: DisconnectReasonCode::NormalDisconnection,
                properties: DisconnectProperties::default(),
            }),
        );
        scratchpad.action_queue.push_back(DriverEventOut::CloseSocket);
        scratchpad.read_buffer.clear();
        sops::reset_keepalive(scratchpad);
        limits::reset_negotiated_limits(settings, session, scratchpad);
        sops::maybe_reset_session_state(session, scratchpad);
        scratchpad.read_queue.push_back(UserWriteOut::Disconnected);
        (ClientState::Disconnected(Disconnected), Ok(()))
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/state/connecting.rs \
        crates/sansio-mqtt-v5-protocol/src/state/mod.rs
git commit -m "refactor(v5-protocol): implement StateHandler for Connecting state"
```

---

### Task 11: Implement `state/connected.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/state/connected.rs`

This is the largest state handler. It mirrors the `ClientLifecycleState::Connected` arm of `handle_read_control_packet` (lines 879–1121 of `proto.rs`) and the `Connected`-guarded arms of `handle_write`, `handle_event`, `handle_timeout`, and `close`.

- [ ] **Step 1: Create `src/state/connected.rs`**

Start with the helper functions cut from `proto.rs` — `map_inbound_publish_to_broker_message` and `map_incoming_reject_reason_*` — placed as free functions in this module:

```rust
use core::time::Duration;
use sansio_mqtt_v5_types::{
    IncomingRejectReason (if exists) // check types.rs for exact name
    ...
};
use crate::types::{BrokerMessage, IncomingRejectReason, ...};
```

Then implement `handle_control_packet` by copying the `ClientLifecycleState::Connected` match arm from `proto.rs` (lines 879–1121), replacing `self.session.` with `session.`, `self.scratchpad.` with `scratchpad.`, and every helper call with its free-function equivalent. Every `[MQTT-x.x.x-y]` citation must be preserved in place.

The disconnect path returns `(ClientState::Disconnected(Disconnected), Ok(()))` or `Err(ProtocolError)` as applicable. Staying in the same state returns `(ClientState::Connected(self), result)`.

For `handle_write`, copy the match arms from `handle_write` in `proto.rs` that are guarded by `lifecycle_state == Connected`. Add a `Connect(_)` arm that returns `Err(InvalidStateTransition)`.

For `handle_event`:
```rust
fn handle_event(self, settings, session, scratchpad, evt) {
    match evt {
        DriverEventIn::SocketConnected =>
            (ClientState::Connected(self), Err(Error::InvalidStateTransition)),
        DriverEventIn::SocketClosed => { /* reset + emit Disconnected, Ok(()) */ }
        DriverEventIn::SocketError  => { /* reset + CloseSocket, Err(ProtocolError) */ }
    }
}
```

For `handle_timeout`, copy the body from `handle_timeout` in the Protocol impl (lines 1556–1585), replacing `self.scratchpad.` → `scratchpad.` and helper calls with free-function equivalents:
```rust
fn handle_timeout(self, settings, session, scratchpad, now) {
    if scratchpad.keep_alive_interval_secs.is_none() {
        scratchpad.next_timeout = None;
        return (ClientState::Connected(self), Ok(()));
    }
    if scratchpad.keep_alive_ping_outstanding {
        // [MQTT-3.1.2-24] [MQTT-4.13.1-1]
        let _ = queues::fail_protocol_and_disconnect(settings, session, scratchpad,
            DisconnectReasonCode::KeepAliveTimeout);
        return (ClientState::Disconnected(Disconnected), Err(Error::ProtocolError));
    }
    if scratchpad.keep_alive_saw_network_activity {
        // [MQTT-3.1.2-22]
        scratchpad.keep_alive_saw_network_activity = false;
        scratchpad.next_timeout = Some(now);
        return (ClientState::Connected(self), Ok(()));
    }
    // [MQTT-3.1.2-22] [MQTT-3.12.4-1]
    let _ = queues::enqueue_packet(scratchpad, &ControlPacket::PingReq(PingReq {}));
    scratchpad.keep_alive_ping_outstanding = true;
    scratchpad.next_timeout = Some(now);
    (ClientState::Connected(self), Ok(()))
}
```

For `close`, copy the `ClientLifecycleState::Connected | Connecting` arm from `close()` in the Protocol impl (lines 1590–1606).

- [ ] **Step 2: Run tests**

```bash
cargo test --workspace
```
Expected: PASS (still unreachable code; proto.rs handles all requests).

- [ ] **Step 3: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/state/connected.rs \
        crates/sansio-mqtt-v5-protocol/src/state/mod.rs
git commit -m "refactor(v5-protocol): implement StateHandler for Connected state"
```

---

### Task 12: Wire `Client` dispatch — the FSM cutover

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/scratchpad.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/queues.rs`

This is the big switchover. After this commit proto.rs is a thin shell, and state/*.rs runs the show.

- [ ] **Step 1: Remove `lifecycle_state` and `connecting_phase` from `ClientScratchpad`**

In `scratchpad.rs`:
1. Remove `lifecycle_state: ClientLifecycleState` and `connecting_phase: ConnectingPhase` fields from `ClientScratchpad<Time>`.
2. Remove their `Default` values.
3. `ClientLifecycleState` and `ConnectingPhase` enums are now unused — delete them from `scratchpad.rs`.

- [ ] **Step 2: Add `state: ClientState` to `Client<Time>` in `proto.rs`**

```rust
use crate::state::{ClientState, Start, StateHandler};

pub struct Client<Time>
where Time: 'static,
{
    settings:   ClientSettings,
    session:    ClientSession,
    scratchpad: ClientScratchpad<Time>,
    state:      ClientState,
}

impl<Time> Default for Client<Time> {
    fn default() -> Self { Self::with_settings(Default::default()) }
}

impl<Time> Client<Time> {
    pub fn with_settings(settings: ClientSettings) -> Self {
        Self::with_settings_and_session(settings, Default::default())
    }

    pub fn with_settings_and_session(settings: ClientSettings, session: ClientSession) -> Self {
        let mut client = Self {
            settings,
            session,
            scratchpad: ClientScratchpad::default(),
            state: ClientState::Start(Start),
        };
        limits::recompute_effective_limits(&client.settings, &mut client.scratchpad);
        client
    }

    #[inline(always)]
    fn dispatch<F>(&mut self, f: F) -> Result<(), Error>
    where
        F: FnOnce(
            ClientState,
            &ClientSettings,
            &mut ClientSession,
            &mut ClientScratchpad<Time>,
        ) -> (ClientState, Result<(), Error>),
    {
        let state = core::mem::take(&mut self.state);
        let (next, result) = f(state, &self.settings, &mut self.session, &mut self.scratchpad);
        self.state = next;
        result
    }
}
```

- [ ] **Step 3: Replace Protocol impl methods in `proto.rs`**

Replace the full bodies of `handle_read`, `handle_write`, `handle_event`, `handle_timeout`, `close`, and the poll methods with thin delegates:

```rust
impl<Time: Copy + Ord + 'static> Protocol<Bytes, UserWriteIn, DriverEventIn> for Client<Time> {
    type Rout = UserWriteOut;
    type Wout = Bytes;
    type Eout = DriverEventOut;
    type Error = Error;
    type Time = Time;

    #[tracing::instrument(skip_all)]
    fn handle_read(&mut self, msg: Bytes) -> Result<(), Self::Error> {
        let packet_bytes = if self.scratchpad.read_buffer.is_empty() {
            msg
        } else {
            let mut combined = core::mem::take(&mut self.scratchpad.read_buffer);
            combined.extend_from_slice(&msg);
            combined.freeze()
        };

        let parser_settings = crate::types::ParserSettings {
            max_bytes_string: self.scratchpad.effective_client_max_bytes_string,
            max_bytes_binary_data: self.scratchpad.effective_client_max_bytes_binary_data,
            max_remaining_bytes: self.scratchpad.effective_client_max_remaining_bytes,
            max_subscriptions_len: self.scratchpad.effective_client_max_subscriptions_len,
            max_user_properties_len: self.scratchpad.effective_client_max_user_properties_len,
            max_subscription_identifiers_len: self
                .scratchpad
                .effective_client_max_subscription_identifiers_len,
        };
        let mut slice: &[u8] = packet_bytes.as_ref();

        while !slice.is_empty() {
            let mut input = Partial::new(slice);
            match ControlPacket::parser::<_, ErrMode<()>, ErrMode<()>>(&parser_settings)
                .parse_next(&mut input)
            {
                Ok(packet) => {
                    slice = input.into_inner();
                    self.scratchpad.keep_alive_saw_network_activity = true;
                    if matches!(packet, ControlPacket::PingResp(_)) {
                        self.scratchpad.keep_alive_ping_outstanding = false;
                    }
                    self.dispatch(|s, set, ses, sp| s.handle_control_packet(set, ses, sp, packet))?;
                }
                Err(ErrMode::Incomplete(_)) => break,
                Err(ErrMode::Backtrack(_)) | Err(ErrMode::Cut(_)) => {
                    // [MQTT-4.13.1-1] Malformed Control Packet is a protocol error.
                    self.dispatch(|s, set, ses, sp| {
                        let _ = queues::fail_protocol_and_disconnect(
                            set, ses, sp,
                            DisconnectReasonCode::MalformedPacket,
                        );
                        (ClientState::Disconnected(crate::state::Disconnected), Err(Error::MalformedPacket))
                    })?;
                    return Err(Error::MalformedPacket);
                }
            }
        }
        self.scratchpad.read_buffer = BytesMut::from(slice);
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn handle_write(&mut self, msg: UserWriteIn) -> Result<(), Self::Error> {
        self.dispatch(|s, set, ses, sp| s.handle_write(set, ses, sp, msg))
    }

    #[tracing::instrument(skip_all)]
    fn handle_event(&mut self, evt: DriverEventIn) -> Result<(), Self::Error> {
        self.dispatch(|s, set, ses, sp| s.handle_event(set, ses, sp, evt))
    }

    #[tracing::instrument(skip_all)]
    fn handle_timeout(&mut self, now: Self::Time) -> Result<(), Self::Error> {
        self.dispatch(|s, set, ses, sp| s.handle_timeout(set, ses, sp, now))
    }

    #[tracing::instrument(skip_all)]
    fn close(&mut self) -> Result<(), Self::Error> {
        self.dispatch(|s, set, ses, sp| s.close(set, ses, sp))
    }

    fn poll_read(&mut self) -> Option<Self::Rout> { self.scratchpad.read_queue.pop_front() }
    fn poll_write(&mut self) -> Option<Self::Wout> { self.scratchpad.write_queue.pop_front() }
    fn poll_event(&mut self) -> Option<Self::Eout> { self.scratchpad.action_queue.pop_front() }
    fn poll_timeout(&mut self) -> Option<Self::Time> { self.scratchpad.next_timeout }
}
```

- [ ] **Step 4: Update `queues::fail_protocol_and_disconnect`**

Remove the `scratchpad.lifecycle_state = ...` line (the field no longer exists). Confirm it now calls the session_ops helpers (already done in Task 5):

```rust
pub(crate) fn fail_protocol_and_disconnect<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    reason: DisconnectReasonCode,
) -> Result<(), Error> {
    // [MQTT-4.13.1-1]
    let _ = enqueue_packet(
        scratchpad,
        &ControlPacket::Disconnect(Disconnect {
            reason_code: reason,
            properties: DisconnectProperties::default(),
        }),
    );
    scratchpad.action_queue.push_back(DriverEventOut::CloseSocket);
    scratchpad.read_buffer.clear();
    crate::session_ops::reset_keepalive(scratchpad);
    crate::limits::reset_negotiated_limits(settings, session, scratchpad);
    crate::session_ops::maybe_reset_session_state(session, scratchpad);
    Ok(())
}
```

- [ ] **Step 5: Delete dead code from `proto.rs`**

Remove `handle_read_control_packet`, `handle_write` (the old full impl), `handle_event` (old impl), `handle_timeout` (old impl), `close` (old impl), `map_inbound_publish_to_broker_message`, `map_incoming_reject_reason_to_puback`, `map_incoming_reject_reason_to_pubrec`, `parser_settings` (inline it in `handle_read` above), and the three `build_connect_packet` unit tests.

Keep the two remaining unit tests (`socket_connected_error_does_not_poison_state`, `pubrel_enqueue_failure_forces_protocol_close`) — move them to `#[cfg(test)] mod tests` inside `state/connected.rs`.

- [ ] **Step 6: Run full test suite**

```bash
cargo test --workspace
```
Expected: PASS. This is the proof that the FSM mirrors the old behaviour exactly. If any test fails, diagnose using the transition map in the design spec before continuing.

- [ ] **Step 7: Run lints**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```
Fix any warnings before committing.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(v5-protocol): cut over Client to dispatch-based FSM, remove legacy proto.rs impl"
```

---

### Task 13: Extract `client.rs` + delete `proto.rs`

**Files:**
- Create: `crates/sansio-mqtt-v5-protocol/src/client.rs`
- Delete: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`

At this point `proto.rs` contains only `Client<Time>`, its constructors, `dispatch`, and the thin Protocol impl. Moving it to `client.rs` is mechanical.

- [ ] **Step 1: Create `src/client.rs`**

Cut everything remaining in `proto.rs` into `client.rs`. Update imports — the `use crate::proto::*` entries in other files become `use crate::client::*`.

- [ ] **Step 2: Delete `proto.rs`**

```bash
rm crates/sansio-mqtt-v5-protocol/src/proto.rs
```

- [ ] **Step 3: Update `lib.rs`**

Replace `mod proto;` with `mod client;`. Update re-exports:

```rust
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

mod client;
mod limits;
mod queues;
mod scratchpad;
mod session;
mod session_ops;
mod state;
mod types;

pub use client::Client;
pub use session::ClientSession;
pub use types::*;
```

- [ ] **Step 4: Run full test suite**

```bash
cargo test --workspace
cargo build -p sansio-mqtt-v5-tokio
```
Both must pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/client.rs \
        crates/sansio-mqtt-v5-protocol/src/lib.rs
git rm crates/sansio-mqtt-v5-protocol/src/proto.rs
git commit -m "refactor(v5-protocol): move Client into client.rs, delete proto.rs"
```

---

### Task 14: Final verification and cleanup

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/*.rs` (formatting + lint fixes only)

- [ ] **Step 1: Full workspace test suite**

```bash
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 2: Format and lint**

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
```
Fix any issues.

- [ ] **Step 3: Confirm `proto.rs` is gone**

```bash
test ! -f crates/sansio-mqtt-v5-protocol/src/proto.rs && echo "OK"
```
Expected: `OK`.

- [ ] **Step 4: Confirm size caps**

```bash
wc -l crates/sansio-mqtt-v5-protocol/src/**/*.rs
```
Check against soft caps: `client.rs` ≤ 250 LOC, `state/*.rs` ≤ 400 LOC each, every other module ≤ 300 LOC. If `connected.rs` exceeds 400 LOC consider extracting the QoS handlers into helper functions within the same file.

- [ ] **Step 5: Confirm all spec citations are present**

```bash
grep -r "\[MQTT-" crates/sansio-mqtt-v5-protocol/src/ | wc -l
grep -r "\[MQTT-" crates/sansio-mqtt-v5-protocol/src/proto.rs 2>/dev/null | wc -l
```
The first number must be non-zero. The second returns `0` (file is deleted).

- [ ] **Step 6: Commit if any fmt/lint changes**

```bash
git add -A
git commit -m "style(v5-protocol): fmt and clippy cleanup after FSM refactor"
```
Skip if nothing changed.
