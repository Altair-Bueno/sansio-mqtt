# UserWrite Message ID and Enum Simplification Design

Date: 2026-04-19
Scope: `crates/sansio-mqtt-v5-protocol` + `crates/sansio-mqtt-v5-tokio` event bridge/tests

## Goal

Simplify driver-facing protocol I/O types and enforce stronger message-id ownership semantics.

Requested outcomes:
- Split inbound delivery output into two explicit variants.
- Replace exposed `NonZero<u16>` inbound ack ids with an opaque protocol-owned wrapper type.
- Remove `Clone`/`PartialEq`/`Eq` derives from driver-facing enums (`UserWriteOut`, `UserWriteIn`, `DriverEventIn`, `DriverEventOut`).

## Decisions

## 1) Opaque inbound message id type

Add new protocol type:

- `InboundMessageId` in `crates/sansio-mqtt-v5-protocol/src/types.rs`
- Backed by `NonZero<u16>` with private field
- No public constructors
- Protocol internals construct/unwrap via `pub(crate)` helpers only

Intent:
- External crates cannot forge ids
- Driver must use ids emitted by protocol output

## 2) Split `ReceivedMessage` into two variants

Replace:
- `UserWriteOut::ReceivedMessage(Option<...>, BrokerMessage)`

With:
- `UserWriteOut::ReceivedMessage(BrokerMessage)`
- `UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(InboundMessageId, BrokerMessage)`

Mapping:
- Inbound QoS0 -> `ReceivedMessage`
- Inbound QoS1/QoS2 -> `ReceivedMessageWithRequiredAcknowledgement`

## 3) Switch ack/reject command id type

Update command API:
- `UserWriteIn::AcknowledgeMessage(InboundMessageId)`
- `UserWriteIn::RejectMessage(InboundMessageId, IncomingRejectReason)`

Internal state machine semantics remain unchanged; only id representation and variant shape change.

## 4) Driver-facing derive simplification

Update derives for these enums to `#[derive(Debug)]` only:
- `UserWriteOut`
- `UserWriteIn`
- `DriverEventIn`
- `DriverEventOut`

Policy:
- If compilation forces any extra trait derive, stop and review with user before keeping it.

## Behavioral Consistency

No protocol behavior expansion is introduced by this change.

Preserved behavior:
- Manual inbound ack/reject flow for QoS1/QoS2
- Unknown/invalid id handling remains protocol-error path
- QoS2 duplicate-after-reject and packet-id conflict hardening remains intact

## Affected Files

Protocol:
- `crates/sansio-mqtt-v5-protocol/src/types.rs`
- `crates/sansio-mqtt-v5-protocol/src/lib.rs` (re-export `InboundMessageId`)
- `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

Tokio bridge:
- `crates/sansio-mqtt-v5-tokio/src/event.rs`
- `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs`
- `crates/sansio-mqtt-v5-tokio/examples/cli.rs` (if message event shape touched)

## Test Strategy

1. Update/add tests first (RED):
   - Variant split shape checks (`ReceivedMessage` vs `ReceivedMessageWithRequiredAcknowledgement`)
   - Opaque id usage path in ack/reject commands
   - Tokio event mapping for both message variants

2. Implement minimal code to pass tests (GREEN).

3. Full verification:
   - `cargo test -p sansio-mqtt-v5-protocol --test client_protocol`
   - `cargo test -p sansio-mqtt-v5-protocol`
   - `cargo test -p sansio-mqtt-v5-tokio`
   - `cargo test -q`

## Non-Goals

- No new timeout policy
- No new packet semantics
- No transport/driver architecture changes beyond type surface and mappings
