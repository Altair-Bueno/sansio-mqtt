# AGENTS.md

## Index
- [Purpose and Scope](#purpose-and-scope)
- [Mandatory Checklist (MUST follow)](#mandatory-checklist-must-follow)
- [Build and Tooling](#build-and-tooling)
- [Architecture and Crates](#architecture-and-crates)
- [Protocol and Spec Compliance](#protocol-and-spec-compliance)
- [Error Handling and Invalid States](#error-handling-and-invalid-states)
- [Contribution Workflow](#contribution-workflow)

Key Paths:
- `AGENTS.md`
- `Cargo.toml`
- `crates/*`
- `rust-toolchain.toml`

## Purpose and Scope
- This repo implements sansio MQTT protocol components in Rust.
- Current focus: MQTT v5.0; future MQTT versions are planned and should follow the same conventions.
- Default stance: no_std-first; `alloc` allowed when required.

## Mandatory Checklist (MUST follow)
- Spec gate: MUST read the MQTT v5.0 spec before implementing anything: https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html
- TDD: for behavior changes, add/adjust tests before implementation when feasible; run relevant tests after.
- Type safety: encode invariants in types; avoid runtime checks when types can enforce.
- No `unsafe`: enforce with `#![forbid(unsafe_code)]`.
- no_std-first: use `alloc` only when required.
- Required formatting: `cargo fmt`.
- Required linting: `cargo clippy`.
- Documentation: update when API behavior or constraints change.
- Change atomicity: keep changes commit-ready as a single coherent unit.

## Build and Tooling
- Build system: Cargo.
- Tests are optional for non-behavior changes; for behavior changes, follow the TDD checklist item.
- Prefer editor LSP for Rust; if LSP is not configured, prompt the user to enable it.

## Architecture and Crates
- Workspace layout: `crates/*`.
- Crate naming convention: `sansio-mqtt-<version>-<scope>` (crate naming).
- Follow existing crate boundaries; add new crates using the naming convention.

## Protocol and Spec Compliance
- MQTT v5.0 spec is authoritative for current behavior.
- Before any implementation, review the MQTT v5.0 spec: https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html
- Cite exact conformance statements using `[MQTT-x.x.x-y]` in code or docs when relevant.
- Apply the same citation style for future MQTT versions.

## Error Handling and Invalid States
- Prefer types that make invalid states unrepresentable.
- Avoid runtime checks when types can encode constraints.

## Contribution Workflow
- Keep changes minimal and atomic.
- Run required checks from the checklist before completion.
- Default execution mode: subagent-driven.
