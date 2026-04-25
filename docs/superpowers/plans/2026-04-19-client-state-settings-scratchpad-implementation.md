# Client State/Settings/Scratchpad Flattening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `sansio_mqtt_v5_protocol::Client` into `ClientState`,
`ClientSettings`, and `ClientScratchpad` with flattened fields, preserving
behavior while enabling external persistence and resumability.

**Architecture:** Perform a behavior-preserving structural refactor in small
slices: first add the three new structs and constructors, then move transient
and persistent fields with flattened layout, then rewire reset/transition logic
(`clean_start`, session expiry, disconnect paths). Keep protocol outputs and
packet handling unchanged, and validate with focused regression tests.

**Tech Stack:** Rust, Cargo, sansio protocol state machine, integration tests in
`client_protocol` and tokio bridge tests.

---

### Task 1: Introduce flattened top-level composition and constructors

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Test: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing constructor/shape tests first**

Add tests in `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` for:

```rust
#[test]
fn client_new_uses_default_state_and_blank_scratchpad() {
    let _client = sansio_mqtt_v5_protocol::Client::<u64>::new(
        sansio_mqtt_v5_protocol::ClientSettings::default(),
    );
}

#[test]
fn client_new_with_state_accepts_preloaded_state() {
    let state = sansio_mqtt_v5_protocol::ClientState::default();
    let _client = sansio_mqtt_v5_protocol::Client::<u64>::new_with_state(
        sansio_mqtt_v5_protocol::ClientSettings::default(),
        state,
    );
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol client_new_uses_default_state_and_blank_scratchpad -- --nocapture
```

Expected: FAIL because `Client::new`, `Client::new_with_state`, and public
`ClientState` do not exist yet.

- [ ] **Step 3: Add flattened structs and constructors in `proto.rs`**

Create new structs (with flattened fields, no
`NegotiatedLimits`/`KeepAliveState`):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ClientState {
    pub(crate) on_flight_sent: BTreeMap<NonZero<u16>, OutboundInflightState>,
    pub(crate) on_flight_received: BTreeMap<NonZero<u16>, InboundInflightState>,
    pub(crate) pending_subscribe: BTreeMap<NonZero<u16>, ()>,
    pub(crate) pending_unsubscribe: BTreeMap<NonZero<u16>, ()>,
    pub(crate) inbound_topic_aliases: BTreeMap<NonZero<u16>, Topic>,
    pub(crate) next_packet_id: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientScratchpad<Time> {
    pub(crate) lifecycle_state: ClientLifecycleState,
    pub(crate) connecting_phase: ConnectingPhase,
    pub(crate) pending_connect_options: ConnectionOptions,
    pub(crate) session_should_persist: bool,
    pub(crate) negotiated_receive_maximum: NonZero<u16>,
    pub(crate) negotiated_maximum_packet_size: Option<NonZero<u32>>,
    pub(crate) negotiated_topic_alias_maximum: u16,
    pub(crate) negotiated_server_keep_alive: Option<u16>,
    pub(crate) negotiated_maximum_qos: Option<MaximumQoS>,
    pub(crate) negotiated_retain_available: bool,
    pub(crate) negotiated_wildcard_subscription_available: bool,
    pub(crate) negotiated_shared_subscription_available: bool,
    pub(crate) negotiated_subscription_identifiers_available: bool,
    pub(crate) keep_alive_interval_secs: Option<NonZero<u16>>,
    pub(crate) keep_alive_saw_network_activity: bool,
    pub(crate) keep_alive_ping_outstanding: bool,
    pub(crate) read_buffer: BytesMut,
    pub(crate) read_queue: VecDeque<UserWriteOut>,
    pub(crate) write_queue: VecDeque<Bytes>,
    pub(crate) action_queue: VecDeque<DriverEventOut>,
    pub(crate) next_timeout: Option<Time>,
}

impl<Time> Client<Time> {
    pub fn new(settings: ClientSettings) -> Self { /* ... */ }
    pub fn new_with_state(settings: ClientSettings, state: ClientState) -> Self { /* ... */ }
}
```

Update `Client<Time>` to contain only:

```rust
pub struct Client<Time> {
    settings: ClientSettings,
    state: ClientState,
    scratchpad: ClientScratchpad<Time>,
}
```

Rename old internal enum `ClientState` to `ClientLifecycleState`.

- [ ] **Step 4: Run targeted tests to verify GREEN for constructors**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol client_new_uses_default_state_and_blank_scratchpad
cargo test -p sansio-mqtt-v5-protocol --test client_protocol client_new_with_state_accepts_preloaded_state
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "refactor(protocol): compose client from state settings and scratchpad"
```

### Task 2: Flatten transient negotiated/keepalive fields into `ClientScratchpad`

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Test: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing regression tests for negotiated-limit and
      keepalive behavior**

Use these existing behavior tests to prove unchanged semantics:

```rust
#[test]
fn keepalive_timeout_without_pingresp_closes_connection() {}

#[test]
fn connect_encodes_receive_maximum_when_configured() {}
```

Add this concrete test if not present:

```rust
#[test]
fn connack_server_keep_alive_zero_disables_keepalive_without_panic() {
    let mut client = Client::<u64>::default();
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write();

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            server_keep_alive: Some(0),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
    assert_eq!(client.handle_timeout(1), Ok(()));
    assert!(matches!(client.poll_write(), None));
}
```

- [ ] **Step 2: Run targeted tests to verify RED after temporary removal of old
      structs**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol keepalive_timeout_without_pingresp_closes_connection connect_encodes_receive_maximum_when_configured -- --nocapture
```

Expected: FAIL while references still point to `NegotiatedLimits` /
`KeepAliveState`.

- [ ] **Step 3: Remove `NegotiatedLimits` and `KeepAliveState`, rewrite
      accesses**

In `proto.rs`:

```rust
// before
self.negotiated_limits.receive_maximum
self.keep_alive.ping_outstanding

// after
self.scratchpad.negotiated_receive_maximum
self.scratchpad.keep_alive_ping_outstanding
```

Rewrite these helpers to operate on flattened scratchpad fields:

- `reset_negotiated_limits`
- `reset_keepalive`
- parser/timeout/network-activity updates

- [ ] **Step 4: Run targeted tests to verify GREEN**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol keepalive_timeout_without_pingresp_closes_connection
cargo test -p sansio-mqtt-v5-protocol --test client_protocol connect_encodes_receive_maximum_when_configured
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "refactor(protocol): flatten negotiated and keepalive runtime fields"
```

### Task 3: Move resumable maps and packet-id tracking into `ClientState`

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Test: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing persistence-boundary tests first**

Run and keep these existing tests green while rewiring ownership:

```rust
#[test]
fn resumed_session_replays_outbound_qos_publish_with_dup_set() {}

#[test]
fn non_resumed_session_drops_inflight_and_emits_publish_dropped_events() {}
```

Add this concrete constructor test to verify `new_with_state` accepts persisted
state:

```rust
#[test]
fn client_new_with_state_accepts_non_default_packet_id_seed() {
    let mut state = sansio_mqtt_v5_protocol::ClientState::default();
    state.next_packet_id = 10;

    let mut client = sansio_mqtt_v5_protocol::Client::<u64>::new_with_state(
        sansio_mqtt_v5_protocol::ClientSettings::default(),
        state,
    );

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write();
    let packet_id = client.next_packet_id_checked().expect("packet id allocated");
    assert_eq!(packet_id.get(), 10);
}
```

- [ ] **Step 2: Run targeted tests to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol resumed_session_replays_outbound_qos_publish_with_dup_set non_resumed_session_drops_inflight_and_emits_publish_dropped_events -- --nocapture
```

Expected: FAIL until map/packet-id ownership is redirected to `self.state`.

- [ ] **Step 3: Rewire persistent ownership to `self.state`**

In `proto.rs`, move all accesses:

```rust
// before
self.on_flight_sent
self.on_flight_received
self.pending_subscribe
self.pending_unsubscribe
self.inbound_topic_aliases
self.next_packet_id

// after
self.state.on_flight_sent
self.state.on_flight_received
self.state.pending_subscribe
self.state.pending_unsubscribe
self.state.inbound_topic_aliases
self.state.next_packet_id
```

Ensure helper methods (`next_packet_id`, inflight replay/reset,
subscribe/unsubscribe tracking) read/write only through `self.state`.

- [ ] **Step 4: Run targeted tests to verify GREEN**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol resumed_session_replays_outbound_qos_publish_with_dup_set
cargo test -p sansio-mqtt-v5-protocol --test client_protocol non_resumed_session_drops_inflight_and_emits_publish_dropped_events
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "refactor(protocol): move resumable session data into client state"
```

### Task 4: Enforce clean-start/session-expiry rules on the new ownership boundaries

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Test: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing tests for clean-start persistence behavior**

Add tests:

```rust
#[test]
fn clean_start_true_drops_preloaded_state() {
    let mut state = sansio_mqtt_v5_protocol::ClientState::default();
    state.pending_subscribe.insert(NonZero::new(7).expect("non-zero"), ());

    let mut client = sansio_mqtt_v5_protocol::Client::<u64>::new_with_state(
        sansio_mqtt_v5_protocol::ClientSettings::default(),
        state,
    );

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            clean_start: true,
            ..ConnectionOptions::default()
        })),
        Ok(())
    );

    assert!(client.state.pending_subscribe.is_empty());
}

#[test]
fn clean_start_false_keeps_preloaded_state_until_session_rules_clear_it() {
    let mut state = sansio_mqtt_v5_protocol::ClientState::default();
    state.pending_subscribe.insert(NonZero::new(9).expect("non-zero"), ());

    let mut client = sansio_mqtt_v5_protocol::Client::<u64>::new_with_state(
        sansio_mqtt_v5_protocol::ClientSettings::default(),
        state,
    );

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            clean_start: false,
            session_expiry_interval: Some(60),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );

    assert!(!client.state.pending_subscribe.is_empty());
}
```

- [ ] **Step 2: Run targeted tests to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol clean_start_true_drops_preloaded_state clean_start_false_keeps_preloaded_state_until_session_rules_clear_it -- --nocapture
```

Expected: FAIL before reset rules are fully rewired.

- [ ] **Step 3: Implement reset policy using `self.state` and
      `self.scratchpad`**

In `handle_write(UserWriteIn::Connect(options))`:

```rust
if options.clean_start {
    self.state = ClientState::default();
}
self.scratchpad.session_should_persist = options.session_expiry_interval.unwrap_or(0) > 0;
self.scratchpad.pending_connect_options = options;
```

In disconnect/error/close paths:

```rust
if !self.scratchpad.session_should_persist {
    self.state = ClientState::default();
}
```

Do not clear `self.state` in paths where session should persist.

- [ ] **Step 4: Run targeted tests to verify GREEN**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol clean_start_true_drops_preloaded_state
cargo test -p sansio-mqtt-v5-protocol --test client_protocol clean_start_false_keeps_preloaded_state_until_session_rules_clear_it
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "fix(protocol): enforce clean-start state drop and session persistence boundaries"
```

### Task 5: Full regression verification and cleanup

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs` (only if final fixes
  required)
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` (only if
  final fixes required)

- [ ] **Step 1: Run full protocol and workspace validation**

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
cargo test -q
```

Expected: PASS.

- [ ] **Step 2: Run formatting and linting**

```bash
cargo fmt
cargo clippy
```

Expected: No new warnings beyond existing baseline warnings.

- [ ] **Step 3: Confirm no banned grouping structs remain**

```bash
rg "struct NegotiatedLimits|struct KeepAliveState" crates/sansio-mqtt-v5-protocol/src/proto.rs
```

Expected: no matches.

- [ ] **Step 4: Commit final polish (if needed)**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "test(protocol): cover flattened client state settings scratchpad split"
```

## Spec Coverage Check

- `Client` composed from exactly three components: covered in Task 1.
- `NegotiatedLimits` / `KeepAliveState` removed and flattened: covered in
  Task 2.
- Persistent fields moved to `ClientState`: covered in Task 3.
- `clean_start=true` drops loaded state: covered in Task 4.
- Scratchpad default-on-create and runtime reset semantics: covered in Tasks 1,
  2, and 4.
- Regression preservation of existing protocol behavior: covered across Tasks
  2-5.
