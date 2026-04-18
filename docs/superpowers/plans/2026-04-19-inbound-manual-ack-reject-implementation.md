# Inbound Manual Acknowledge/Reject Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add application-driven inbound message acknowledgment/rejection using packet-id aware message delivery and explicit acknowledge/reject commands.

**Architecture:** Extend protocol IO surface first (`UserWriteOut`/`UserWriteIn` + reject reason type), then implement inbound pending-state transitions in `proto.rs`, and finally adapt tokio mapping and tests. Keep existing reconnect/session semantics and avoid introducing timeout policy.

**Tech Stack:** Rust (no_std + alloc), Cargo tests, sansio protocol state machine, tokio bridge tests.

---

### Task 1: Extend public types for manual inbound ack/reject

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`
- Test: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing API-shape test updates first**

Update `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` test `user_write_out_exposes_qos_delivery_events_with_packet_id` to expect:

```rust
let msg = BrokerMessage::default();
let out = UserWriteOut::ReceivedMessage(Some(packet_id), msg.clone());
assert!(matches!(out, UserWriteOut::ReceivedMessage(Some(_), _)));

let ack = UserWriteIn::AcknowledgeMessage(packet_id);
let reject = UserWriteIn::RejectMessage(packet_id, IncomingRejectReason::UnspecifiedError);
assert!(matches!(ack, UserWriteIn::AcknowledgeMessage(_)));
assert!(matches!(reject, UserWriteIn::RejectMessage(_, _)));
```

Keep existing publish-delivery tuple variant assertions intact.

- [ ] **Step 2: Run targeted test to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol user_write_out_exposes_qos_delivery_events_with_packet_id -- --nocapture
```

Expected: compile/test failure due to missing `ReceivedMessage` tuple order or missing `UserWriteIn` variants/type.

- [ ] **Step 3: Implement type API changes in protocol types**

In `crates/sansio-mqtt-v5-protocol/src/types.rs`:

1. Update `UserWriteOut`:

```rust
ReceivedMessage(Option<NonZero<u16>>, BrokerMessage),
```

2. Add reject reason enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomingRejectReason {
    UnspecifiedError,
    ImplementationSpecificError,
    NotAuthorized,
    TopicNameInvalid,
    QuotaExceeded,
    PayloadFormatInvalid,
}
```

3. Add `UserWriteIn` variants:

```rust
AcknowledgeMessage(NonZero<u16>),
RejectMessage(NonZero<u16>, IncomingRejectReason),
```

4. Re-export `IncomingRejectReason` from `crates/sansio-mqtt-v5-protocol/src/lib.rs`.

- [ ] **Step 4: Run targeted test to verify GREEN for API shape**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol user_write_out_exposes_qos_delivery_events_with_packet_id
```

Expected: passes or progresses to next failing tests in state-machine paths.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/sansio-mqtt-v5-protocol/src/types.rs crates/sansio-mqtt-v5-protocol/src/lib.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "feat(protocol): add packet-id aware inbound delivery API"
```

### Task 2: Implement protocol state machine for app-driven ack/reject

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing behavior tests for QoS1 and QoS2 manual decisions**

Append tests in `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`:

```rust
#[test]
fn inbound_qos1_publish_waits_for_app_ack_then_sends_puback() { /* ... */ }

#[test]
fn inbound_qos1_publish_reject_sends_puback_failure_reason() { /* ... */ }

#[test]
fn inbound_qos2_publish_waits_for_app_ack_then_sends_pubrec_and_completes_on_pubrel() { /* ... */ }

#[test]
fn inbound_qos2_publish_reject_sends_pubrec_failure_and_clears_state() { /* ... */ }

#[test]
fn manual_ack_or_reject_unknown_packet_id_is_protocol_error() { /* ... */ }
```

For RED, assert that current behavior incorrectly auto-acks on inbound QoS1/QoS2 publish.

- [ ] **Step 2: Run only new tests to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol inbound_qos1_publish_waits_for_app_ack_then_sends_puback inbound_qos1_publish_reject_sends_puback_failure_reason inbound_qos2_publish_waits_for_app_ack_then_sends_pubrec_and_completes_on_pubrel inbound_qos2_publish_reject_sends_pubrec_failure_and_clears_state manual_ack_or_reject_unknown_packet_id_is_protocol_error -- --nocapture
```

Expected: FAIL due to current auto-ack behavior and missing command handling.

- [ ] **Step 3: Add inbound pending-decision state and command handlers**

In `crates/sansio-mqtt-v5-protocol/src/proto.rs`:

1. Introduce inbound state enum:

```rust
enum InboundInflightState {
    Qos1AwaitAppDecision,
    Qos2AwaitAppDecision,
    Qos2AwaitPubRel,
}
```

2. Replace `on_flight_received: BTreeMap<NonZero<u16>, ()>` with map to `InboundInflightState`.

3. On inbound `PUBLISH`:
- QoS0: emit `UserWriteOut::ReceivedMessage(None, message)`.
- QoS1: emit `UserWriteOut::ReceivedMessage(Some(packet_id), message)` and store `Qos1AwaitAppDecision`.
- QoS2 first delivery: emit `UserWriteOut::ReceivedMessage(Some(packet_id), message)` and store `Qos2AwaitAppDecision`.
- QoS2 duplicate while waiting: do not duplicate user delivery.

4. Handle `UserWriteIn::AcknowledgeMessage(packet_id)`:
- QoS1 state => send `PUBACK(Success)`, clear.
- QoS2 await-app => send `PUBREC(Success)`, transition to `Qos2AwaitPubRel`.
- else => protocol error.

5. Handle `UserWriteIn::RejectMessage(packet_id, reason)`:
- Map reason to `PubAckReasonCode` or `PubRecReasonCode` depending on stored state.
- QoS1 await-app => send `PUBACK(failure)`, clear.
- QoS2 await-app => send `PUBREC(failure)`, clear.
- else => protocol error.

6. On inbound `PUBREL(packet_id)`:
- only valid for `Qos2AwaitPubRel`, then send `PUBCOMP(Success)` and clear.
- otherwise preserve existing packet-not-found/protocol-error behavior.

- [ ] **Step 4: Run targeted tests to verify GREEN**

Re-run new tests from Step 2. Expected: PASS.

- [ ] **Step 5: Run full protocol test suite**

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol
cargo test -p sansio-mqtt-v5-protocol
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "feat(protocol): implement manual inbound ack/reject flow"
```

### Task 3: Tokio event mapping and integration tests

**Files:**
- Modify: `crates/sansio-mqtt-v5-tokio/src/event.rs`
- Modify: `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs`

- [ ] **Step 1: Write failing tokio mapping tests first**

Update/add tests in `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs`:

```rust
#[test]
fn maps_received_message_with_optional_packet_id() {
    // assert Event mapping from UserWriteOut::ReceivedMessage(None|Some(id), msg)
}

#[test]
fn maps_publish_dropped_and_delivery_events_tuple_variants() {
    // assert tuple variant mapping remains correct
}
```

- [ ] **Step 2: Run tokio mapping tests to verify RED**

```bash
cargo test -p sansio-mqtt-v5-tokio --test client_event_loop maps_received_message_with_optional_packet_id -- --nocapture
```

Expected: FAIL due to event mapping mismatch.

- [ ] **Step 3: Update tokio event model and conversion logic**

In `crates/sansio-mqtt-v5-tokio/src/event.rs`:

1. Add packet id to message event:

```rust
Message(Option<NonZero<u16>>, BrokerMessage),
```

2. Update `from_protocol_output` mapping for new `ReceivedMessage` ordering:

```rust
UserWriteOut::ReceivedMessage(packet_id, message) => Self::Message(packet_id, message)
```

3. Keep tuple-variant mapping aligned with current protocol enum style.

- [ ] **Step 4: Run tokio tests and full workspace verification**

```bash
cargo test -p sansio-mqtt-v5-tokio
cargo test -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add crates/sansio-mqtt-v5-tokio/src/event.rs crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs
git commit -m "feat(tokio): expose packet-id aware inbound message events"
```

### Task 4: Final consistency checks and reason mapping coverage

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add reason mapping tests for reject reasons**

Add tests ensuring `IncomingRejectReason` maps correctly:

```rust
#[test]
fn reject_reason_maps_to_puback_failure_codes_for_qos1() { /* ... */ }

#[test]
fn reject_reason_maps_to_pubrec_failure_codes_for_qos2() { /* ... */ }
```

- [ ] **Step 2: Run reason-mapping tests (RED -> GREEN cycle)**

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol reject_reason_maps_to_puback_failure_codes_for_qos1 reject_reason_maps_to_pubrec_failure_codes_for_qos2 -- --nocapture
```

Expected: PASS after implementation.

- [ ] **Step 3: Final required verification suite**

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
cargo test -q
```

Expected: PASS, no regressions.

- [ ] **Step 4: Commit Task 4**

```bash
git add crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "test(protocol): cover inbound reject reason mappings"
```

## Spec Coverage Check

- `ReceivedMessage(Option<NonZero<u16>>, BrokerMessage)` surface: Task 1 + Task 3.
- `AcknowledgeMessage` and `RejectMessage` commands: Task 1 + Task 2.
- QoS1/QoS2 manual decision flow: Task 2.
- Unknown packet-id strict handling: Task 2 tests.
- Typed reject reason + mapping: Task 1 + Task 4.
- No timeout policy changes: preserved by design in Task 2 implementation scope.

## Placeholder Scan

- No TBD/TODO placeholders.
- Every code-change task includes explicit files and concrete commands.
- Tests and verification commands are specified and executable.

## Type Consistency Check

- `ReceivedMessage` ordering is consistently packet-id first.
- `UserWriteIn` command names are consistent across tasks.
- `IncomingRejectReason` is the only new rejection type introduced and used consistently.
