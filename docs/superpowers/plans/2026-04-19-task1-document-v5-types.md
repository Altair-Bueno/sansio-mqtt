# Task 1 — Document `sansio-mqtt-v5-types` (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add spec-grounded rustdoc to every public item in `sansio-mqtt-v5-types`, enable rustdoc lints, ensure `cargo doc` is warning-free, then validate the rendered HTML against the MQTT v5.0 spec.

**Architecture:** Purely additive documentation. No public API changes. Two sequential parts on the same worktree: Part 1 (writer) then Part 2 (validator).

**Tech Stack:** Rust 2021, nightly toolchain, `cargo doc`, rustdoc lints, MQTT v5.0 spec (https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html) including Appendix B.

---

## Context

- Worktree: `../sansio-mqtt-task1-docs` on branch `task1-docs-v5-types` (created by orchestrator).
- Spec reference: `docs/superpowers/specs/2026-04-19-parallel-docs-and-refactor-design.md`.
- Repository rules (from `CLAUDE.md`): spec gate, `#![forbid(unsafe_code)]`, no_std-first, `cargo fmt`, `cargo clippy`, rust-analyzer required, atomic commits.
- Nightly toolchain (`rust-toolchain.toml` → `nightly`). Prefer stable rustdoc lints; if a nightly-only lint is used it must be gated behind `#[cfg_attr(doc, …)]` or documented as such.

## Constraints (non-negotiable)

- **No public API changes.** Every existing import in `sansio-mqtt-v5-tokio`, `sansio-mqtt-v5-protocol`, and tests must continue to resolve.
- **Additive only.** Docs, doc attributes, `#![deny(…)]` lint attributes. No refactoring, no renames, no new re-exports, no removed items.
- `cargo test --workspace` must pass unchanged.
- `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` must pass.
- `cargo doc --no-deps -p sansio-mqtt-v5-types` must succeed with zero warnings under the enabled lint profile.

## Crate surface to document

Entry: `crates/sansio-mqtt-v5-types/src/lib.rs` re-exports `types::*`, plus `EncodeError` (from `encoder/error.rs`) and `ParserSettings` (from `parser/mod.rs`).

Public surface files (under `crates/sansio-mqtt-v5-types/src/types/`):
- `mod.rs` (re-exports — needs module-level doc)
- `auth.rs`, `basic.rs`, `connack.rs`, `connect.rs`, `control_packet.rs`, `disconnect.rs`
- `pingreq.rs`, `pingresp.rs`, `properties.rs`
- `puback.rs`, `pubcomp.rs`, `publish.rs`, `pubrec.rs`, `pubrel.rs`
- `reason_code.rs`, `reserved.rs`
- `suback.rs`, `subscribe.rs`, `unsuback.rs`, `unsubscribe.rs`

Also document:
- `EncodeError` in `encoder/error.rs`.
- `ParserSettings` (publicly re-exported from `parser/mod.rs`).
- `lib.rs` crate-level docs.

## Spec mapping (non-exhaustive reference)

| Type/packet           | Spec §  | Appendix B codes to scan for             |
|-----------------------|---------|------------------------------------------|
| `Connect`             | 3.1     | MQTT-3.1.*                               |
| `ConnAck`             | 3.2     | MQTT-3.2.*                               |
| `Publish`             | 3.3     | MQTT-3.3.*                               |
| `PubAck`              | 3.4     | MQTT-3.4.*                               |
| `PubRec`              | 3.5     | MQTT-3.5.*                               |
| `PubRel`              | 3.6     | MQTT-3.6.*                               |
| `PubComp`             | 3.7     | MQTT-3.7.*                               |
| `Subscribe`           | 3.8     | MQTT-3.8.*                               |
| `SubAck`              | 3.9     | MQTT-3.9.*                               |
| `Unsubscribe`         | 3.10    | MQTT-3.10.*                              |
| `UnsubAck`            | 3.11    | MQTT-3.11.*                              |
| `PingReq`             | 3.12    | MQTT-3.12.*                              |
| `PingResp`            | 3.13    | MQTT-3.13.*                              |
| `Disconnect`          | 3.14    | MQTT-3.14.*                              |
| `Auth`                | 3.15    | MQTT-3.15.*                              |
| Properties            | 2.2.2   | MQTT-2.2.*                               |
| `Utf8String`          | 1.5.4   | MQTT-1.5.4.*                             |
| `BinaryData`          | 1.5.6   | MQTT-1.5.6.*                             |
| `Qos`                 | 4.3     | MQTT-4.3.*                               |
| Reason codes          | 2.4, §A | MQTT-2.4.*                               |

Agent MUST read Appendix B in full and identify every code relevant to each documented item.

---

## Part 1 — Writer agent

### Task 1: Verify starting state

- [ ] **Step 1: Confirm rust-analyzer is available**

Run: `rust-analyzer --version`
Expected: version string printed. If not available, STOP and ask the orchestrator.

- [ ] **Step 2: Confirm clean baseline**

Run:
```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: all three pass.

- [ ] **Step 3: Commit nothing; branch is ready**

No changes yet. Proceed.

### Task 2: Add rustdoc lint profile to the crate

**Files:**
- Modify: `crates/sansio-mqtt-v5-types/src/lib.rs`

- [ ] **Step 1: Add crate-level lint attributes and module doc**

Open `crates/sansio-mqtt-v5-types/src/lib.rs` and prepend to the top of the file (above `#![no_std]`):

```rust
//! MQTT v5.0 wire-level types, parsers, and encoders.
//!
//! This crate provides the value types of the MQTT v5.0 control packets
//! plus `winnow`-based parsers and `encode`-based encoders. It is
//! `no_std` and does not depend on any runtime.
//!
//! Spec: <https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html>.
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![deny(rustdoc::bare_urls)]
#![deny(rustdoc::redundant_explicit_links)]
```

Keep the existing `#![no_std]` and module declarations below.

- [ ] **Step 2: Build and confirm the lints fire**

Run: `cargo doc --no-deps -p sansio-mqtt-v5-types 2>&1 | head -80`

Expected: MANY `missing_docs` errors for undocumented items. This confirms the lint profile is active.

- [ ] **Step 3: Commit the lint profile**

```
git add crates/sansio-mqtt-v5-types/src/lib.rs
git commit -m "docs(v5-types): enable rustdoc lint profile and crate-level docs"
```

### Task 3: Document `types/mod.rs`

**Files:**
- Modify: `crates/sansio-mqtt-v5-types/src/types/mod.rs`

- [ ] **Step 1: Add module-level doc**

Prepend:
```rust
//! MQTT v5.0 control-packet value types.
//!
//! Each submodule models one control packet or a shared concept
//! (properties, reason codes, basic wire types). See the MQTT v5.0
//! specification, section 2 (basic wire types) and section 3 (control
//! packets), at
//! <https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html>.
```

- [ ] **Step 2: Build and verify this file no longer triggers `missing_docs`**

Run: `cargo doc --no-deps -p sansio-mqtt-v5-types 2>&1 | grep types/mod.rs | head -20`

Expected: no warnings from `types/mod.rs`.

### Task 4: Document each type module (iterative)

For EACH of the following files, follow the same iteration pattern:

`types/basic.rs`, `types/reason_code.rs`, `types/properties.rs`, `types/connect.rs`, `types/connack.rs`, `types/publish.rs`, `types/puback.rs`, `types/pubrec.rs`, `types/pubrel.rs`, `types/pubcomp.rs`, `types/subscribe.rs`, `types/suback.rs`, `types/unsubscribe.rs`, `types/unsuback.rs`, `types/disconnect.rs`, `types/auth.rs`, `types/pingreq.rs`, `types/pingresp.rs`, `types/control_packet.rs`, `types/reserved.rs`.

- [ ] **Step 1: Read the relevant spec section** for the file being documented. Record section number and applicable Appendix B codes.

- [ ] **Step 2: Document every public item in the file.** For each `pub struct`, `pub enum`, `pub type`, `pub const`, field, and variant:
  - Add a rustdoc block explaining the meaning, with a link to the spec section and citation of every applicable `[MQTT-X.Y.Z-N]` code.
  - Use `//!` for module-level, `///` for items.
  - Use `[MQTT-X.Y.Z-N]` verbatim; do not hyperlink it (it is a reference label, not a URL).
  - For section references, use full URL: `<https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901…>` or intra-doc links only to local items.

Example for `Connect` in `types/connect.rs`:

```rust
/// MQTT v5.0 `CONNECT` packet ([§3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901033)).
///
/// First packet a client sends to a server after establishing the
/// network connection; it requests the creation or resumption of an
/// MQTT session. Carries protocol version, client identifier,
/// authentication, keep-alive, will, and connection properties.
///
/// Conformance: `[MQTT-3.1.0-1]`, `[MQTT-3.1.0-2]`,
/// `[MQTT-3.1.2-1]`, `[MQTT-3.1.2-2]`, `[MQTT-3.1.2-3]`, … (complete
/// by scanning Appendix B for `MQTT-3.1.*`).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Connect {
    /// Protocol Name; MUST equal `"MQTT"` for v5.0
    /// ([MQTT-3.1.2-1]).
    pub protocol_name: Utf8String,

    /// Protocol Version; MUST be `5` for v5.0 ([MQTT-3.1.2-2]).
    pub protocol_version: u8,

    /// Clean Start flag ([§3.1.2.4](…), [MQTT-3.1.2-4],
    /// [MQTT-3.1.2-5]).
    pub clean_start: bool,

    // … document every field …
}
```

- [ ] **Step 3: Build and fix warnings for this file**

Run: `cargo doc --no-deps -p sansio-mqtt-v5-types 2>&1 | head -40`

Expected: no warnings from the file just edited. If there are, fix them before proceeding.

- [ ] **Step 4: Commit this file**

```
git add crates/sansio-mqtt-v5-types/src/types/<file>.rs
git commit -m "docs(v5-types): document <module>"
```

Repeat Task 4 for every listed file.

### Task 5: Document `EncodeError` and `ParserSettings`

**Files:**
- Modify: `crates/sansio-mqtt-v5-types/src/encoder/error.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/parser/mod.rs`

- [ ] **Step 1: Document `EncodeError`** — explain each variant, cite relevant spec sections (malformed packet MQTT-1.3.*, etc.).

- [ ] **Step 2: Document `ParserSettings`** — explain every field, default, and the limits' relationship to MQTT v5.0 spec limits (e.g., max variable-byte integer per §1.5.5).

- [ ] **Step 3: Build and verify**

Run: `cargo doc --no-deps -p sansio-mqtt-v5-types`
Expected: zero warnings.

- [ ] **Step 4: Commit**

```
git add crates/sansio-mqtt-v5-types/src/encoder/error.rs crates/sansio-mqtt-v5-types/src/parser/mod.rs
git commit -m "docs(v5-types): document EncodeError and ParserSettings"
```

### Task 6: Full verification gate

- [ ] **Step 1: Format and lint**

Run:
```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: both pass.

- [ ] **Step 2: Full doc build**

Run: `cargo doc --no-deps -p sansio-mqtt-v5-types`
Expected: zero warnings.

- [ ] **Step 3: Full test run**

Run: `cargo test --workspace`
Expected: all tests pass (same count as pre-change baseline).

- [ ] **Step 4: Confirm no public API change**

Run: `cargo public-api --manifest-path crates/sansio-mqtt-v5-types/Cargo.toml 2>/dev/null || echo "cargo-public-api not installed; skip"`

If `cargo-public-api` is installed, compare against `master`: no differences. If not installed, proceed (the test suite already enforces import stability indirectly).

- [ ] **Step 5: Final commit if any fixup is needed**

If previous tasks left unstaged fixups, commit them:
```
git add -A
git commit -m "docs(v5-types): final verification fixups"
```

### Task 7: Hand off to Part 2

- [ ] **Step 1: Record Part 1 completion message for orchestrator**

Write file: `crates/sansio-mqtt-v5-types/.part1-done` (untracked) containing:
```
Part 1 complete. cargo doc clean, tests pass, no API change.
Commits: <list commit hashes from `git log master..HEAD --oneline`>
```

The orchestrator will dispatch Part 2 on this same worktree.

---

## Part 2 — Validator agent

The validator runs on the same worktree after Part 1's commits are present.

### Task 8: Build the docs

- [ ] **Step 1: Build**

Run: `cargo doc --no-deps -p sansio-mqtt-v5-types`
Expected: success, zero warnings.

- [ ] **Step 2: Note the rendered docs path**

Docs are at `target/doc/sansio_mqtt_v5_types/index.html`. Read rendered HTML via `Read` tool (text extraction of HTML is acceptable).

### Task 9: Review every documented item against three checks

Iterate over every public item. For each:

- [ ] **Check A — Spec citation correctness:** the linked section exists and describes this item.
- [ ] **Check B — Implementation coherence:** the documented constraint matches what the parser/encoder enforces. Cross-check by reading the matching `parser/<name>.rs` and `encoder/<name>.rs` functions. Confirm the docs do not over-promise.
- [ ] **Check C — Appendix B coverage:** every `[MQTT-X.Y.Z-N]` code in Appendix B relevant to this item is cited somewhere in that item's docs.

For **trivial fixes** (typo, wrong section number, missing citation where the code is clearly applicable), fix inline and commit at end of task. For **non-trivial mismatches** (the docs claim behavior X, but the implementation does Y; or the spec mandates Z and the code does not honor it), DO NOT modify the code. Record in a report.

### Task 10: Record findings

- [ ] **Step 1: Write validation report**

Create: `crates/sansio-mqtt-v5-types/VALIDATION_REPORT.md` (uncommitted or committed per orchestrator request). Sections:

```
# Validation Report — sansio-mqtt-v5-types docs

## Summary
- Items reviewed: <N>
- Trivial fixes applied: <N>
- Non-trivial mismatches: <N>

## Trivial fixes
<list with file:line — brief description>

## Non-trivial mismatches (REQUIRE ORCHESTRATOR REVIEW)
### Mismatch 1: <title>
- Location: <file>:<line> (<item name>)
- Spec statement: <quote with [MQTT-X.Y.Z-N]>
- Observed behavior: <what parser/encoder does>
- Question for orchestrator: <decision needed>

### Mismatch 2: …
```

- [ ] **Step 2: Commit trivial fixes**

```
git add -A
git commit -m "docs(v5-types): validation pass — trivial fixes"
```

### Task 11: Final verification and handoff

- [ ] **Step 1: Re-run verification gate**

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --no-deps -p sansio-mqtt-v5-types
cargo test --workspace
```

All must pass.

- [ ] **Step 2: Report back**

Produce for the orchestrator:
- Count of items documented.
- Count of trivial fixes.
- Content of `VALIDATION_REPORT.md` non-trivial mismatches section (or "none").
- Final commit list: `git log master..HEAD --oneline`.

---

## Self-review checklist (run before declaring done)

- [ ] Every public item across the crate has a rustdoc block.
- [ ] Every documented packet links to its MQTT v5 spec section.
- [ ] Every applicable Appendix B code is cited.
- [ ] `cargo doc --no-deps -p sansio-mqtt-v5-types` emits zero warnings.
- [ ] `cargo test --workspace` passes with identical test count.
- [ ] `sansio-mqtt-v5-tokio` compiles without modification.
- [ ] No public API change.
- [ ] All commits are atomic and use conventional-commit prefixes (`docs(v5-types): …`).
