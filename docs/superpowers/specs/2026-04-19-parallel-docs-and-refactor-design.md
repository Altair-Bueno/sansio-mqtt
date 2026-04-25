# Parallel Docs + Protocol Refactor — Design

**Date:** 2026-04-19 **Status:** Approved (pending user review of this document)

## Goal

Run two independent bodies of work in parallel, each in its own git worktree,
coordinated by the main session as orchestrator:

1. **Task 1** — Add complete, spec-grounded rustdoc to every public item of
   `sansio-mqtt-v5-types`, then validate it.
2. **Task 2** — Refactor the monolithic `sansio-mqtt-v5-protocol/src/proto.rs`
   (1815 lines) into something maintainable, add normative comments, and add
   `tracing` instrumentation. The refactor approach is decided by a two-agent
   bake-off (with `statig` vs plain Rust) followed by a reviewer agent.

Both tasks must respect the repository's `CLAUDE.md` mandatory checklist: spec
gate, TDD where feasible, `#![forbid(unsafe_code)]`, no_std-first, `cargo fmt`,
`cargo clippy`, documentation currency, atomic commits, rust-analyzer-backed
Rust intelligence.

## Non-goals

- Task 1 makes no public-API changes. `sansio-mqtt-v5-tokio` and the existing
  test suites must continue to pass unmodified.
- Task 2 does not change MQTT v5 parser/encoder semantics or public API.
- No unrelated refactoring, no new features.

## Architecture

The main Claude session is the orchestrator. It:

1. Commits this spec and the plan files to `master` up front.
2. Creates three git worktrees branched from `master`.
3. Dispatches subagents via the `Agent` tool, one per worktree, running in
   parallel.
4. Receives reports at checkpoints; surfaces decisions to the user.
5. Continues Task 2 Parts 2 and 3 sequentially on the winning branch after the
   bake-off review.

### Execution topology

```
master
  ├── task1-docs-v5-types            Task 1 Part 1  →  Part 2         (same worktree, sequential)
  ├── task2a-proto-statig            Task 2 Part 1 variant A (statig)  (stops after Part 1)
  └── task2b-proto-plain             Task 2 Part 1 variant B (plain)   (stops after Part 1)
                                           ↓
                                 Reviewer agent: read both branches, produce a written
                                 report (LOC, module count, complexity, clarity, public-API
                                 surface delta, clippy/fmt cleanliness, recommendation).
                                 No diffs. Stops.
                                           ↓
                                 Orchestrator + user pick winner.
                                           ↓
                                 Winner branch:  Part 2 (conformance) → Part 3 (tracing)
```

## Task 1 — Document `sansio-mqtt-v5-types`

Crate path: `crates/sansio-mqtt-v5-types/`. Entry points: `lib.rs` re-exports
`types::*`; public surface lives under `src/types/*.rs`, plus `EncodeError` from
`encoder/error.rs` and `ParserSettings` from `parser/*`.

### Part 1 — Writer agent (sequential, runs first on `task1-docs-v5-types`)

- Read the MQTT v5.0 spec
  (https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html) and **Appendix B —
  Mandatory normative statements** before writing.
- Document every public item: modules, types, fields, variants, trait items,
  constants, re-exports. For each packet or property type, link to the relevant
  spec section (e.g., CONNECT → §3.1) and cite every applicable `[MQTT-X.Y.Z-N]`
  code.
- Enable the rustdoc lints documented at
  https://doc.rust-lang.org/rustdoc/lints.html on the crate (at minimum:
  `missing_docs`, `broken_intra_doc_links`, `private_intra_doc_links`,
  `invalid_rust_codeblocks`, `bare_urls`, `redundant_explicit_links`). Lint
  gate: all enabled lints must pass as `deny`.
- Build documentation: `cargo doc --no-deps -p sansio-mqtt-v5-types` must
  complete without warnings under the chosen lint profile.
- **Additive only.** No public API changes. Verification gate:
  `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
  `cargo fmt --check` must all pass before handing off.

### Part 2 — Validator agent (sequential, runs after Part 1 on the same worktree)

- Run `cargo doc --no-deps -p sansio-mqtt-v5-types` and review the rendered HTML
  under `target/doc/sansio_mqtt_v5_types/` — **not** the source files.
- For every documented item, verify:
  - **Spec citation correctness** — section link resolves, matches the item.
  - **Implementation coherence** — documented constraint matches what
    parser/encoder actually enforce (cross-check source during this step only,
    to confirm claims).
  - **Appendix B coverage** — every `[MQTT-X.Y.Z-N]` code relevant to the item
    is cited somewhere in that item's docs.
- Apply trivial fixes inline (typos, missing citations, wrong section numbers).
- For non-trivial mismatches (spec vs. implementation divergence), stop
  modifying and produce a report to the orchestrator listing each mismatch with
  location, spec statement, and observed behavior.

## Task 2 — Refactor `sansio-mqtt-v5-protocol`

Crate path: `crates/sansio-mqtt-v5-protocol/`. Target: `src/proto.rs` (1815
lines). `types.rs` and `lib.rs` are only touched as needed for module splits.

### Part 1 bake-off — two parallel agents

Both agents must keep the public API and tests green
(`cargo test -p sansio-mqtt-v5-protocol`, plus `sansio-mqtt-v5-tokio`
unchanged). Both must satisfy `cargo clippy` and `cargo fmt`. Neither touches
Task 1 files.

- **Agent 2A — `statig`-based.** Add `statig` to the workspace `Cargo.toml` with
  `default-features = false` (the user's explicit constraint). Model the
  protocol state machine using `statig`. Split `proto.rs` into smaller modules
  reflecting the state machine's structure. no_std-first;
  `#![forbid(unsafe_code)]` remains in force.
- **Agent 2B — plain Rust.** No new dependencies. Refactor into helper
  functions, smaller modules, enums + `match`, and narrow interfaces. Goal:
  reduce indentation depth and per-function size.

### Part 1 reviewer — comparison agent

Reads both branches directly. Produces a concise written report comparing:

- Lines of code, module count, largest-function size.
- Complexity signals (`cargo clippy` warnings, `cognitive_complexity` lint,
  depth of nesting).
- Clarity and maintainability, judged against MQTT v5 state-machine semantics.
- Public-API surface delta vs. `master` (should be zero).
- `cargo fmt` / `cargo clippy` cleanliness.
- Recommendation: 2A or 2B, with reasoning.

Output: report only. No diffs, no code changes. Orchestrator + user pick the
winner.

### Part 2 — Conformance comments (runs on winner, sequential)

- Read MQTT v5.0 spec + Appendix B.
- Add `[MQTT-X.Y.Z-N]` conformance comments at every state-machine decision
  point in the refactored module(s). Parser/encoder-only codes are skipped (Task
  1 covers those).
- Preserve all behavior. `cargo test` must remain green.

### Part 3 — Tracing instrumentation (runs on winner, sequential, after Part 2)

- Add `tracing` as a workspace dependency with `default-features = false`
  (no_std implications: confirm `tracing` core is no_std-compatible; if not,
  gate behind an opt-in feature).
- Apply `#[tracing::instrument(skip_all)]` to every relevant (non-utility)
  function on the state machine. Use `err` for fallible functions; use
  `fields(packet_id = …)` style for relevant parameters.
- At each decision point, state change, or warning condition, emit
  `tracing::<level>!(fields, "message")` — structured fields, not formatted
  strings. Recurring fields belong on the `instrument` macro rather than at each
  call site.
- Do not log errors at emission sites; the `err` attribute covers them.

## Checkpoints requiring user input

1. **Spec review** — user reviews this design document before the plans are
   produced.
2. **Task 1 Part 2 completes** — orchestrator surfaces any non-trivial spec vs.
   implementation mismatches.
3. **Task 2 reviewer completes** — user picks 2A or 2B.
4. **Task 2 Part 3 completes** — final review before merging both branches back
   to `master`.

## Artifacts

This design and the plans below are committed to `master` before any subagent is
dispatched:

- `docs/superpowers/specs/2026-04-19-parallel-docs-and-refactor-design.md` (this
  file)
- `docs/superpowers/plans/2026-04-19-task1-document-v5-types.md`
- `docs/superpowers/plans/2026-04-19-task2a-proto-refactor-statig.md`
- `docs/superpowers/plans/2026-04-19-task2b-proto-refactor-plain.md`
- `docs/superpowers/plans/2026-04-19-task2-reviewer.md`
- `docs/superpowers/plans/2026-04-19-task2-conformance-and-tracing.md` (Parts
  2 + 3, winner only)

## Constraints recap (all tasks)

- Follow `CLAUDE.md`. rust-analyzer must be used for Rust intelligence.
- `#![forbid(unsafe_code)]`.
- no_std-first; use `alloc` only where required.
- `cargo fmt` + `cargo clippy` clean before handoff.
- Spec gate: read the MQTT v5.0 spec before implementing.
- Atomic, commit-ready changes.
- Public API stability: no breaking changes in either task.
