# AGENTS.md

## Index

- [Purpose and Scope](#purpose-and-scope)
- [Mandatory Checklist (MUST follow)](#mandatory-checklist-must-follow)
- [Build and Tooling](#build-and-tooling)
- [Architecture and Crates](#architecture-and-crates)
- [Protocol and Spec Compliance](#protocol-and-spec-compliance)
- [Error Handling and Invalid States](#error-handling-and-invalid-states)
- [Contribution Workflow](#contribution-workflow)
- [Key Paths](#key-paths)

## Purpose and Scope

- This repo implements sansio MQTT protocol components in Rust.
- Current focus: MQTT v5.0; future MQTT versions are planned and should follow
  the same conventions.

## Mandatory Checklist (MUST follow)

- Follow andrej-karpathy-skills skill for all code changes
- Spec gate: MUST read the MQTT v5.0 spec before implementing anything:
  https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html
- TDD: for behavior changes, add/adjust tests before implementation when
  feasible; run relevant tests after. Prefer integration tests in `tests/` over
  inline `#[cfg(test)]` unit tests; unit tests for private functions are
  optional. All behavioral changes MUST have at least one test that would fail
  before the change and pass after.
- Type safety: encode invariants in types; avoid runtime checks when types can
  enforce.
- Prefer `#![forbid(unsafe_code)]` by default. `unsafe` is allowed only when:
  - Providing a `Type::*_unchecked` public function (e.g. `new_unchecked` that
    skips invariant validation).
  - Calling a `Type::*_unchecked` public function instead of the validated
    constructor, to make unsafe type construction easy to track.
  - Create `*_unchecked` variants only when callers have already validated
    invariants or when the validated constructor has measurable overhead in a
    hot path. Every `unsafe` block MUST include a `// SAFETY:` comment stating
    the invariant the caller is required to uphold, not merely what the code
    does.
- no_std-first: use `alloc` only when required.
- Required formatting: `cargo fmt`.
- Required linting: `cargo clippy`.
- Documentation: update when API behavior or constraints change. Follow
  https://rust-lang.github.io/rfcs/1574-more-api-documentation-conventions.html
- Change atomicity: keep changes commit-ready as a single coherent unit.

## Build and Tooling

- Build system: Cargo.
- Tests: for behavior changes follow the TDD checklist item; for non-behavior
  changes they are optional.
- **rust-analyzer is REQUIRED**: at session start, verify rust-analyzer is
  available by running `rust-analyzer --version`. If the command fails, STOP all
  work and ask the user whether you should install it by running
  `rustup component add rust-analyzer`. Do NOT proceed with any code changes
  until rust-analyzer is confirmed available.
- Use the rust-analyzer LSP for all Rust intelligence (diagnostics, type info,
  go-to-definition, etc.).

## Architecture and Crates

- Workspace layout: `crates/*`.
- Crate naming convention: `sansio-mqtt-<mqtt-version>-<scope>` (crate naming).
- Follow existing crate boundaries; add new crates using the naming convention.

## Protocol and Spec Compliance

- MQTT v5.0 spec is authoritative for current behavior.
- Spec gate applies (see checklist item).
- Cite exact conformance statements using `[MQTT-x.x.x-y]` in code or docs when
  relevant.
- Apply the same citation style for future MQTT versions.

## Error Handling and Invalid States

- Prefer types that make invalid states unrepresentable. Examples from this
  codebase: newtype wrappers with validated constructors (`Topic`, `Utf8String`
  in `sansio-mqtt-v5-types`), `NonZero<u16>` for packet identifiers, and the
  typestate pattern in the protocol state machine (`Start` → `Connecting` →
  `Connected`).
- Use `thiserror` for all error types (it is a workspace dependency). Derive
  `Display` and `Error` via `#[derive(thiserror::Error)]`. Error messages MUST
  include enough context to identify which MQTT packet or operation triggered
  the error.

## Contribution Workflow

- Keep changes minimal and atomic (see checklist).
- Run required checks from the checklist before completion.
- Default execution mode: subagent-driven (use subagents for plan execution by
  default).

## Key Paths

- `AGENTS.md` (repo root)
- `Cargo.toml`
- `crates/*`
- `rust-toolchain.toml`
