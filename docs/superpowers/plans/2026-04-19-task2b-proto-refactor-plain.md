# Task 2B — Refactor `sansio-mqtt-v5-protocol` with plain Rust (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the 1815-line `sansio-mqtt-v5-protocol/src/proto.rs` into smaller modules and helper functions using only plain Rust (enums + `match`, helper fns, module splits). No new external dependencies. Public API MUST remain unchanged.

**Architecture:** Keep the existing `ClientLifecycleState`, `ConnectingPhase`, `OutboundInflightState`, `InboundInflightState` enums; extract per-state handler functions, packet builders, limits/validation, and queue helpers into dedicated submodules. Reduce indentation depth and per-function size.

**Tech Stack:** Rust 2021, nightly, existing `winnow`/`encode`/`sansio` deps. No new crates.

---

## Context

- Worktree: `../sansio-mqtt-task2b-plain` on branch `task2b-proto-plain`.
- Spec: `docs/superpowers/specs/2026-04-19-parallel-docs-and-refactor-design.md`.
- Repository rules (`CLAUDE.md`): spec gate, `#![forbid(unsafe_code)]`, no_std-first, `cargo fmt`, `cargo clippy`, rust-analyzer required, atomic commits, TDD.
- This plan is one half of a bake-off. A parallel plan (`task2a-proto-refactor-statig.md`) uses the `statig` crate. A reviewer will compare both.

## Non-negotiable constraints

- **No new dependencies.** `Cargo.toml` untouched.
- **Public API frozen.** `Client`, `ClientState`, and `sansio::Protocol` impl signature unchanged.
- **All existing tests pass unmodified.**
- `#![forbid(unsafe_code)]`, `no_std`, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` all clean.
- `sansio-mqtt-v5-tokio` must compile unchanged.

## Files map

- **Create:**
  - `crates/sansio-mqtt-v5-protocol/src/state/mod.rs` — state-machine enums (re-exports)
  - `crates/sansio-mqtt-v5-protocol/src/state/lifecycle.rs` — `ClientLifecycleState`, `ConnectingPhase`
  - `crates/sansio-mqtt-v5-protocol/src/state/outbound.rs` — `OutboundInflightState`, transition helpers
  - `crates/sansio-mqtt-v5-protocol/src/state/inbound.rs` — `InboundInflightState`, transition helpers
  - `crates/sansio-mqtt-v5-protocol/src/read/mod.rs` — `handle_read_control_packet` dispatcher
  - `crates/sansio-mqtt-v5-protocol/src/read/connecting.rs` — packets accepted while connecting (CONNACK, AUTH, DISCONNECT)
  - `crates/sansio-mqtt-v5-protocol/src/read/connected.rs` — packets accepted while connected (PUBLISH, PUBACK, PUBREC, PUBREL, PUBCOMP, SUBACK, UNSUBACK, PINGRESP, DISCONNECT)
  - `crates/sansio-mqtt-v5-protocol/src/write.rs` — `handle_write` dispatch
  - `crates/sansio-mqtt-v5-protocol/src/packet_build.rs` — `build_connect_packet` and any builder helpers
  - `crates/sansio-mqtt-v5-protocol/src/limits.rs` — `recompute_effective_limits`, `reset_negotiated_limits`, `validate_*`
  - `crates/sansio-mqtt-v5-protocol/src/queues.rs` — encode/enqueue helpers
  - `crates/sansio-mqtt-v5-protocol/src/client.rs` — `Client`, `ClientScratchpad`, `sansio::Protocol` impl (thinned)
- **Modify:**
  - `crates/sansio-mqtt-v5-protocol/src/lib.rs` — module declarations and re-exports
- **Delete:**
  - `crates/sansio-mqtt-v5-protocol/src/proto.rs` — contents absorbed into modules above

## Target boundaries

- `client.rs` ≤ 400 lines.
- Every other file ≤ 300 lines.
- No function > 80 lines.
- Max nesting depth inside handlers ≤ 3 levels (`match` + `if` is 2).

---

## Task 1: Verify baseline

- [ ] **Step 1: Confirm rust-analyzer**

Run: `rust-analyzer --version`

- [ ] **Step 2: Confirm clean baseline**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
All must pass.

### Task 2: Extract queue helpers into `queues.rs`

Move from `proto.rs` without behavior change:
- `fn encode_control_packet`
- `fn enqueue_packet`
- `fn enqueue_pubrel_or_fail_protocol`
- `fn enqueue_puback_or_fail_protocol`
- `fn enqueue_pubrec_or_fail_protocol`
- `fn enqueue_pubcomp_or_fail_protocol`

- [ ] **Step 1: Create `queues.rs`** with a `pub(crate)` API. Prefer free functions over `impl Client`. Functions take `&mut ClientScratchpad<Time>` (for write_queue / action_queue) and relevant owned packets.

- [ ] **Step 2: Update call sites in `proto.rs`.**

- [ ] **Step 3: Run tests**

```
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
```
Both must pass.

- [ ] **Step 4: Commit**

```
git commit -m "refactor(v5-protocol): extract queue helpers into queues module"
```

### Task 3: Extract packet builders into `packet_build.rs`

- [ ] **Step 1: Move `fn build_connect_packet`** and any private utilities it uses (`min_option_nonzero_u16`, `min_option_nonzero_u32`, `min_option_maximum_qos` if appropriate).
- [ ] **Step 2: Update call sites, run tests, commit.**

```
git commit -m "refactor(v5-protocol): extract packet builders into packet_build module"
```

### Task 4: Extract limits/validation into `limits.rs`

Move:
- `fn recompute_effective_limits`
- `fn reset_negotiated_limits`
- `fn ensure_outbound_receive_maximum_capacity`
- `fn validate_outbound_topic_alias`
- `fn validate_outbound_packet_size`
- `fn validate_outbound_publish_capabilities`
- `fn apply_inbound_publish_topic_alias`

- [ ] **Step 1: Move, update call sites, run tests, commit.**

```
git commit -m "refactor(v5-protocol): extract limits and validation into limits module"
```

### Task 5: Extract state enums into `state/`

- [ ] **Step 1: Create `state/mod.rs`** with:
```rust
pub(crate) mod inbound;
pub(crate) mod lifecycle;
pub(crate) mod outbound;

pub(crate) use inbound::InboundInflightState;
pub(crate) use lifecycle::{ClientLifecycleState, ConnectingPhase};
pub(crate) use outbound::OutboundInflightState;
```

- [ ] **Step 2: Move enum definitions** plus any narrowly-scoped transition helpers. For each state enum consider a small `impl` block for operations like `is_awaiting_puback(&self) -> bool`.

- [ ] **Step 3: Update imports, run tests, commit.**

```
git commit -m "refactor(v5-protocol): move state enums into state module"
```

### Task 6: Split read-path

**Before:** `fn handle_read_control_packet` (~366 lines, lines 757-1122). Giant nested match.

**After:** dispatcher in `read/mod.rs` routes on lifecycle state to `read/connecting.rs` or `read/connected.rs`. Each file has one function per inbound packet variant.

- [ ] **Step 1: Write failing tests** for any currently-untested branch (see `#[test]` blocks in `proto.rs` at line 1640+; add more as appropriate when splitting out code uncovers silent branches).

- [ ] **Step 2: Create `read/mod.rs`** with:
```rust
pub(crate) mod connecting;
pub(crate) mod connected;

pub(crate) fn handle_read_control_packet<Time>(
    client: &mut Client<Time>,
    packet: ControlPacket,
) -> Result<(), Error> {
    match client.scratchpad.lifecycle_state {
        ClientLifecycleState::Connecting => connecting::handle(client, packet),
        ClientLifecycleState::Connected => connected::handle(client, packet),
        ClientLifecycleState::Start | ClientLifecycleState::Disconnected => {
            // [MQTT-4.1.0-1] and related: packet arrival outside a session
            // is a protocol error.
            Err(Error::InvalidStateTransition)
        }
    }
}
```

- [ ] **Step 3: Populate `connecting.rs`** with the CONNACK/AUTH/DISCONNECT branches moved from `proto.rs`. One function per packet variant: `fn on_connack`, `fn on_auth`, `fn on_disconnect`.

- [ ] **Step 4: Populate `connected.rs`** with the remaining packet variants.

- [ ] **Step 5: Replace the original `handle_read_control_packet` method** in `Client` with a one-line delegate to `crate::read::handle_read_control_packet(self, packet)`.

- [ ] **Step 6: Run tests**

```
cargo test --workspace
```
All must pass.

- [ ] **Step 7: Commit**

```
git commit -m "refactor(v5-protocol): split read-path into connecting/connected modules"
```

### Task 7: Split write-path

`fn handle_write` is ~274 lines (1231-1504).

- [ ] **Step 1: Create `write.rs`** with:
```rust
pub(crate) fn handle_write<Time>(
    client: &mut Client<Time>,
    msg: UserWriteIn,
) -> Result<(), Error> {
    match msg {
        UserWriteIn::Connect(opts) => on_connect(client, opts),
        UserWriteIn::PublishMessage(m) => on_publish(client, m),
        UserWriteIn::AcknowledgeMessage(id) => on_acknowledge(client, id),
        UserWriteIn::RejectMessage(id, r) => on_reject(client, id, r),
        UserWriteIn::Subscribe(o) => on_subscribe(client, o),
        UserWriteIn::Unsubscribe(o) => on_unsubscribe(client, o),
        UserWriteIn::Disconnect => on_disconnect(client),
    }
}
```

- [ ] **Step 2: Extract each `on_*` function** with its current body.

- [ ] **Step 3: Replace `handle_write` in the trait impl** with a delegate.

- [ ] **Step 4: Run tests, commit.**

```
git commit -m "refactor(v5-protocol): split write-path into per-message functions"
```

### Task 8: Finalize `client.rs`, delete `proto.rs`

- [ ] **Step 1: Move remaining `Client`/`ClientScratchpad`/`sansio::Protocol` impl into `client.rs`.**

The trait impl body should be thin — each method a 1-3 line delegate to the corresponding module function.

- [ ] **Step 2: Update `lib.rs`**

```rust
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

mod client;
mod limits;
mod packet_build;
mod queues;
mod read;
mod state;
mod types;
mod write;

pub use client::Client;
pub use client::ClientState;
pub use types::*;
```

- [ ] **Step 3: Delete `proto.rs`.**

- [ ] **Step 4: Full verification**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p sansio-mqtt-v5-tokio
```

All must pass.

- [ ] **Step 5: Commit**

```
git commit -m "refactor(v5-protocol): finalize module split, remove proto.rs"
```

### Task 9: Measure outcome

- [ ] **Step 1: Capture metrics**

Run and record in `crates/sansio-mqtt-v5-protocol/METRICS.md` (uncommitted, for reviewer):

```
wc -l crates/sansio-mqtt-v5-protocol/src/**/*.rs
cargo clippy -p sansio-mqtt-v5-protocol -- -W clippy::cognitive_complexity 2>&1 | tail -50
```

Also record:
- Number of files created / deleted.
- Largest function (file:line, LOC).
- Max indentation depth observed.

- [ ] **Step 2: Report back to orchestrator**

Provide:
- Final module structure (paths + LOC).
- Largest function metrics.
- Any refactor bugs caught by tests during the process.

### Task 10: Handoff

- [ ] No further tasks. Reviewer agent will read this branch directly.

---

## Self-review checklist

- [ ] No new dependencies added.
- [ ] `#![forbid(unsafe_code)]` still in `lib.rs`.
- [ ] `no_std` still declared; no `std::` imports (except `cfg(test)`).
- [ ] `Client`, `ClientState` publicly exported unchanged.
- [ ] `sansio::Protocol` impl unchanged in signature.
- [ ] All existing tests pass.
- [ ] `sansio-mqtt-v5-tokio` compiles unchanged.
- [ ] `proto.rs` deleted.
- [ ] No file > 400 LOC, no function > 80 LOC.
