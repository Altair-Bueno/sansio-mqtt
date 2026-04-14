# AGENTS.md

## Purpose and Scope
- This repo implements sansio MQTT protocol components in Rust.
- Current focus: MQTT v5.0. Future MQTT versions are planned.
- Default stance: no_std-first; `alloc` allowed when required.

## Mandatory Checklist (MUST follow)
- TDD: add/adjust tests before implementation where feasible; run tests after.
- Type safety: encode invariants in types; avoid runtime checks when types can enforce.
- No `unsafe`: enforce with `#![forbid(unsafe_code)]`.
- no_std-first: use `alloc` only when required.
- Documentation: update when API behavior or constraints change.
- Change atomicity: keep changes commit-ready as a single coherent unit.

## Build and Tooling
- Build system: Cargo.
- Formatting is required: `cargo fmt`.
- Linting is required: `cargo clippy`.
- Tests are optional unless required for the change; use TDD when feasible.
- Prefer editor LSP for Rust; if not configured, prompt the user to enable it.

## Architecture and Crates
- Workspace layout: `crates/*`.
- Crate naming convention: `sansio-mqtt-<version>-<scope>`.
- Follow existing crate boundaries; add new crates using the naming convention.

## Protocol and Spec Compliance
- MQTT v5.0 spec is authoritative for current behavior.
- Cite exact conformance statements using `[MQTT-x.x.x-y]` in code or docs when relevant.
- Apply the same citation style for future MQTT versions.

## Error Handling and Invalid States
- Prefer types that make invalid states unrepresentable.
- Avoid runtime checks when types can encode constraints.

## Contribution Workflow
- Keep changes minimal and atomic.
- Run required formatting and linting before completion.
