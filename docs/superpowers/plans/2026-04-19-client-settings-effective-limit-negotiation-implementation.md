# ClientSettings Effective Limit Negotiation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand `ClientSettings` into local negotiation policy input and
implement directional effective limit recomputation so parser/protocol behavior
always uses the currently applied limits.

**Architecture:** Keep persistent session continuity in `ClientState` and move
all negotiated/effective limits to `ClientScratchpad`. Introduce one
recomputation path that runs on every relevant transition (`Connect`,
`SocketConnected`, `ConnAck`, disconnect/close/error) and computes directional
effective limits (`effective_client_*`, `effective_broker_*`, and shared
`effective_*` where truly shared). Route parser settings and runtime checks
exclusively through effective fields.

**Tech Stack:** Rust (`no_std` with `alloc`), Cargo test suite
(`client_protocol` + crate/workspace tests), tokio wrapper integration.

---

### Task 1: Extend `ClientSettings` with negotiation policy fields and defaults

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing settings shape tests first**

Add/extend tests in `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`:

```rust
#[test]
fn client_settings_default_includes_permissive_negotiation_policy() {
    let settings = ClientSettings::default();

    assert!(settings.max_incoming_receive_maximum.is_none());
    assert!(settings.max_incoming_packet_size.is_none());
    assert!(settings.max_incoming_topic_alias_maximum.is_none());
    assert!(settings.max_outgoing_qos.is_none());
    assert!(settings.allow_retain);
    assert!(settings.allow_wildcard_subscriptions);
    assert!(settings.allow_shared_subscriptions);
    assert!(settings.allow_subscription_identifiers);
    assert!(settings.default_request_response_information.is_none());
    assert!(settings.default_request_problem_information.is_none());
    assert!(settings.default_keep_alive.is_none());
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol client_settings_default_includes_permissive_negotiation_policy -- --nocapture
```

Expected: FAIL because new fields are not yet present on `ClientSettings`.

- [ ] **Step 3: Add new fields and defaults in `types.rs`**

Update `ClientSettings` in `crates/sansio-mqtt-v5-protocol/src/types.rs`:

```rust
pub struct ClientSettings {
    pub max_bytes_string: u16,
    pub max_bytes_binary_data: u16,
    pub max_remaining_bytes: u64,
    pub max_subscriptions_len: u32,
    pub max_user_properties_len: usize,
    pub max_incoming_receive_maximum: Option<NonZero<u16>>,
    pub max_incoming_packet_size: Option<NonZero<u32>>,
    pub max_incoming_topic_alias_maximum: Option<u16>,
    pub max_outgoing_qos: Option<MaximumQoS>,
    pub allow_retain: bool,
    pub allow_wildcard_subscriptions: bool,
    pub allow_shared_subscriptions: bool,
    pub allow_subscription_identifiers: bool,
    pub default_request_response_information: Option<bool>,
    pub default_request_problem_information: Option<bool>,
    pub default_keep_alive: Option<NonZero<u16>>,
}
```

Initialize new defaults in `impl Default for ClientSettings`.

- [ ] **Step 4: Run test to verify GREEN**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol client_settings_default_includes_permissive_negotiation_policy
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/sansio-mqtt-v5-protocol/src/types.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "feat(protocol): add negotiation policy fields to client settings"
```

### Task 2: Introduce directional effective limit fields on `ClientScratchpad`

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing compile/shape tests for effective directional
      fields**

Add tests that force directional behavior usage (client/broker distinction), for
example:

```rust
#[test]
fn connack_receive_maximum_only_limits_broker_facing_publish_flow() {
    // setup: app/client receive max remains permissive, broker receive max=1 from CONNACK
    // expected: second outbound qos1 publish is rejected with ReceiveMaximumExceeded,
    // while inbound parsing path remains unaffected by this broker-facing limit.
}
```

- [ ] **Step 2: Run test to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol connack_receive_maximum_only_limits_broker_facing_publish_flow -- --nocapture
```

Expected: FAIL before directional fields/logic exist.

- [ ] **Step 3: Add directional effective fields and initialize defaults**

In `ClientScratchpad` (`proto.rs`), add:

```rust
effective_client_max_bytes_string: u16,
effective_client_max_bytes_binary_data: u16,
effective_client_max_remaining_bytes: u64,
effective_client_max_subscriptions_len: u32,
effective_client_max_user_properties_len: usize,
effective_client_receive_maximum: NonZero<u16>,
effective_client_maximum_packet_size: Option<NonZero<u32>>,
effective_client_topic_alias_maximum: u16,
effective_broker_receive_maximum: NonZero<u16>,
effective_broker_maximum_packet_size: Option<NonZero<u32>>,
effective_broker_topic_alias_maximum: u16,
effective_broker_maximum_qos: Option<MaximumQoS>,
```

Initialize in `Default` with permissive values.

- [ ] **Step 4: Route current checks to directional fields (mechanical pass)**

Update existing checks in `proto.rs` to stop reading old negotiated fields
directly and use effective directional fields where applicable:

```rust
// outbound inflight capacity
self.state.on_flight_sent.len() >= usize::from(self.scratchpad.effective_broker_receive_maximum.get())

// outbound packet size
if let Some(max) = self.scratchpad.effective_broker_maximum_packet_size { ... }
```

- [ ] **Step 5: Run targeted tests to verify GREEN**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol connack_receive_maximum_only_limits_broker_facing_publish_flow
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "refactor(protocol): add directional effective limit fields"
```

### Task 3: Implement `recompute_effective_limits()` and call it on every transition

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`

- [ ] **Step 1: Write failing recomputation trigger tests**

Add tests for recomputation events:

```rust
#[test]
fn effective_limits_recompute_on_connect_socketconnected_connack_and_socketclosed() {
    // verify behavior changes after each transition in a single flow
}
```

Also add test for topic alias semantics:

```rust
#[test]
fn app_topic_alias_zero_disables_inbound_alias_even_if_connect_requests_more() {
    // ClientSettings.max_incoming_topic_alias_maximum = Some(0)
    // inbound alias publish should fail as protocol error.
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol effective_limits_recompute_on_connect_socketconnected_connack_and_socketclosed -- --nocapture
cargo test -p sansio-mqtt-v5-protocol --test client_protocol app_topic_alias_zero_disables_inbound_alias_even_if_connect_requests_more -- --nocapture
```

Expected: FAIL before recomputation hook wiring is complete.

- [ ] **Step 3: Implement `recompute_effective_limits()` in `proto.rs`**

Implement one method that recalculates effective fields from:

- `self.settings`
- `self.scratchpad.pending_connect_options`
- broker-advertised values currently in scratchpad

Use helper `min_option_u16`, `min_option_u32` style local functions where
useful.

Representative logic:

```rust
self.scratchpad.effective_client_receive_maximum = NonZero::new(
    u16::min(
        self.settings.max_incoming_receive_maximum.map(|x| x.get()).unwrap_or(u16::MAX),
        self.scratchpad.pending_connect_options.receive_maximum.map(|x| x.get()).unwrap_or(u16::MAX),
    ),
).expect("min result is non-zero for receive maximum");
```

And for broker-facing receive maximum:

```rust
self.scratchpad.effective_broker_receive_maximum =
    self.scratchpad.negotiated_receive_maximum;
```

- [ ] **Step 4: Call recomputation after every relevant state mutation**

Ensure `recompute_effective_limits()` is called in:

- `UserWriteIn::Connect` after `pending_connect_options` update
- `DriverEventIn::SocketConnected`
- successful `ConnAck` branch after negotiated fields are updated
- disconnect/close/error/reset paths after negotiated fields are reset

- [ ] **Step 5: Run targeted tests to verify GREEN**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol effective_limits_recompute_on_connect_socketconnected_connack_and_socketclosed
cargo test -p sansio-mqtt-v5-protocol --test client_protocol app_topic_alias_zero_disables_inbound_alias_even_if_connect_requests_more
```

Expected: PASS.

- [ ] **Step 6: Commit Task 3**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "feat(protocol): recompute effective limits on every negotiation transition"
```

### Task 4: Route parser settings and CONNECT defaults through effective/app policy

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`
- Modify: `crates/sansio-mqtt-v5-tokio/src/connect.rs` (if constructor callsite
  adjustments needed)

- [ ] **Step 1: Write failing parser integration tests**

Add tests:

```rust
#[test]
fn parser_uses_effective_client_limits_after_connect_policy_applied() {
    // configure app max_remaining_bytes lower than default and verify parser rejects larger packet
}

#[test]
fn connect_defaults_use_client_settings_when_connection_options_omit_values() {
    // default_keep_alive / request info defaults are encoded into CONNECT
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol parser_uses_effective_client_limits_after_connect_policy_applied -- --nocapture
cargo test -p sansio-mqtt-v5-protocol --test client_protocol connect_defaults_use_client_settings_when_connection_options_omit_values -- --nocapture
```

Expected: FAIL prior to parser/effective integration.

- [ ] **Step 3: Update parser settings and connect packet assembly**

In `proto.rs`:

```rust
fn parser_settings(&self) -> ParserSettings {
    ParserSettings {
        max_bytes_string: self.scratchpad.effective_client_max_bytes_string,
        max_bytes_binary_data: self.scratchpad.effective_client_max_bytes_binary_data,
        max_remaining_bytes: self.scratchpad.effective_client_max_remaining_bytes,
        max_subscriptions_len: self.scratchpad.effective_client_max_subscriptions_len,
        max_user_properties_len: self.scratchpad.effective_client_max_user_properties_len,
    }
}
```

In `build_connect_packet`, apply settings defaults when option fields are
`None`.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol parser_uses_effective_client_limits_after_connect_policy_applied
cargo test -p sansio-mqtt-v5-protocol --test client_protocol connect_defaults_use_client_settings_when_connection_options_omit_values
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs crates/sansio-mqtt-v5-tokio/src/connect.rs
git commit -m "feat(protocol): apply effective parser limits and connect defaults from settings"
```

### Task 5: Full regression and cleanup

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs` (if final fixes
  required)
- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs` (if final fixes
  required)
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` (if final
  fixes required)

- [ ] **Step 1: Run protocol integration suite**

```bash
cargo test -p sansio-mqtt-v5-protocol --test client_protocol
```

Expected: PASS.

- [ ] **Step 2: Run crate/workspace regression suites**

```bash
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
cargo test -q
```

Expected: PASS.

- [ ] **Step 3: Run formatting and lints**

```bash
cargo fmt
cargo clippy
```

Expected: no new warnings beyond established baseline.

- [ ] **Step 4: Confirm no stale names remain**

```bash
rg "effective_receive_maximum\b|effective_topic_alias_maximum\b" crates/sansio-mqtt-v5-protocol/src/proto.rs
```

Expected: no matches for deprecated ambiguous names.

- [ ] **Step 5: Commit final verification fixes (if needed)**

```bash
git add crates/sansio-mqtt-v5-protocol/src/proto.rs crates/sansio-mqtt-v5-protocol/src/types.rs crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs
git commit -m "test(protocol): cover directional effective limit recomputation"
```

## Spec Coverage Check

- `ClientSettings` as app policy inputs: Task 1.
- Directional effective limits and naming (`client`/`broker`): Tasks 2 and 3.
- Recompute on every relevant transition: Task 3.
- Parser configured from effective applied limits: Task 4.
- CONNECT defaults and app-clamped outgoing client-facing values: Task 4.
- Session persistence unaffected by negotiated transient limits: Task 3
  regression paths + Task 5 full suite.
