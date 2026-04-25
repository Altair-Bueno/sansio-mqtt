# Inbound Manual Acknowledge/Reject API Design

Date: 2026-04-19 Scope: `crates/sansio-mqtt-v5-protocol` (+
`sansio-mqtt-v5-tokio` event bridge)

## Goal

Allow applications to manually decide inbound message acknowledgment outcomes.

Specifically:

- Deliver inbound messages with an optional packet id reference.
- Add explicit application commands to acknowledge or reject inbound messages.
- Keep current reconnect/disconnect policy behavior (no new timeout semantics).

## API Changes

### `UserWriteOut`

Current:

- `ReceivedMessage(BrokerMessage)`

New:

- `ReceivedMessage(Option<NonZero<u16>>, BrokerMessage)`

Rules:

- QoS0 inbound publish => `None`
- QoS1/QoS2 inbound publish => `Some(packet_id)`

### `UserWriteIn`

Add:

- `AcknowledgeMessage(NonZero<u16>)`
- `RejectMessage(NonZero<u16>, IncomingRejectReason)`

### New protocol type

Add in `types.rs`:

- `IncomingRejectReason`

Intent:

- Strongly typed rejection semantics for inbound processing.
- Prevent raw-byte reason misuse.

## Behavioral Semantics

### QoS0 inbound publish

- Emit `ReceivedMessage(None, msg)`.
- No pending inbound-ack state is created.

### QoS1 inbound publish

- Emit `ReceivedMessage(Some(packet_id), msg)`.
- Track packet id in inbound pending map as `AwaitingAppDecision`.

On app command:

- `AcknowledgeMessage(packet_id)` => send `PUBACK(Success)`, clear pending
  entry.
- `RejectMessage(packet_id, reason)` => send `PUBACK(mapped failure reason)`,
  clear pending entry.

### QoS2 inbound publish

- Emit `ReceivedMessage(Some(packet_id), msg)`.
- Track packet id in inbound pending map as `AwaitingAppDecision`.

On app command:

- `AcknowledgeMessage(packet_id)` => send `PUBREC(Success)`, transition pending
  state to `AwaitingPubRel`.
- `RejectMessage(packet_id, reason)` => send `PUBREC(mapped failure reason)`,
  clear pending entry.

On inbound `PUBREL(packet_id)`:

- If state is `AwaitingPubRel` => send `PUBCOMP(Success)`, clear pending entry.
- If not found => existing packet-not-found behavior remains.

## Session/Disconnect Policy

No new timeout behavior is introduced.

Pending inbound decisions follow existing cleanup rules already used for
disconnect/session paths. This design does not add time-based
auto-ack/auto-reject.

## Error Handling

- `AcknowledgeMessage` or `RejectMessage` with unknown/non-pending packet id =>
  protocol error path (consistent with existing strict unknown-id handling
  style).
- Invalid reason mapping for packet class (QoS1 vs QoS2) must be unrepresentable
  via `IncomingRejectReason` modeling.

## `IncomingRejectReason` Mapping Strategy

Use a typed enum that can be deterministically mapped to wire reason codes:

- QoS1 path maps to `PubAckReasonCode`
- QoS2 path maps to `PubRecReasonCode`

Initial supported semantic reasons should be the practical failure reasons we
want to expose now (for example: not authorized, implementation specific error,
unspecified error), with future extension possible.

## Affected Areas

### Protocol crate

- `src/types.rs`
  - new `UserWriteOut::ReceivedMessage` payload shape
  - new `UserWriteIn` variants
  - new `IncomingRejectReason`
- `src/proto.rs`
  - inbound QoS1/QoS2 flow changes for deferred ack/reject
  - pending inbound state structure and transitions
  - outbound packet construction for ack/reject responses
- `tests/client_protocol.rs`
  - update existing assertions for `ReceivedMessage` shape
  - add tests for ack/reject command behavior and invalid packet-id cases

### Tokio crate

- `src/event.rs`
  - map updated `ReceivedMessage` form and keep public event behavior coherent
- `tests/client_event_loop.rs`
  - adapt protocol output mapping tests

## Non-Goals

- No automatic timeout-based ack/reject policy
- No persistence backend changes
- No protocol behavior changes outside inbound manual decision flow

## Verification Plan

Minimum required checks after implementation:

1. `cargo test -p sansio-mqtt-v5-protocol --test client_protocol`
2. `cargo test -p sansio-mqtt-v5-protocol`
3. `cargo test -p sansio-mqtt-v5-tokio`
4. `cargo test -q`

Expected result: all pass, with no regressions in existing QoS handling.
