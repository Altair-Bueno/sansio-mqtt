# Task 2 Parts 2 & 3 — Conformance comments + tracing (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for
> tracking.

**Goal:** On the winning refactor branch (2A or 2B), (Part 2) add MQTT v5
conformance comments at state-machine decision points using Appendix B codes,
then (Part 3) add `tracing` instrumentation to every relevant state-machine
function and emit structured-field events at every decision, state change, or
warning.

**Architecture:** Two sequential parts, committed separately.

**Tech Stack:** Rust 2021, nightly, `tracing` (already a dep of
`sansio-mqtt-v5-protocol`, version 0.1.41 with `attributes` feature).

---

## Context

- Worktree: the winning branch, reused (e.g., `../sansio-mqtt-task2a-statig` or
  `../sansio-mqtt-task2b-plain`), chosen by the orchestrator after the reviewer
  report.
- Spec:
  `docs/superpowers/specs/2026-04-19-parallel-docs-and-refactor-design.md`.
- Refactored module layout depends on winner; refer to that branch's layout.
- Repository rules (`CLAUDE.md`): spec gate, `#![forbid(unsafe_code)]`,
  no_std-first, `cargo fmt`, `cargo clippy`, rust-analyzer required, atomic
  commits, TDD.

## Constraints

- **No behavior changes.** All existing tests must pass unmodified.
- **Public API unchanged.**
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo build -p sansio-mqtt-v5-tokio` all clean.
- `no_std` stays. `tracing` is already no_std-compatible at
  `default-features = false`. Do not add `tracing-subscriber` to
  `sansio-mqtt-v5-protocol`.
- Do not log error payloads manually at emission sites — `#[instrument(err)]`
  covers fallible paths.
- No `format!` or `format_args!` in `tracing` calls. Use structured fields:
  `tracing::debug!(packet_id = ?id, "publish received")`.

---

## Part 2 — Conformance comments

### Task 1: Verify starting state

- [ ] **Step 1: Confirm rust-analyzer and baseline green**

```
rust-analyzer --version
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All must pass.

- [ ] **Step 2: Confirm this branch's layout**

Run: `ls crates/sansio-mqtt-v5-protocol/src/`

Record the module list. The next tasks refer to it.

### Task 2: Map Appendix B codes to state-machine decision points

Read https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html Appendix B.

For each `[MQTT-X.Y.Z-N]` code in Appendix B:

- [ ] **Step 1: Classify it** as (a) parser/encoder (skip — covered by Task 1),
      (b) protocol behavior (include), or (c) both.

- [ ] **Step 2: Locate its enforcement site** in the refactored code. Grep
      hints:
  - Lifecycle decisions → `state/lifecycle.rs` or `state_machine/lifecycle.rs`,
    plus `read/connecting.rs`.
  - QoS 1/2 flow → `state/outbound.rs` + `state/inbound.rs` (or
    `state_machine/*`), `read/connected.rs`, `write.rs`.
  - Topic aliases → `limits.rs::apply_inbound_publish_topic_alias`,
    `limits.rs::validate_outbound_topic_alias`.
  - Keep-alive → `client.rs::handle_timeout`, ping code paths.
  - Receive maximum → `limits.rs::ensure_outbound_receive_maximum_capacity`.
  - Session state on reconnect → the code path that handles
    `CONNACK.session_present`.

- [ ] **Step 3: Add a one-line comment** at the enforcement site citing the
      code:

```rust
// [MQTT-3.3.4-6] The Client MUST NOT send a PUBLISH packet with a
// Topic Alias greater than Topic Alias Maximum received in the
// CONNACK.
```

Place comments immediately above the `if` / `match` / `return Err(…)` that
enforces the rule. Prefer one comment per enforcement point; if the same code is
enforced in multiple places, repeat the comment.

### Task 3: Iterate file by file

For each source file in `crates/sansio-mqtt-v5-protocol/src/`, produce one
commit per file:

- [ ] **Step 1: Read the file.**
- [ ] **Step 2: Add conformance comments for codes that apply.**
- [ ] **Step 3: Run `cargo test --workspace` — must pass (comments are
      comments).**
- [ ] **Step 4: Commit.**

```
git commit -m "docs(v5-protocol): conformance comments in <module>"
```

### Task 4: Verify coverage

- [ ] **Step 1: Produce a coverage report**

Create (uncommitted): `crates/sansio-mqtt-v5-protocol/CONFORMANCE_COVERAGE.md`

Columns: `code | spec statement (short) | file:line where enforced | notes`.
Rows for every Appendix B code classified as (b) or (c) in Task 2 Step 1.

- [ ] **Step 2: Flag uncovered codes**

If any protocol-behavior code has no enforcement site in the codebase, add a row
with `NOT ENFORCED — requires orchestrator review`. STOP and report these to the
orchestrator before proceeding to Part 3.

### Task 5: Part 2 verification gate

- [ ] **Step 1: Run gates**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All must pass.

- [ ] **Step 2: Commit any fixups**

```
git commit -m "docs(v5-protocol): part 2 conformance final fixups"
```

---

## Part 3 — Tracing instrumentation

### Task 6: Instrument state-machine entry points

Targets (names depend on branch layout — substitute):

- `Client::handle_read`, `Client::handle_write`, `Client::handle_event`,
  `Client::handle_timeout`, `Client::close`, `Client::poll_read`,
  `Client::poll_write`, `Client::poll_event`, `Client::poll_timeout`.

- [ ] **Step 1: Add `#[tracing::instrument(skip_all, err)]` to each fallible
      entry point** (`handle_read`, `handle_write`, `handle_event`,
      `handle_timeout`, `close`).

- [ ] **Step 2: Add `#[tracing::instrument(skip_all)]` to each infallible entry
      point** (`poll_*`).

- [ ] **Step 3: Add relevant fields where easy**

For example:

```rust
#[tracing::instrument(skip_all, fields(packet_len = msg.len()), err)]
fn handle_read(&mut self, msg: Bytes) -> Result<(), Self::Error> { … }
```

- [ ] **Step 4: Run tests — must pass.**

- [ ] **Step 5: Commit.**

```
git commit -m "feat(v5-protocol): instrument sansio::Protocol entry points"
```

### Task 7: Instrument read-path handlers

For every `fn on_<packet>` under `read/connecting.rs`, `read/connected.rs`, or
the equivalent in the statig variant:

- [ ] **Step 1: Add `#[tracing::instrument(skip_all, err)]`.** If the packet has
      a packet id, add `fields(packet_id = ?packet_id)` so it appears on every
      span event.

- [ ] **Step 2: Add event logs at decision points:**
  - Emit `tracing::debug!(…, "accepted")` or `tracing::warn!(…, "rejected")`
    when the state machine accepts or rejects a packet.
  - Emit `tracing::trace!(…)` at each state transition with the from/to state as
    fields: `tracing::trace!(from = ?prev, to = ?next, "lifecycle transition")`.
  - NEVER format error contents — `instrument(err)` records them.

- [ ] **Step 3: Run tests — must pass.**

- [ ] **Step 4: Commit per module.**

```
git commit -m "feat(v5-protocol): instrument read-path handlers"
```

### Task 8: Instrument write-path handlers

Same pattern for `fn on_connect`, `fn on_publish`, `fn on_acknowledge`,
`fn on_reject`, `fn on_subscribe`, `fn on_unsubscribe`, `fn on_disconnect` (or
statig equivalents).

- [ ] **Step 1: Instrument each.**
- [ ] **Step 2: Emit events at decision points (QoS branch, topic-alias
      allocation, receive-maximum check).**
- [ ] **Step 3: Run tests, commit.**

```
git commit -m "feat(v5-protocol): instrument write-path handlers"
```

### Task 9: Instrument state-machine transition functions

For the `state/` or `state_machine/` modules:

- [ ] **Step 1: Instrument transition functions** with
      `#[tracing::instrument(skip_all)]` (or `err` if fallible).

- [ ] **Step 2: Emit `tracing::trace!` on every transition** with `from` and
      `to` fields.

- [ ] **Step 3: Emit `tracing::warn!` on every rejected transition** (invalid
      packet for state, duplicate packet id, etc.). No formatting — use fields.

- [ ] **Step 4: Run tests, commit.**

```
git commit -m "feat(v5-protocol): instrument state-machine transitions"
```

### Task 10: Skip utility functions

Do NOT add `#[instrument]` to:

- `fn encode_control_packet`, `fn enqueue_packet` (pure helpers; noisy).
- `fn min_option_nonzero_u16`, `fn min_option_nonzero_u32` (arithmetic
  utilities).
- `Default` impls, simple getters.

- [ ] **Step 1: Confirm none of the above were instrumented.** If so, remove.

### Task 11: Verification gate

- [ ] **Step 1: Run gates**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p sansio-mqtt-v5-tokio
```

All must pass.

- [ ] **Step 2: Smoke-test tracing output**

Run an example via `sansio-mqtt-v5-tokio`'s tests with
`RUST_LOG=trace cargo test -p sansio-mqtt-v5-tokio -- --nocapture 2>&1 | head -40`
(if any test exercises the protocol meaningfully). Confirm structured tracing
events appear. This is a sanity check; does not need to be asserted.

- [ ] **Step 3: Final commit**

```
git commit -m "feat(v5-protocol): tracing verification fixups"
```

### Task 12: Handoff

- [ ] **Step 1: Summarize for orchestrator**

Produce:

- Number of `#[instrument]` attributes added.
- Number of `tracing::*!` events added.
- Any uncovered Appendix B codes reported in Part 2.
- Final commit list (`git log master..HEAD --oneline`).

---

## Self-review checklist

- [ ] Every state-machine entry point and transition function instrumented.
- [ ] Fallible functions use `err`; infallible do not.
- [ ] No string formatting inside `tracing::*!` calls; fields only.
- [ ] No `tracing::error!` for errors returned by the function (the `err`
      attribute handles them).
- [ ] Recurring fields (e.g., `packet_id`) live on the `#[instrument]` macro,
      not every event.
- [ ] Utility functions NOT instrumented.
- [ ] No behavior change. All tests pass.
- [ ] No public API change. `sansio-mqtt-v5-tokio` compiles unchanged.
- [ ] CONFORMANCE_COVERAGE.md table filled; uncovered codes (if any) reported.
