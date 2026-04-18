# UserWrite Message ID and Enum Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split inbound message output variants, introduce opaque inbound message ids, and simplify driver-facing enum derives while preserving protocol behavior.

**Architecture:** First update protocol public types and compile-facing tests, then adapt protocol state-machine internals to use opaque ids, then update tokio bridge/event mapping. Keep behavior unchanged except the requested output/API surface changes and derive pruning.

**Tech Stack:** Rust, Cargo, protocol state-machine tests, tokio integration tests.

---

### Task 1: Reshape protocol public types and add opaque `InboundMessageId`

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/lib.rs`
- Test: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing tests for new output/input API shape**

Update `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` test(s) to compile against:

```rust
let msg = BrokerMessage::default();
let no_ack = UserWriteOut::ReceivedMessage(msg.clone());
let with_ack = UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, msg);

let ack_cmd = UserWriteIn::AcknowledgeMessage(id);
let reject_cmd = UserWriteIn::RejectMessage(id, IncomingRejectReason::UnspecifiedError);
```

Also add one API-shape test that ensures `DriverEventIn`/`DriverEventOut` are still pattern-matchable without requiring equality.

- [ ] **Step 2: Run targeted test to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol user_write_out_exposes_qos_delivery_events_with_packet_id -- --nocapture
```

Expected: FAIL due to missing split variants and missing `InboundMessageId` type wiring.

- [ ] **Step 3: Implement type/API changes in `types.rs`**

In `crates/sansio-mqtt-v5-protocol/src/types.rs`:

1) Add opaque id:

```rust
#[derive(Debug)]
pub struct InboundMessageId(NonZero<u16>);

impl InboundMessageId {
    pub(crate) fn new(id: NonZero<u16>) -> Self { Self(id) }
    pub(crate) fn get(self) -> NonZero<u16> { self.0 }
}
```

2) Split output variants:

```rust
ReceivedMessage(BrokerMessage),
ReceivedMessageWithRequiredAcknowledgement(InboundMessageId, BrokerMessage),
```

3) Update command ids:

```rust
AcknowledgeMessage(InboundMessageId),
RejectMessage(InboundMessageId, IncomingRejectReason),
```

4) Remove derives from driver-facing enums to `#[derive(Debug)]` only:
- `UserWriteOut`
- `UserWriteIn`
- `DriverEventIn`
- `DriverEventOut`

**Important:** If compilation requires adding any extra derive trait, stop and ask user for explicit approval before adding it.

5) Re-export `InboundMessageId` from `crates/sansio-mqtt-v5-protocol/src/lib.rs`.

- [ ] **Step 4: Run targeted protocol test to verify GREEN for API shape**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol user_write_out_exposes_qos_delivery_events_with_packet_id
```

Expected: PASS or progression to later runtime-flow failures only.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/sansio-mqtt-v5-protocol/src/types.rs crates/sansio-mqtt-v5-protocol/src/lib.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "refactor(protocol): split inbound message outputs and add opaque ids"
```

### Task 2: Adapt protocol state machine internals to opaque ids and split outputs

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Add failing behavior assertions for new variants**

In `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`, update relevant tests to require:
- QoS0 inbound publish emits `ReceivedMessage(msg)`
- QoS1/QoS2 inbound publish emits `ReceivedMessageWithRequiredAcknowledgement(id, msg)`
- `AcknowledgeMessage(id)` / `RejectMessage(id, reason)` behavior still matches previous semantics

Keep existing manual ack/reject behavior assertions intact.

- [ ] **Step 2: Run targeted tests to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol inbound_qos1_publish_waits_for_app_ack_then_sends_puback inbound_qos2_publish_waits_for_app_ack_then_sends_pubrec_and_completes_on_pubrel -- --nocapture
```

Expected: FAIL until output variant and id plumbing are updated.

- [ ] **Step 3: Update `proto.rs` mapping and id handling**

In `crates/sansio-mqtt-v5-protocol/src/proto.rs`:

1) When emitting inbound user messages:
- QoS0 -> `UserWriteOut::ReceivedMessage(message)`
- QoS1/QoS2 -> `UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(InboundMessageId::new(packet_id), message)`

2) For command handling:
- `UserWriteIn::AcknowledgeMessage(id)` use `id.get()` internally
- `UserWriteIn::RejectMessage(id, reason)` use `id.get()` internally

3) Keep existing conflict/reject-duplicate hardening behavior unchanged.

- [ ] **Step 4: Run full protocol tests**

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol
cargo test -p sansio-mqtt-v5-protocol
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "refactor(protocol): route manual ack flow through opaque inbound ids"
```

### Task 3: Update tokio event bridge and tests for split message variants

**Files:**
- Modify: `crates/sansio-mqtt-v5-tokio/src/event.rs`
- Modify: `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs`
- Modify: `crates/sansio-mqtt-v5-tokio/examples/cli.rs` (if needed for new event shape)

- [ ] **Step 1: Write failing tokio mapping tests first**

Update/add tests in `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs` to assert mapping of:

```rust
UserWriteOut::ReceivedMessage(msg)
UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, msg)
```

to distinct public `Event` variants.

- [ ] **Step 2: Run targeted tokio test to verify RED**

```bash
cargo test -p sansio-mqtt-v5-tokio --test client_event_loop maps_received_message_with_optional_packet_id -- --nocapture
```

Expected: FAIL due to outdated event mapping.

- [ ] **Step 3: Implement event mapping updates**

In `crates/sansio-mqtt-v5-tokio/src/event.rs`:

Replace existing message event with two variants:

```rust
Message(BrokerMessage),
MessageWithRequiredAcknowledgement(InboundMessageId, BrokerMessage),
```

Update `from_protocol_output` mapping accordingly.

Adjust `crates/sansio-mqtt-v5-tokio/examples/cli.rs` pattern matches for the new variants if compilation requires.

- [ ] **Step 4: Run tokio tests and workspace tests**

```bash
cargo test -p sansio-mqtt-v5-tokio
cargo test -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add crates/sansio-mqtt-v5-tokio/src/event.rs crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs crates/sansio-mqtt-v5-tokio/examples/cli.rs
git commit -m "refactor(tokio): split inbound message events by ack requirement"
```

### Task 4: Derive-pruning enforcement and final checks

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` (assertion style updates)

- [ ] **Step 1: Replace equality-based assertions for driver-facing enums**

In protocol tests, migrate any `assert_eq!` relying on `PartialEq` for:
- `UserWriteOut`
- `UserWriteIn`
- `DriverEventIn`
- `DriverEventOut`

to pattern assertions (`matches!`) + field-level assertions.

- [ ] **Step 2: Run final verification suite**

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
cargo test -q
```

Expected: PASS.

- [ ] **Step 3: Confirm no extra derives were added without review**

Run:

```bash
rg "#\[derive\(" crates/sansio-mqtt-v5-protocol/src/types.rs
```

Verify driver-facing enums are `Debug` only.

If compile currently depends on extra traits for these enums, STOP and request user review before adding traits.

- [ ] **Step 4: Commit Task 4**

```bash
git add crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs crates/sansio-mqtt-v5-protocol/src/types.rs
git commit -m "test(protocol): align assertions with debug-only driver enums"
```

## Spec Coverage Check

- Split `ReceivedMessage` into two explicit variants: Task 1 + Task 2 + Task 3.
- Opaque wrapper type for inbound ids with no public construction: Task 1 + Task 2.
- Remove `Clone`/`PartialEq`/`Eq` derives from driver-facing enums: Task 1 + Task 4.
- “If extra derive is required, review with user”: explicit gate in Task 1 and Task 4.

## Placeholder Scan

- No `TODO`/`TBD` placeholders.
- All steps include explicit files and executable commands.
- Test-first steps defined for each behavior-changing task.

## Type Consistency Check

- Uses a single id wrapper name: `InboundMessageId`.
- Uses consistent split output names:
  - `ReceivedMessage`
  - `ReceivedMessageWithRequiredAcknowledgement`
- Ack/reject command signatures consistently use `InboundMessageId`.
