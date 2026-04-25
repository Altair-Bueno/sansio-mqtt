# sansio-mqtt-v5-protocol MVP Client Design

## Goal

Finish `crates/sansio-mqtt-v5-protocol` as a functional sansio MQTT v5 client
MVP with strict protocol handling, typed state transitions, and no_std-first
constraints.

This design targets a coherent MVP implementation in the existing scaffold (no
new crates), with explicit room for later QoS1/QoS2 expansion.

## Scope

### In scope (MVP)

- Client lifecycle: open socket, CONNECT/CONNACK handshake, DISCONNECT, socket
  close handling.
- Inbound parsing loop with buffering/remainder handling in `handle_read`.
- Outbound user commands:
  - connect
  - publish (QoS0 only in MVP)
  - subscribe
  - unsubscribe
  - disconnect
- Inbound broker packets:
  - CONNACK
  - PUBLISH (QoS0)
  - SUBACK
  - UNSUBACK
  - PINGRESP
  - DISCONNECT
- Keepalive ping cycle (PINGREQ/PINGRESP).
- Enforce broker-advertised limits from CONNACK on outbound behavior.
- Strict error policy: treat client-side SHOULD close guidance as MUST in this
  implementation.

### Out of scope (for this MVP)

- QoS1/QoS2 publish flows (PUBACK/PUBREC/PUBREL/PUBCOMP stateful flows).
- Session persistence beyond in-memory runtime state.
- Transport/runtime concerns (`tokio` crate remains separate).

## Non-negotiable constraints

- MQTT v5.0 spec is authoritative.
- `#![no_std]`, `extern crate alloc`, `#![forbid(unsafe_code)]` remain enforced.
- Use `sansio::Protocol` trait implementation as the protocol boundary.
- Keep changes atomic and minimal within `sansio-mqtt-v5-protocol`.
- Conformance references should use `[MQTT-x.x.x-y]` where normative behavior is
  implemented.

## Architecture

Implement a typed-state-first protocol core inside `src/proto.rs` with
`Protocol` methods as thin adapters.

### State model

- `Start`
- `Connecting`
- `Connected`
- `Disconnected`

Each transition is explicit and invalid transitions return
`Error::InvalidStateTransition`.

### Internal responsibilities

- `handle_read`: parse loop, control-packet dispatch, remainder buffering.
- `handle_read_control_packet`: route parsed packets by state.
- `handle_write`: validate and encode outbound user intents.
- `handle_event`: socket lifecycle integration.
- `handle_timeout`: keepalive scheduling and timeout-driven actions.
- queue outputs through:
  - `read_queue: VecDeque<UserWriteOut>`
  - `write_queue: VecDeque<Bytes>`
  - `action_queue: VecDeque<DriverEventOut>`

### Capabilities/limits cache

On CONNACK, store negotiated constraints in internal fields and enforce on
subsequent outbound packets:

- Maximum Packet Size
- Topic Alias Maximum
- Receive Maximum
- Server Keep Alive (overrides client keepalive when present)

## Data and API updates

### `types.rs`

1. Add QoS to outbound message API:

```rust
pub struct ClientMessage {
    pub topic: Topic,
    pub payload: Payload,
    pub qos: Qos,
    // existing fields...
}
```

2. Expand `Config` from empty struct to protocol/parser/runtime knobs used by
   the MVP implementation.

3. Replace empty `Error` enum with concrete variants for parse, protocol, state,
   encoding, and MVP limitations.

### Known limitation (explicit)

QoS1/QoS2 outbound publish is intentionally rejected in MVP even though
`ClientMessage` now carries `Qos`.

- Behavior: `qos != Qos::AtMostOnce` returns `Error::UnsupportedQosForMvp`.
- Follow-up required: implement QoS1/QoS2 packet flows and remove this guard.

## `handle_read` design

Use the provided buffering strategy to avoid unnecessary allocations when the
read buffer is empty, then parse in a loop with `ControlPacket::parse(...)`.

Core behavior:

1. If no buffered remainder exists, parse `msg` directly.
2. If remainder exists, append and parse combined bytes.
3. Loop parse control packets until:
   - complete parse success -> dispatch packet
   - incomplete parse -> stop and keep remainder
   - parse backtrack/cut error -> protocol failure path
4. Preserve only unconsumed bytes back into `self.read_buffer`.

Parser settings are configured from `self.config` and negotiated limits.

## Protocol/error handling policy

For malformed packets and protocol violations, this implementation uses strict
close semantics:

- Send DISCONNECT with the best available reason code when possible.
- Queue `DriverEventOut::CloseSocket`.
- Transition to `Disconnected`.
- Return an error.

This intentionally treats client-side SHOULD guidance in section 4.13.1 as MUST
for robustness in this MVP.

Reason code preference:

- specific code when defined
- otherwise `0x81` (Malformed Packet) or `0x82` (Protocol Error)

## Behavior details by method

### `handle_event`

- `SocketConnected` in `Start/Disconnected`: transition to `Connecting`, emit
  CONNECT bytes.
- `SocketClosed`: transition to `Disconnected`, emit
  `UserWriteOut::Disconnected`.
- socket error event: route to standardized close/error path.

### `handle_write`

- `Connect`: only valid before established session.
- `PublishMessage`:
  - enforce negotiated limits
  - accept only QoS0 in MVP
  - encode PUBLISH and queue bytes
- `Subscribe`/`Unsubscribe`:
  - allocate packet id
  - encode and queue
- `Disconnect`:
  - encode DISCONNECT
  - queue close action and transition

### `handle_read_control_packet`

- In `Connecting`: accept only CONNACK.
- In `Connected`:
  - PUBLISH QoS0 -> emit `UserWriteOut::ReceivedMessage`
  - SUBACK/UNSUBACK/PINGRESP/DISCONNECT handled per flow
  - unexpected packet -> protocol error path

### `handle_timeout`

- In `Connected`, send PINGREQ when keepalive deadline expires.
- Reschedule timeout based on negotiated keepalive.
- If timeout policy is violated, close connection.

## Tests (behavior-first)

Add tests in protocol crate for:

1. Handshake success path (`SocketConnected` -> CONNECT -> CONNACK -> Connected
   event).
2. Fragmented packet parsing and read remainder behavior.
3. Malformed packet handling: disconnect + close + error.
4. Invalid-state packet handling (e.g. PUBLISH before CONNACK).
5. QoS0 publish accepted and encoded.
6. QoS1/QoS2 publish rejected with explicit MVP error.
7. CONNACK limits enforced on outbound commands.
8. Keepalive timeout sends PINGREQ; PINGRESP handling.
9. Socket close drives disconnected event and state transition.

Where practical, annotate tests/implementation with spec markers like
`[MQTT-4.13.1-1]` and packet-specific normative IDs.

## Verification gates

Before implementation completion claims:

- `cargo fmt`
- `cargo clippy`
- `cargo test -p sansio-mqtt-v5-protocol`

Optionally run full workspace checks after protocol tests are stable.

## Implementation sequence (high level)

1. Expand `types.rs` (`Config`, `Error`, `ClientMessage.qos`).
2. Implement typed-state transition helpers in `proto.rs`.
3. Implement `handle_read` with parser configuration and remainder logic.
4. Implement inbound control packet dispatch.
5. Implement outbound command handling with negotiated-limit guards.
6. Implement keepalive timeout path.
7. Add/adjust tests for the MVP behaviors above.
8. Run fmt, clippy, and tests.
