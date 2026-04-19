# Task 2A — Refactor `sansio-mqtt-v5-protocol` with `statig` (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the 1815-line `sansio-mqtt-v5-protocol/src/proto.rs` into smaller, more maintainable modules by modelling the MQTT-client state machine with the `statig` crate. Public API (`sansio::Protocol` implementation, `Client`, `ClientState`) MUST remain unchanged.

**Architecture:** Adopt `statig` with `default-features = false` (no_std-first). Replace the hand-written `ClientLifecycleState`, `ConnectingPhase`, `OutboundInflightState`, and `InboundInflightState` machines (or a subset) with `statig` state machines. Extract packet builders, validators, and queue helpers into dedicated submodules.

**Tech Stack:** Rust 2021, nightly, `statig`, existing `winnow`/`encode`/`sansio` deps.

---

## Context

- Worktree: `../sansio-mqtt-task2a-statig` on branch `task2a-proto-statig`.
- Spec: `docs/superpowers/specs/2026-04-19-parallel-docs-and-refactor-design.md`.
- Repository rules (`CLAUDE.md`): spec gate, `#![forbid(unsafe_code)]`, no_std-first, `cargo fmt`, `cargo clippy`, rust-analyzer required, atomic commits, TDD.
- This plan is one half of a bake-off. A parallel plan (`task2b-proto-refactor-plain.md`) refactors the same file without a state-machine crate. A reviewer agent will compare.

## Non-negotiable constraints

- **Public API frozen.** `sansio-mqtt-v5-protocol` re-exports (`Client`, `ClientState`) and its `sansio::Protocol` impl must be byte-identical in signature.
- **All existing tests pass unmodified.**
  - `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`
  - In-file unit tests in `proto.rs`
  - `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs`
- `#![forbid(unsafe_code)]` stays in `lib.rs`.
- `no_std` remains. `alloc` only. No `std::` imports outside `cfg(test)`.
- `cargo fmt` / `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `statig` must support no_std. If the version chosen does not, STOP and report back.

## Files map

- **Create:**
  - `crates/sansio-mqtt-v5-protocol/src/state_machine/mod.rs` — re-exports + top-level doc
  - `crates/sansio-mqtt-v5-protocol/src/state_machine/lifecycle.rs` — connection lifecycle SM (Start→Disconnected→Connecting→Connected)
  - `crates/sansio-mqtt-v5-protocol/src/state_machine/outbound.rs` — per-packet-id outbound QoS machine
  - `crates/sansio-mqtt-v5-protocol/src/state_machine/inbound.rs` — per-packet-id inbound QoS machine
  - `crates/sansio-mqtt-v5-protocol/src/packet_build.rs` — `build_connect_packet`, any other builder helpers
  - `crates/sansio-mqtt-v5-protocol/src/limits.rs` — `recompute_effective_limits`, `reset_negotiated_limits`, `validate_*_capacity`
  - `crates/sansio-mqtt-v5-protocol/src/queues.rs` — `encode_control_packet`, `enqueue_packet`, `enqueue_puback_or_fail_protocol`, etc.
  - `crates/sansio-mqtt-v5-protocol/src/client.rs` — `Client`, `ClientScratchpad`, `sansio::Protocol` impl (thinned)
- **Modify:**
  - `crates/sansio-mqtt-v5-protocol/Cargo.toml` — add `statig` dep
  - `Cargo.toml` (workspace) — add `statig` to `[workspace.dependencies]`
  - `crates/sansio-mqtt-v5-protocol/src/lib.rs` — module declarations and re-exports
- **Delete:**
  - `crates/sansio-mqtt-v5-protocol/src/proto.rs` — contents absorbed into modules above (last step)

## Target module boundaries

- `client.rs` ≤ 400 lines; contains `Client` struct, `ClientScratchpad`, `sansio::Protocol` impl, and delegates to other modules.
- Each `state_machine/*.rs` ≤ 250 lines.
- `packet_build.rs`, `limits.rs`, `queues.rs` each ≤ 300 lines.

---

## Task 1: Verify baseline and pick a `statig` version

- [ ] **Step 1: Confirm rust-analyzer**

Run: `rust-analyzer --version`

- [ ] **Step 2: Confirm clean baseline**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
All must pass.

- [ ] **Step 3: Inspect `statig`**

Run: `cargo search statig --limit 1`

Then consult `statig` docs (via `context7` MCP tool if available):
`mcp__plugin_context7_context7__resolve-library-id` with query "statig state machine rust", then `query-docs` for no_std support.

Pick the latest version that:
1. Compiles on `no_std` (explicit `default-features = false` no_std support, or a `no_std` feature).
2. Supports enum-based state.
3. Is `#![forbid(unsafe_code)]`-compatible (check that transitive crate tree does not force `unsafe` in our own code — transitive `unsafe` inside `statig` itself is acceptable).

If no suitable version exists, STOP and report to the orchestrator.

### Task 2: Add `statig` to the workspace

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/sansio-mqtt-v5-protocol/Cargo.toml`

- [ ] **Step 1: Add to workspace deps**

In workspace `Cargo.toml`, under `[workspace.dependencies]`, add:
```toml
statig = { version = "<chosen-version>", default-features = false }
```

- [ ] **Step 2: Add to crate deps**

In `crates/sansio-mqtt-v5-protocol/Cargo.toml`, under `[dependencies]`, add:
```toml
statig = { workspace = true }
```

- [ ] **Step 3: Verify the dep compiles in no_std context**

Run: `cargo build -p sansio-mqtt-v5-protocol`
Expected: success. If failures mention `std`, pick a different `statig` version or feature flags.

- [ ] **Step 4: Commit**

```
git add Cargo.toml crates/sansio-mqtt-v5-protocol/Cargo.toml Cargo.lock
git commit -m "build(v5-protocol): add statig dependency (no_std)"
```

### Task 3: Extract queue helpers into `queues.rs`

Move these from `proto.rs` without behavior change:
- `fn encode_control_packet`
- `fn enqueue_packet`
- `fn enqueue_pubrel_or_fail_protocol`
- `fn enqueue_puback_or_fail_protocol`
- `fn enqueue_pubrec_or_fail_protocol`
- `fn enqueue_pubcomp_or_fail_protocol`

- [ ] **Step 1: Create `queues.rs`** with the functions copied verbatim. They operate on `&mut self` of `Client<Time>`, so either:
  (a) Keep them as `impl<Time> Client<Time>` methods in a `mod queues` inside `client.rs`, or
  (b) Move them to free functions taking `&mut ClientScratchpad<Time>`.

Prefer (b) where feasible to reduce coupling. Otherwise keep (a).

- [ ] **Step 2: Update `proto.rs` / `client.rs` call sites** to use the new paths.

- [ ] **Step 3: Run tests**

```
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
```
Both must pass.

- [ ] **Step 4: Commit**

```
git add -A
git commit -m "refactor(v5-protocol): extract queue helpers into queues module"
```

### Task 4: Extract packet builders into `packet_build.rs`

Move `fn build_connect_packet` and any other builder helpers (e.g., the `fn min_option_nonzero_u16`/`u32` utilities if they belong).

- [ ] **Step 1: Create `packet_build.rs`**
- [ ] **Step 2: Move functions, update call sites**
- [ ] **Step 3: Run tests** — must pass.
- [ ] **Step 4: Commit**

```
git commit -m "refactor(v5-protocol): extract packet builders into packet_build module"
```

### Task 5: Extract limits/validation into `limits.rs`

Move:
- `fn recompute_effective_limits`
- `fn reset_negotiated_limits`
- `fn ensure_outbound_receive_maximum_capacity`
- `fn validate_outbound_topic_alias`
- `fn validate_outbound_packet_size`
- `fn validate_outbound_publish_capabilities`
- `fn apply_inbound_publish_topic_alias`

- [ ] **Step 1: Create module, move functions**
- [ ] **Step 2: Run tests**
- [ ] **Step 3: Commit**

### Task 6: Model lifecycle SM with `statig`

**Files:** `crates/sansio-mqtt-v5-protocol/src/state_machine/mod.rs` and `.../lifecycle.rs`.

- [ ] **Step 1: Write a failing test**

Create a unit test that drives a lifecycle SM through Start → Disconnected → Connecting (AwaitConnAck) → Connecting (AuthInProgress) → Connected → Disconnected, and asserts on allowed transitions. Place in `state_machine/lifecycle.rs` under `#[cfg(test)] mod tests`.

- [ ] **Step 2: Define the `statig` machine**

Use `statig` to define states equivalent to `ClientLifecycleState × ConnectingPhase`. Actions on transition emit side effects (e.g., `keep_alive_reset`) via callbacks that the outer `Client` wires up. The SM holds no external state; the scratchpad still owns buffers/queues.

- [ ] **Step 3: Run the test — expect pass**

- [ ] **Step 4: Replace `handle_read_control_packet` lifecycle match arms** with SM dispatch. `handle_event(SocketConnected/SocketClosed)` and `handle_timeout` too.

- [ ] **Step 5: Full test suite**

```
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
```

- [ ] **Step 6: Commit**

```
git commit -m "refactor(v5-protocol): model lifecycle state machine with statig"
```

### Task 7: Model outbound inflight SM with `statig`

One `statig` machine per in-flight outbound packet id. States: `Qos1AwaitPubAck`, `Qos2AwaitPubRec`, `Qos2AwaitPubComp`, terminal `Done`. Owning the `BTreeMap<NonZero<u16>, _>` stays in `ClientState`, but the enum becomes the `statig` state.

- [ ] **Step 1: Failing test covering each transition** (PUBACK received in each state, PUBREC in each state, PUBCOMP in each state, session-resume replay).
- [ ] **Step 2: Implement in `state_machine/outbound.rs`.**
- [ ] **Step 3: Replace match arms in `handle_read_control_packet`** and `replay_outbound_inflight_with_dup`.
- [ ] **Step 4: Full test suite.**
- [ ] **Step 5: Commit** — `refactor(v5-protocol): outbound QoS state machine via statig`.

### Task 8: Model inbound inflight SM with `statig`

States: `Qos1AwaitAppDecision`, `Qos2AwaitAppDecision`, `Qos2AwaitPubRel`, `Qos2Rejected(PubRecReasonCode)`.

- [ ] **Step 1: Failing test per transition.**
- [ ] **Step 2: Implement in `state_machine/inbound.rs`.**
- [ ] **Step 3: Replace match arms in `handle_read_control_packet` and `handle_write` (acknowledge/reject).**
- [ ] **Step 4: Full test suite.**
- [ ] **Step 5: Commit.**

### Task 9: Consolidate `client.rs`, delete `proto.rs`

- [ ] **Step 1: Ensure `client.rs` owns `Client`, `ClientScratchpad`, and the `sansio::Protocol` impl only.** All helper logic lives in `queues`, `packet_build`, `limits`, `state_machine/*`.

- [ ] **Step 2: Update `lib.rs`**

```rust
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

mod client;
mod limits;
mod packet_build;
mod queues;
mod state_machine;
mod types;

pub use client::Client;
pub use client::ClientState;
pub use types::*;
```

- [ ] **Step 3: Delete `proto.rs`** if its contents are fully moved.

- [ ] **Step 4: Full verification**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p sansio-mqtt-v5-tokio   # ensure downstream compiles unchanged
```

All must pass.

- [ ] **Step 5: Commit**

```
git commit -m "refactor(v5-protocol): finalize module split, remove proto.rs"
```

### Task 10: Measure outcome

- [ ] **Step 1: Capture metrics**

Run and record:
```
wc -l crates/sansio-mqtt-v5-protocol/src/**/*.rs
cargo clippy -p sansio-mqtt-v5-protocol -- -W clippy::cognitive_complexity 2>&1 | tail -50
```

Write results to `crates/sansio-mqtt-v5-protocol/METRICS.md` (uncommitted) for the reviewer.

- [ ] **Step 2: Report back to orchestrator**

Provide:
- Final module structure (paths + LOC).
- Largest function (file:line, LOC, cyclomatic complexity if clippy reports it).
- `sansio::Protocol` trait-impl boilerplate size.
- Any surprises or blockers encountered.

### Task 11: Handoff

- [ ] **Step 1: Ensure branch is pushed (if remote exists) or ready for local worktree access.**

No further tasks. The reviewer will read this branch directly.

---

## Self-review checklist

- [ ] `statig` added with `default-features = false`.
- [ ] `#![forbid(unsafe_code)]` still in `lib.rs`.
- [ ] `no_std` still declared; no `std::` imports (except `cfg(test)`).
- [ ] `Client`, `ClientState` publicly exported unchanged.
- [ ] `sansio::Protocol` impl unchanged in signature.
- [ ] All existing tests pass.
- [ ] `sansio-mqtt-v5-tokio` compiles unchanged.
- [ ] `proto.rs` deleted OR reduced to a thin facade (document which).
- [ ] No file > 400 LOC.
