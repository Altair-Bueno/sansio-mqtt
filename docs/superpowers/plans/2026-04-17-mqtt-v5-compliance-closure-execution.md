# MQTT v5 Compliance Closure Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all remaining checklist items required to claim practical MQTT v5 client protocol compliance for `sansio-mqtt-v5-protocol`.

**Architecture:** Keep the existing sansio state machine, but fill protocol gaps with narrowly scoped extensions: explicit session-presence validation, complete reconnect replay semantics, topic-alias tables for inbound resolution, CONNECT property support, and capability guards on outbound operations. Use strict TDD and preserve existing queue-driven protocol API.

**Tech Stack:** Rust (`no_std` + `alloc`), `sansio`, `bytes`, `winnow`, `sansio-mqtt-v5-types`, `thiserror`, `tracing`.

---

## File Structure and Responsibilities

- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs`
  - Add missing configuration/option fields and user-facing outcome events required by compliance closure.
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
  - Implement missing state-machine and validation semantics.
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`
  - Add conformance regression tests for each missing requirement.
- Modify: `docs/superpowers/checklists/2026-04-17-mqtt-v5-client-compliance-closure.md`
  - Mark completed items and keep status aligned with code.

## Task 1: Session Present and Session-State Semantics

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing tests for session-present mismatch and state-discard rules**

```rust
#[test]
fn connack_session_present_without_local_session_is_protocol_error() {
    // no local session state, receive ConnAckKind::ResumePreviousSession
    // expect Err(ProtocolError) + CloseSocket
}

#[test]
fn non_resumed_connack_discards_all_local_session_state() {
    // seed inflight publishes + pending subscribe/unsubscribe
    // receive ConnAck success with Session Present=0
    // assert all local session state cleared
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p sansio-mqtt-v5-protocol connack_session_present_without_local_session_is_protocol_error non_resumed_connack_discards_all_local_session_state -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement Session Present enforcement and full discard behavior**

```rust
// proto.rs (shape)
fn has_local_session_state(&self) -> bool {
    !self.on_flight_sent.is_empty()
        || !self.on_flight_received.is_empty()
        || !self.pending_subscribe.is_empty()
        || !self.pending_unsubscribe.is_empty()
}

// In Connecting + CONNACK path:
// - if Session Present=1 and !has_local_session_state() => protocol close error
// - if Session Present=0 => clear full local session state
```

- [ ] **Step 4: Re-run targeted tests**

Run: `cargo test -p sansio-mqtt-v5-protocol connack_session_present_without_local_session_is_protocol_error non_resumed_connack_discards_all_local_session_state`

Expected: PASS.

## Task 2: Reconnect Replay Completeness (Include PUBREL)

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing test for unacknowledged PUBREL replay**

```rust
#[test]
fn resumed_session_replays_unacknowledged_pubrel() {
    // set outbound qos2 state to await pubcomp
    // reconnect with Session Present=1
    // expect PUBREL retransmitted (same packet id)
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p sansio-mqtt-v5-protocol resumed_session_replays_unacknowledged_pubrel -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement replay of PUBREL for `Qos2AwaitPubComp`**

```rust
// replay_outbound_inflight_with_dup():
match state {
    Qos2AwaitPubComp => enqueue PUBREL(packet_id),
    Qos1AwaitPubAck{..} | Qos2AwaitPubRec{..} => replay PUBLISH with DUP=1,
}
```

- [ ] **Step 4: Re-run targeted test**

Run: `cargo test -p sansio-mqtt-v5-protocol resumed_session_replays_unacknowledged_pubrel`

Expected: PASS.

## Task 3: Keep Alive Edge Cases (Server Keep Alive = 0 and Timing Contract)

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing test for CONNACK `server_keep_alive=0`**

```rust
#[test]
fn connack_server_keep_alive_zero_disables_keepalive_without_panic() {
    // connect then feed CONNACK with server_keep_alive=0
    // verify no panic, timeout disabled
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p sansio-mqtt-v5-protocol connack_server_keep_alive_zero_disables_keepalive_without_panic -- --nocapture`

Expected: FAIL (panic or incorrect behavior).

- [ ] **Step 3: Implement zero-safe keepalive assignment and timing contract cleanup**

```rust
// Instead of NonZero::expect for server_keep_alive:
self.keep_alive.interval_secs = self
    .negotiated_limits
    .server_keep_alive
    .and_then(NonZero::new)
    .or(self.pending_connect_options.keep_alive);
```

- [ ] **Step 4: Re-run targeted test**

Run: `cargo test -p sansio-mqtt-v5-protocol connack_server_keep_alive_zero_disables_keepalive_without_panic`

Expected: PASS.

## Task 4: Topic Alias Compliance (Inbound)

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs` (if additional internal helper types are exposed)
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing inbound topic-alias tests**

```rust
#[test]
fn inbound_publish_registers_topic_alias_then_resolves_alias_only_publish() {
    // first publish with topic + alias => register
    // second publish alias-only => resolve to previous topic
}

#[test]
fn inbound_publish_alias_only_unknown_alias_is_protocol_error() {
    // alias-only without prior registration => protocol error close
}

#[test]
fn inbound_publish_alias_exceeds_client_alias_max_is_protocol_error() {
    // enforce client-advertised alias max
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p sansio-mqtt-v5-protocol inbound_publish_registers_topic_alias_then_resolves_alias_only_publish inbound_publish_alias_only_unknown_alias_is_protocol_error inbound_publish_alias_exceeds_client_alias_max_is_protocol_error -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement inbound alias table and resolution rules**

```rust
// Add inbound_topic_aliases: BTreeMap<NonZero<u16>, Topic>
// On inbound publish:
// - if topic non-empty + alias present: register/replace alias mapping
// - if topic empty + alias present: resolve mapping or protocol error
// - enforce alias <= client topic_alias_maximum from CONNECT options
```

- [ ] **Step 4: Re-run alias tests**

Run: `cargo test -p sansio-mqtt-v5-protocol inbound_publish_registers_topic_alias_then_resolves_alias_only_publish inbound_publish_alias_only_unknown_alias_is_protocol_error inbound_publish_alias_exceeds_client_alias_max_is_protocol_error`

Expected: PASS.

## Task 5: CONNECT Property Surface Completion

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing tests for CONNECT-side Receive Maximum and Maximum Packet Size**

```rust
#[test]
fn connect_encodes_receive_maximum_when_configured() {
    // set ConnectionOptions.receive_maximum
    // assert encoded CONNECT includes property
}

#[test]
fn connect_encodes_maximum_packet_size_when_configured() {
    // set ConnectionOptions.maximum_packet_size
    // assert encoded CONNECT includes property
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p sansio-mqtt-v5-protocol connect_encodes_receive_maximum_when_configured connect_encodes_maximum_packet_size_when_configured -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement options fields and CONNECT property mapping**

```rust
// types.rs
pub receive_maximum: Option<NonZero<u16>>,
pub maximum_packet_size: Option<NonZero<u32>>,

// proto.rs build_connect_packet
receive_maximum: options.receive_maximum,
maximum_packet_size: options.maximum_packet_size,
```

- [ ] **Step 4: Re-run targeted tests**

Run: `cargo test -p sansio-mqtt-v5-protocol connect_encodes_receive_maximum_when_configured connect_encodes_maximum_packet_size_when_configured`

Expected: PASS.

## Task 6: Will QoS/Retain Representation and Encoding

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing test for will qos/retain mapping**

```rust
#[test]
fn build_connect_packet_maps_will_qos_and_retain_from_options() {
    // will.qos = ExactlyOnce, will.retain = true
    // assert ConnectWill fields preserve values
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p sansio-mqtt-v5-protocol build_connect_packet_maps_will_qos_and_retain_from_options -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Extend `Will` type and map fields**

```rust
// types.rs
pub qos: Qos,
pub retain: bool,

// proto.rs build_connect_packet
qos: will.qos,
retain: will.retain,
```

- [ ] **Step 4: Re-run targeted test**

Run: `cargo test -p sansio-mqtt-v5-protocol build_connect_packet_maps_will_qos_and_retain_from_options`

Expected: PASS.

## Task 7: Server Capability Enforcement and Subscribe Validation

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs` (if needed)
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing tests for capability enforcement**

```rust
#[test]
fn publish_qos_above_server_maximum_qos_is_rejected() {
    // CONNACK maximum_qos=AtMostOnce, attempt qos1 publish => protocol error
}

#[test]
fn publish_retain_when_server_retain_not_available_is_rejected() {
    // retain_available=false and retain=true => protocol error
}

#[test]
fn subscribe_shared_with_no_local_is_rejected() {
    // shared filter + no_local=true => protocol error
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p sansio-mqtt-v5-protocol publish_qos_above_server_maximum_qos_is_rejected publish_retain_when_server_retain_not_available_is_rejected subscribe_shared_with_no_local_is_rejected -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Store needed CONNACK capability flags and enforce in write paths**

```rust
// negotiated_limits add:
maximum_qos: Option<MaximumQoS>,
retain_available: bool,
wildcard_subscription_available: bool,
shared_subscription_available: bool,
subscription_identifiers_available: bool,

// enforce before enqueueing publish/subscribe
```

- [ ] **Step 4: Re-run targeted tests**

Run: `cargo test -p sansio-mqtt-v5-protocol publish_qos_above_server_maximum_qos_is_rejected publish_retain_when_server_retain_not_available_is_rejected subscribe_shared_with_no_local_is_rejected`

Expected: PASS.

## Task 8: Enhanced AUTH State Machine Completion

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs` (if user events/config needed)
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing tests for AUTH continuation and invalid transitions**

```rust
#[test]
fn connecting_auth_continue_then_connack_success_connects() {
    // accept AUTH ContinueAuthentication and later CONNACK success
}

#[test]
fn auth_in_connected_without_support_is_protocol_error() {
    // define expected behavior and assert strict close
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p sansio-mqtt-v5-protocol connecting_auth_continue_then_connack_success_connects auth_in_connected_without_support_is_protocol_error -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Implement explicit auth phase tracking and transitions**

```rust
// proto.rs
enum ConnectingPhase { AwaitConnAck, AuthInProgress }
// enforce AUTH/CONNACK transition validity
```

- [ ] **Step 4: Re-run targeted tests**

Run: `cargo test -p sansio-mqtt-v5-protocol connecting_auth_continue_then_connack_success_connects auth_in_connected_without_support_is_protocol_error`

Expected: PASS.

## Task 9: Conformance Evidence, Checklist Sync, and Final Verification

**Files:**
- Modify: `docs/superpowers/checklists/2026-04-17-mqtt-v5-client-compliance-closure.md`
- Create: `docs/superpowers/checklists/2026-04-17-mqtt-v5-client-requirement-traceability.md`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add/update exact `[MQTT-x.x.x-y]` markers for new logic**

```rust
// annotate new checks: Session Present mismatch, Session Present=0 discard,
// PUBREL replay, Topic Alias rules, subscribe no-local shared restriction, etc.
```

- [ ] **Step 2: Create requirement traceability document**

```markdown
# MQTT v5 Client Requirement Traceability

| Requirement | Spec Ref | Code Path | Test Name | Status |
|-------------|----------|-----------|-----------|--------|
| Session Present mismatch close | MQTT-3.2.2-4 | proto.rs:... | ... | PASS |
```

- [ ] **Step 3: Sync closure checklist statuses**

Update: `docs/superpowers/checklists/2026-04-17-mqtt-v5-client-compliance-closure.md`

- [ ] **Step 4: Run formatting and verification suite**

Run: `cargo fmt && cargo clippy -p sansio-mqtt-v5-protocol --all-targets && cargo test -p sansio-mqtt-v5-protocol && cargo test -p sansio-mqtt-v5-types`

Expected: tests PASS; clippy warnings may remain in external crate areas unless policy is tightened globally.

## Plan Self-Review

- **Spec coverage:**
  - Session-present and session-expiry semantics: Tasks 1 and 3.
  - Reconnect replay correctness (`PUBREL` included): Task 2.
  - Topic Alias inbound semantics: Task 4.
  - CONNECT-side limits + Will flags: Tasks 5 and 6.
  - Server capability and subscribe validation constraints: Task 7.
  - AUTH full-state behavior: Task 8.
  - Evidence and checklist closure: Task 9.
- **Placeholder scan:** All tasks include concrete files/tests/commands.
- **Type consistency:** Uses existing crate identifiers (`ConnectionOptions`, `Will`, `ClientState`, `on_flight_sent`, `pending_subscribe`) and introduces only explicit, scoped extensions.
