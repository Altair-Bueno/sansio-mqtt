# ClientSettings Effective Limit Negotiation and Parser Integration Design

Date: 2026-04-19
Scope: `crates/sansio-mqtt-v5-protocol` (`types.rs`, `proto.rs`) and tokio integration call sites

## Goal

Expand `ClientSettings` from parser-only knobs into application policy input for protocol negotiation,
and recompute effective limits whenever state changes so protocol checks and parser configuration
always use the currently applied envelope.

Core requirement:
- Always enforce the most restrictive limit among participating sources.
- Broker may further restrict limits per connection.
- Negotiated/effective limits are transient and must be recomputed after reconnect.

## Non-Goals

- No persistence of broker-negotiated capability envelope across network reconnections.
- No change to MQTT wire protocol semantics.
- No external storage API in this change.

## Spec and lifecycle grounding

- MQTT v5 reconnect requires a new `CONNECT`/`CONNACK` exchange per transport connection.
- `Session Present=1` resumes session data, but capability limits are still renegotiated.
- Therefore negotiated/effective limits are per-connection transient data.

Placement decision:
- Persistent: `ClientState` only session continuity data.
- Transient: `ClientScratchpad` negotiated and effective limits.

## Data model

## 1) `ClientSettings` additions (application policy)

`ClientSettings` keeps parser settings and gains local negotiation policy fields:

- Parser maxima (existing):
  - `max_bytes_string: u16`
  - `max_bytes_binary_data: u16`
  - `max_remaining_bytes: u64`
  - `max_subscriptions_len: u32`
  - `max_user_properties_len: usize`
- New negotiation policy maxima/preferences:
  - `max_incoming_receive_maximum: Option<NonZero<u16>>`
  - `max_incoming_packet_size: Option<NonZero<u32>>`
  - `max_incoming_topic_alias_maximum: Option<NonZero<u16>>`
  - `max_outgoing_qos: Option<MaximumQoS>`
  - `allow_retain: bool`
  - `allow_wildcard_subscriptions: bool`
  - `allow_shared_subscriptions: bool`
  - `allow_subscription_identifiers: bool`
  - `default_request_response_information: Option<bool>`
  - `default_request_problem_information: Option<bool>`
  - `default_keep_alive: Option<NonZero<u16>>`

Defaults remain permissive except parser maxima inherited from `ParserSettings::default()`.

## 2) `ClientScratchpad` effective limit fields

`ClientScratchpad` holds computed current effective limits used at runtime:

- Effective parser envelope:
  - `effective_client_max_bytes_string: u16`
  - `effective_client_max_bytes_binary_data: u16`
  - `effective_client_max_remaining_bytes: u64`
  - `effective_client_max_subscriptions_len: u32`
  - `effective_client_max_user_properties_len: usize`
- Effective protocol envelope:
  - `effective_receive_maximum: NonZero<u16>`
  - `effective_maximum_packet_size: Option<NonZero<u32>>`
  - `effective_topic_alias_maximum: u16`
  - `effective_maximum_qos: Option<MaximumQoS>`
  - `effective_retain_available: bool`
  - `effective_wildcard_subscription_available: bool`
  - `effective_shared_subscription_available: bool`
  - `effective_subscription_identifiers_available: bool`
  - Keepalive effective runtime fields (existing flattened keepalive fields remain)

Naming rule:
- Do not add parser/encoder prefixes where one effective value is shared.
- Use neutral names for shared limits/capabilities (`effective_maximum_packet_size`,
  `effective_topic_alias_maximum`, etc.) and route those values to both parser/protocol checks
  when applicable.
- Prefix only when audience differs and values diverge:
  - client-only value: `effective_client_*`
  - broker-only value: `effective_broker_*`

Broker-advertised raw values may also remain in scratchpad for traceability if needed,
but all behavior checks must reference effective fields.

## Effective-limit recomputation model

## 1) Recompute on every relevant change

Add internal function:

```rust
fn recompute_effective_limits(&mut self)
```

Call it on these events:

- `UserWriteIn::Connect(options)`
- `DriverEventIn::SocketConnected`
- successful `ControlPacket::ConnAck` handling
- disconnect/close/error lifecycle transitions that reset connection context

Rule: after any state mutation that can affect limits, recompute immediately.

## 2) Merge rules

- Numeric caps: minimum across all relevant sources.
- Boolean capabilities: restrictive conjunction (`local_allow && broker_allow`).
- Option sources:
  - `None` means “no further restriction from this source.”
  - Effective option present if at least one restricting source present.

Representative numeric merge:

```text
effective_receive_maximum = min(
  app.max_incoming_receive_maximum or u16::MAX,
  connect.receive_maximum or u16::MAX,
  broker.receive_maximum or u16::MAX
)
```

Representative parser merge:

```text
effective_max_remaining_bytes = min(
  app.max_remaining_bytes,
  app.max_incoming_packet_size or u64::MAX,
  connect.maximum_packet_size or u64::MAX
)
```

Note: `CONNACK.maximum_packet_size` constrains outbound packet size to broker, not inbound parser envelope.

## Parser integration

`parser_settings()` must use scratchpad effective parser fields, not raw `ClientSettings`:

```rust
ParserSettings {
    max_bytes_string: self.scratchpad.effective_client_max_bytes_string,
    max_bytes_binary_data: self.scratchpad.effective_client_max_bytes_binary_data,
    max_remaining_bytes: self.scratchpad.effective_client_max_remaining_bytes,
    max_subscriptions_len: self.scratchpad.effective_client_max_subscriptions_len,
    max_user_properties_len: self.scratchpad.effective_client_max_user_properties_len,
}
```

This keeps parser behavior aligned with currently applied negotiation envelope.

## CONNECT assembly integration

When building CONNECT:

- Use explicit `ConnectionOptions` values when provided.
- Otherwise use `ClientSettings` defaults for request fields (`default_keep_alive`,
  request info defaults).
- Clamp outgoing CONNECT-advertised limits to app policy maxima before encoding.

Example:

```text
connect.receive_maximum = min(connection.receive_maximum or u16::MAX,
                              app.max_incoming_receive_maximum or u16::MAX)
```

## Behavioral invariants

- Effective limits are always available and internally consistent after every transition.
- Runtime checks (`publish`, `subscribe`, packet-size, keepalive, alias checks)
  use effective fields only.
- Reconnection starts from app+connect baseline and re-applies broker restrictions
  when CONNACK arrives.
- Session state persistence does not carry negotiated/effective connection limits.

## Testing strategy

Required additions/updates in `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`:

1. Effective recompute on `Connect`:
   - connect options less restrictive than app still resolve to app caps.
2. Effective recompute on `ConnAck`:
   - broker-lower receive maximum/packet size/capabilities further restrict runtime behavior.
3. Parser settings alignment:
   - parser uses effective values (including max_remaining_bytes clamp).
4. Reconnect reset/recompute:
   - after socket close/disconnect, effective limits reset and recompute for next connection.
5. Capability booleans:
   - any app `allow_* = false` remains false even if broker advertises true.

Also run tokio crate tests to validate constructor/config call sites compile and behave as before.

## Verification commands

- `cargo test -p sansio-mqtt-v5-protocol --test client_protocol`
- `cargo test -p sansio-mqtt-v5-protocol`
- `cargo test -p sansio-mqtt-v5-tokio`
- `cargo test -q`
- `cargo fmt`
- `cargo clippy`
