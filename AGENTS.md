# AGENTS.md

## Purpose and Scope

- Current focus: MQTT v5.0; future MQTT versions are planned and should follow
  the same conventions.

## Mandatory Checklist (MUST follow)

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
- Required formatting: `cargo +nightly fmt` (nightly required for `rustfmt.toml`
  options).
- Required linting: `cargo clippy`.
- Documentation: update when API behavior or constraints change. Follow
  https://rust-lang.github.io/rfcs/1574-more-api-documentation-conventions.html
- Change atomicity: keep changes commit-ready as a single coherent unit.
- You are operating in an environment where `ast-grep` is installed. For any
  code search that requires understanding of syntax or code structure, you
  should default to using `ast-grep --lang [language] -p '<pattern>'`. Adjust
  the `--lang` flag as needed for the specific programming language. Avoid using
  text-only search tools unless a plain-text search is explicitly requested.

## Build and Tooling

- Toolchain: **stable** (pinned in `rust-toolchain.toml`).
- **rust-analyzer is REQUIRED**: at session start, verify rust-analyzer is
  available by running `rust-analyzer --version`. If the command fails, STOP all
  work and ask the user whether you should install it by running
  `rustup component add rust-analyzer`. Do NOT proceed with any code changes
  until rust-analyzer is confirmed available.
- Use the rust-analyzer LSP for all Rust intelligence (diagnostics, type info,
  go-to-definition, etc.).

## Architecture and Crates

- Crate naming convention: `sansio-mqtt-<mqtt-version>-<scope>` (crate naming).
  `sansio-mqtt-protocol` is the one version-neutral crate: it defines the public
  application API (`Command`, `Event`, `Message`, ...) and the `MqttProtocol`
  trait that every `sansio-mqtt-<version>-protocol` crate implements; those
  crates expose only `Client` and `ClientSettings`.
- Follow existing crate boundaries; add new crates using the naming convention.

## Protocol and Spec Compliance

- MQTT v5.0 spec is authoritative for current behavior.
- Cite exact conformance statements using `[MQTT-x.x.x-y]` in code or docs when
  relevant.
- Apply the same citation style for future MQTT versions.

## Error Handling and Invalid States

- Prefer types that make invalid states unrepresentable. Examples from this
  codebase: newtype wrappers with validated constructors (`Topic`, `Utf8String`
  in `sansio-mqtt-v5-types`), `NonZero<u16>` for packet identifiers, and the
  typestate pattern in the protocol state machine (`Disconnected` →
  `Connecting` → `Connected`).
- Use `thiserror` for all error types (it is a workspace dependency). Derive
  `Display` and `Error` via `#[derive(thiserror::Error)]`. Error messages MUST
  include enough context to identify which MQTT packet or operation triggered
  the error.

## Contribution Workflow

- Default execution mode: subagent-driven (use subagents for plan execution by
  default).
