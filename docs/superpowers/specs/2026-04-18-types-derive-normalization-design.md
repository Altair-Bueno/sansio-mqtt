# sansio-mqtt-v5-types Derive Normalization Design

Date: 2026-04-18
Scope: `crates/sansio-mqtt-v5-types/src/types/*.rs`

## Goal

Normalize derives across `sansio-mqtt-v5-types` using a per-type review, with high ergonomic utility while preserving semantic correctness.

## Constraints (User-Specified)

1. Control packets MUST NOT derive `Default`.
2. `Hash` / `Ord` / `PartialOrd` do not make sense for empty types and must not be derived there.
3. Review each type individually; no one-size-fits-all blanket rule.
4. Bias decisions toward ergonomic utility when semantically valid.

## Decision Framework

For every type, apply this order:

1. Semantic correctness first (derive only what makes sense for the type).
2. Ergonomic utility second (if semantically valid and broadly useful, prefer deriving).
3. Enforce hard constraints above.

## Category Baselines (Guidance, Not Blanket Rules)

These baselines guide consistency and then get refined per type.

### A) Control Packet Enums / Header Types

Baseline:
- `Debug`, `Clone`, `PartialEq`
- Add `Eq` if all fields support it

Never:
- `Default`

### B) Packet Payload Structs

Examples: `Connect`, `Publish`, `ConnAck`, packet property structs.

Baseline:
- `Debug`, `Clone`, `PartialEq`
- Add `Eq` when valid

Optional:
- `Default` only when there is a meaningful neutral/default state and concrete utility

Generally avoid:
- `Ord` / `PartialOrd` unless ordering is meaningful and useful

### C) Value Newtypes / Scalar Wrappers

Examples: byte/string/topic wrappers and strongly-typed scalar wrappers.

Baseline:
- `Debug`, `Clone`, `PartialEq`, `Eq`

Typically include when semantically clear:
- `Hash`
- `PartialOrd`, `Ord` (for lexicographic/value ordering)

`Default`:
- Keep only when empty/default value is meaningful and useful in existing code/tests

### D) Reason Code / Closed Protocol Enums

Baseline:
- `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`

Usually include:
- `Hash`

Optional:
- `Default` only if one canonical default is intentionally part of API ergonomics

Use carefully:
- `Ord` / `PartialOrd` only if ordering utility is clear and non-misleading

### E) Empty Marker / Unit-like Error Types

Baseline:
- `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `thiserror::Error`

Never:
- `Hash`, `PartialOrd`, `Ord`

`Default`:
- Allowed only when it provides practical ergonomic value and does not imply semantic state

## Per-Type Review Procedure

For each type, evaluate:

1. Equality semantics: should it support `PartialEq`/`Eq`?
2. Hash semantics: is hash identity meaningful and useful?
3. Ordering semantics: does ordering express meaningful relation or just arbitrary ordering?
4. Default semantics: is there a non-surprising, valid default state?
5. Usage utility: do tests/protocol/tokio integration benefit now?

Expected output for implementation notes:
- `TypeName`: `+TraitX`, `-TraitY` with one-line rationale.

## Out-of-Scope

- Behavioral/protocol logic changes.
- Parser/encoder algorithm changes.
- Broad API redesign beyond derive lists.

## Verification Plan

After derive updates:

1. `cargo test -p sansio-mqtt-v5-types`
2. `cargo test -p sansio-mqtt-v5-protocol`
3. `cargo test -p sansio-mqtt-v5-tokio`
4. `cargo clippy -p sansio-mqtt-v5-types --all-targets`

Acceptance:
- No regressions.
- Derive policy is internally consistent and follows constraints.
