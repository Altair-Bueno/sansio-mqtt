# MQTT v5 Protocol Spec Conformance Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all spec-vs-implementation gaps found in `sansio-mqtt-v5-protocol` while preserving current QoS behavior and test stability.

**Architecture:** Keep the existing sansio state machine shape, but add targeted substates/trackers for Keep Alive and AUTH handshake, explicit local-session reset rules for Clean Start and Session Expiry, and packet-id lifecycle tracking for SUBSCRIBE/UNSUBSCRIBE. Add tests first for each defect and fix incrementally.

**Tech Stack:** Rust (`no_std` + `alloc`), `sansio`, `bytes`, `winnow`, `sansio-mqtt-v5-types`, `thiserror`, `tracing`.

---

## File Structure and Responsibilities

- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs`
  - Add config and event/error fields needed for keepalive/auth/session-policy behaviors.
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
  - Implement all state-machine/spec fixes.
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`
  - Add failing regression tests for each issue and validate fixes.
- Optional split if test file grows too large:
  - Create: `crates/sansio-mqtt-v5-protocol/tests/client_protocol_keepalive.rs`
  - Create: `crates/sansio-mqtt-v5-protocol/tests/client_protocol_auth.rs`
  - Create: `crates/sansio-mqtt-v5-protocol/tests/client_protocol_packet_id.rs`

## Task 1: Correct Keep Alive Semantics (MQTT 3.1.2.10, 3.12.4, 4.13)

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing tests for keepalive behavior**

```rust
#[test]
fn keepalive_schedules_from_negotiated_interval_and_idle_activity() {
    // connect, negotiate server_keep_alive, verify timeout is set to deadline (not now)
    // verify outbound/inbound packet activity pushes deadline forward
}

#[test]
fn keepalive_pingreq_only_on_idle_expiry() {
    // ensure handle_timeout before idle deadline does nothing,
    // at/after deadline emits one PINGREQ and marks ping outstanding
}

#[test]
fn keepalive_timeout_without_pingresp_closes_connection() {
    // after PINGREQ outstanding and timeout elapsed, expect protocol close path
}
```

- [ ] **Step 2: Run keepalive tests and confirm failure**

Run: `cargo test -p sansio-mqtt-v5-protocol keepalive_ -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement keepalive trackers and deadline logic**

```rust
// proto.rs (shape)
struct KeepAliveState<Time> {
    interval: Option<core::time::Duration>,
    last_network_activity: Option<Time>,
    ping_outstanding_since: Option<Time>,
}

fn mark_network_activity(&mut self, now: Time) { /* ... */ }
fn next_keepalive_deadline(&self) -> Option<Time> { /* ... */ }
```

- [ ] **Step 4: Wire keepalive to read/write/timeout paths**

```rust
// after successful send/read control packet, call mark_network_activity
// handle_timeout:
// - if not expired -> no-op
// - if expired and no ping outstanding -> send PINGREQ, set ping_outstanding_since
// - if ping outstanding and timeout elapsed -> close with KeepAliveTimeout
```

- [ ] **Step 5: Re-run keepalive tests**

Run: `cargo test -p sansio-mqtt-v5-protocol keepalive_`

Expected: PASS.

## Task 2: Add AUTH Flow Support During Connecting (MQTT 3.15, 4.12)

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs` (if user-visible events/config needed)
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing tests for AUTH in connecting phase**

```rust
#[test]
fn connecting_accepts_auth_and_stays_open() {
    // after CONNECT sent, inbound AUTH should not force protocol error close
}

#[test]
fn connecting_auth_then_connack_success_transitions_connected() {
    // AUTH exchange followed by successful CONNACK should connect
}
```

- [ ] **Step 2: Run AUTH tests and confirm failure**

Run: `cargo test -p sansio-mqtt-v5-protocol connecting_auth -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement connecting AUTH acceptance and transitions**

```rust
// proto.rs (shape)
enum ConnectingPhase {
    AwaitConnAck,
    AuthInProgress,
}

// handle_read_control_packet in Connecting:
// - AUTH => transition/continue auth phase, optional user event
// - CONNACK success => Connected
// - truly invalid packet => protocol error close
```

- [ ] **Step 4: Re-run AUTH tests**

Run: `cargo test -p sansio-mqtt-v5-protocol connecting_auth`

Expected: PASS.

## Task 3: Enforce Clean Start Local Session Reset (MQTT 3.1.2.4)

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing clean-start reset test**

```rust
#[test]
fn clean_start_true_clears_local_session_before_connect() {
    // seed local inflight state, issue Connect(clean_start=true), SocketConnected,
    // assert inflight state cleared before/at CONNECT emission
}
```

- [ ] **Step 2: Run test and confirm failure**

Run: `cargo test -p sansio-mqtt-v5-protocol clean_start_true_clears_local_session_before_connect -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement clean-start pre-connect reset**

```rust
// SocketConnected / CONNECT path
if self.pending_connect_options.clean_start {
    self.reset_inflight_transactions();
    self.clear_pending_subscriptions();
}
```

- [ ] **Step 4: Re-run clean-start test**

Run: `cargo test -p sansio-mqtt-v5-protocol clean_start_true_clears_local_session_before_connect`

Expected: PASS.

## Task 4: Respect Session Expiry on Disconnect/Close Persistence

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing session-expiry persistence tests**

```rust
#[test]
fn session_with_expiry_keeps_inflight_across_graceful_disconnect() {
    // connect with non-zero session expiry, create inflight qos txn,
    // disconnect and reconnect with session present => txn still resumable
}

#[test]
fn zero_session_expiry_clears_inflight_on_disconnect() {
    // connect with zero expiry and ensure disconnect clears state
}
```

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test -p sansio-mqtt-v5-protocol session_.*expiry -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement effective-session policy gate for reset paths**

```rust
// track effective session policy from CONNECT (+ DISCONNECT override if present)
fn should_persist_session_state(&self) -> bool { /* ... */ }

// disconnect/close/socket paths:
if !self.should_persist_session_state() {
    self.reset_inflight_transactions();
    self.clear_pending_subscriptions();
}
```

- [ ] **Step 4: Re-run session-expiry tests**

Run: `cargo test -p sansio-mqtt-v5-protocol session_.*expiry`

Expected: PASS.

## Task 5: Complete Packet Identifier Lifecycle for SUBSCRIBE/UNSUBSCRIBE

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs` (if internal state/types exposed)
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing packet-id tracking tests**

```rust
#[test]
fn subscribe_tracks_packet_id_until_suback() {
    // SUBSCRIBE consumes PID; reused PID is not allowed until SUBACK
}

#[test]
fn unsubscribe_tracks_packet_id_until_unsuback() {
    // same for UNSUBSCRIBE/UNSUBACK
}

#[test]
fn unknown_suback_or_unsuback_is_protocol_error() {
    // unmatched ACK must trigger strict protocol close
}
```

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test -p sansio-mqtt-v5-protocol subscribe_tracks_packet_id_until_suback unsubscribe_tracks_packet_id_until_unsuback unknown_suback_or_unsuback_is_protocol_error -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Add pending maps and PID allocator safety across packet classes**

```rust
// proto.rs
pending_subscribe: BTreeMap<NonZero<u16>, ()>,
pending_unsubscribe: BTreeMap<NonZero<u16>, ()>,

fn next_packet_id_checked(&mut self) -> Result<NonZero<u16>, Error> {
    // ensure not used in outbound qos inflight OR pending_subscribe OR pending_unsubscribe
}
```

- [ ] **Step 4: Handle SUBACK/UNSUBACK by correlation and release**

```rust
ControlPacket::SubAck(suback) => {
    if self.pending_subscribe.remove(&suback.packet_id).is_none() {
        self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
        return Err(Error::ProtocolError);
    }
    Ok(())
}
```

- [ ] **Step 5: Re-run packet-id tests**

Run: `cargo test -p sansio-mqtt-v5-protocol subscribe_tracks_packet_id_until_suback unsubscribe_tracks_packet_id_until_unsuback unknown_suback_or_unsuback_is_protocol_error`

Expected: PASS.

## Task 6: Spec Markers and Final Verification

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add exact conformance markers near new normative logic**

```rust
// KeepAlive: [MQTT-3.1.2-24] etc (use exact IDs from spec table where available)
// AUTH exchange: [MQTT-3.15.0-*] / [MQTT-4.12.0-*]
// Clean Start / Session state: [MQTT-3.1.2-4]
// PID lifecycle and ACK correlation: [MQTT-2.2.1-*], [MQTT-3.8.*], [MQTT-3.10.*]
```

- [ ] **Step 2: Run full checks**

Run: `cargo fmt && cargo clippy -p sansio-mqtt-v5-protocol --all-targets && cargo test -p sansio-mqtt-v5-protocol`

Expected: PASS (warnings in unrelated crates acceptable unless policy changed).

- [ ] **Step 3: Optional compatibility check**

Run: `cargo test -p sansio-mqtt-v5-types`

Expected: PASS.

## Plan Self-Review

- **Spec coverage:**
  - Keepalive correctness: Task 1.
  - AUTH during connecting: Task 2.
  - Clean Start local reset: Task 3.
  - Session-expiry-aware state persistence: Task 4.
  - SUBSCRIBE/UNSUBSCRIBE packet-id lifecycle: Task 5.
  - Marker and verification gate: Task 6.
- **Placeholder scan:** All tasks include concrete files, tests, and commands.
- **Type consistency:** Uses existing protocol naming (`on_flight_sent`, `on_flight_received`, `ClientState`) and extends with explicit pending maps/state helpers.
