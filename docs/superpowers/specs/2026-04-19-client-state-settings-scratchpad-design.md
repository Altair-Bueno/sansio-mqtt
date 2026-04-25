# Client State/Settings/Scratchpad Flattening Design

Date: 2026-04-19 Scope: `crates/sansio-mqtt-v5-protocol/src/proto.rs` client
internals and constructors

## Goal

Refactor `sansio_mqtt_v5_protocol::Client` into three explicit components to
support disk persistence and resumability after disconnection and shutdown:

- `ClientState`: persistent, resumable protocol state
- `ClientSettings`: external inputs provided by application
- `ClientScratchpad`: transient runtime-only state

Hard constraint:

- Existing grouping structs like `NegotiatedLimits` and `KeepAliveState` are
  removed.
- Their fields are flattened directly into one of the three target structs.

## Non-Goals

- No behavioral expansion of MQTT protocol semantics.
- No new transport/storage abstraction in this change.
- No change to wire encoding/decoding behavior.

## Decisions

## 1) Top-level Client composition

`Client<Time>` will hold exactly three components:

- `state: ClientState`
- `settings: ClientSettings`
- `scratchpad: ClientScratchpad<Time>`

No additional state container structs are introduced for grouped fields.

Small enums used by the state machine (for example connection phase enums) may
remain enums, but not aggregate structs that group fields previously held in
`Client`.

## 2) Field ownership and flattening

### 2.1 `ClientState` (persistent/resumable)

Fields moved into `ClientState`:

- `on_flight_sent: BTreeMap<NonZero<u16>, OutboundInflightState>`
- `on_flight_received: BTreeMap<NonZero<u16>, InboundInflightState>`
- `pending_subscribe: BTreeMap<NonZero<u16>, ()>`
- `pending_unsubscribe: BTreeMap<NonZero<u16>, ()>`
- `inbound_topic_aliases: BTreeMap<NonZero<u16>, Topic>`
- `next_packet_id: u16`

Rationale:

- These fields represent protocol session continuity and must survive
  reconnect/shutdown to support replay, pending ACK handling, and packet-id
  continuity.

### 2.2 `ClientSettings` (external, app-provided)

`ClientSettings` remains application-provided and includes parser/config limits
as currently defined in `types.rs`.

Rationale:

- These are externally supplied inputs, not protocol-evolving runtime state.

### 2.3 `ClientScratchpad` (transient runtime)

All non-persistent runtime and negotiated values become flattened fields on
`ClientScratchpad<Time>`:

- State machine / phases:
  - `lifecycle_state: ClientLifecycleState` (renamed from internal `ClientState`
    enum)
  - `connecting_phase: ConnectingPhase`
- Connection options and runtime persistence policy:
  - `pending_connect_options: ConnectionOptions`
  - `session_should_persist: bool`
- Negotiated broker limits/capabilities (flattened replacement for
  `NegotiatedLimits`):
  - `negotiated_receive_maximum: NonZero<u16>`
  - `negotiated_maximum_packet_size: Option<NonZero<u32>>`
  - `negotiated_topic_alias_maximum: u16`
  - `negotiated_server_keep_alive: Option<u16>`
  - `negotiated_maximum_qos: Option<MaximumQoS>`
  - `negotiated_retain_available: bool`
  - `negotiated_wildcard_subscription_available: bool`
  - `negotiated_shared_subscription_available: bool`
  - `negotiated_subscription_identifiers_available: bool`
- Keepalive tracking (flattened replacement for `KeepAliveState`):
  - `keep_alive_interval_secs: Option<NonZero<u16>>`
  - `keep_alive_saw_network_activity: bool`
  - `keep_alive_ping_outstanding: bool`
- Buffers, queues, timer:
  - `read_buffer: BytesMut`
  - `read_queue: VecDeque<UserWriteOut>`
  - `write_queue: VecDeque<Bytes>`
  - `action_queue: VecDeque<DriverEventOut>`
  - `next_timeout: Option<Time>`

Rationale:

- These values are per-connection/per-runtime scratch state and must start blank
  for every newly constructed client instance.

## 3) Construction and initialization API

### 3.1 Constructors

Introduce explicit constructors:

- `Client::new(settings: ClientSettings) -> Self`
  - uses `ClientState::default()` and `ClientScratchpad::default()`
- `Client::new_with_state(settings: ClientSettings, state: ClientState) -> Self`
  - uses supplied persisted state and `ClientScratchpad::default()`

`Default for Client<Time>` remains supported and delegates to
`Client::new(ClientSettings::default())`.

### 3.2 Driver persistence contract

Persistence behavior is external to protocol crate:

- Driver loads `ClientState` from storage (or default)
- Driver creates client via `new_with_state`
- Driver persists updated `ClientState` as desired by integration policy

This change does not mandate storage timing or backend.

## 4) Session semantics and reset rules

### 4.1 Clean Start

Confirmed requirement:

- `clean_start = true` MUST drop stored protocol state.

Rule:

- During `UserWriteIn::Connect(options)` handling, if `options.clean_start` is
  true, call `self.state.reset()` (or equivalent clear method) before
  continuing.

This aligns with [MQTT-3.1.2-4].

### 4.2 Session-expiry-driven persistence

`session_should_persist` remains runtime-derived from connect options in
scratchpad.

On disconnect/error/close transitions:

- If `session_should_persist` is false: clear `ClientState`.
- If `session_should_persist` is true: keep `ClientState` intact.

### 4.3 Scratchpad reset behavior

`ClientScratchpad` is always initialized from `Default` on client construction.
Lifecycle reset helpers update scratchpad fields only and must not erase
`ClientState` unless session rules require it.

## 5) Internal API updates

Methods currently using root fields are updated to target component ownership:

- State-machine checks use `self.scratchpad.lifecycle_state`
- Negotiated capability checks use `self.scratchpad.negotiated_*`
- Keepalive paths use `self.scratchpad.keep_alive_*`
- Queues/buffer/timer use `self.scratchpad.*`
- Inflight/pending maps and packet-id allocation use `self.state.*`

Helper reset methods are retained but rewritten to mutate component fields:

- `reset_negotiated_limits` -> resets flattened negotiated fields in scratchpad
- `reset_keepalive` -> resets flattened keepalive fields in scratchpad
- `reset_session_state` -> clears `ClientState`

## 6) Compatibility and migration notes

- Public protocol behavior remains unchanged.
- Internal naming conflict is resolved by renaming the existing lifecycle enum
  from `ClientState` to `ClientLifecycleState`.
- Existing tests that directly reference internal fields are updated to new
  locations.

## 7) Verification strategy

Required checks after refactor:

- `cargo test -p sansio-mqtt-v5-protocol --test client_protocol`
- `cargo test -p sansio-mqtt-v5-protocol`
- `cargo test -p sansio-mqtt-v5-tokio`
- `cargo test -q`
- `cargo fmt`
- `cargo clippy`

Focused behavior tests to add/update:

1. Construction:
   - `new_with_state` preserves supplied `ClientState`
   - `new` starts with default `ClientState`
   - both start with default scratchpad
2. Clean start:
   - preloaded `ClientState` is cleared on `Connect(clean_start=true)`
3. Persistence on disconnect:
   - state retained/dropped based on session expiry policy
4. Resume behavior regression:
   - QoS inflight replay/ack semantics remain unchanged
