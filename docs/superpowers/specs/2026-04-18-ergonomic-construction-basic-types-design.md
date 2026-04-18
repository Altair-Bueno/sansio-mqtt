# Ergonomic Construction for Basic MQTT Value Types

Date: 2026-04-18
Scope: `crates/sansio-mqtt-v5-types` (`Payload`, `BinaryData`, `Utf8String`, `Topic`)

## Goal

Make construction of core value types materially easier while preserving protocol invariants and no-std compatibility.

Current pain points:
- Verbose call sites with chained conversions (`Utf8String::try_from(...)?` then `Topic::try_from(...)`)
- Inconsistent constructor style across types
- Limited trait coverage for non-`'static` borrowed inputs

Success criteria:
- A consistent constructor model across all four types
- Fewer conversion chains in protocol/tokio/tests call sites
- No weakening of invariants or parser/encoder behavior

## Design Decisions

### 1) Unified Constructor Model

Each type exposes both constructors:
- `try_new(value: impl Into<bytes::Bytes>) -> Result<Self, ErrorType>`
- `new(value: impl Into<bytes::Bytes>) -> Self` (panics on invalid input)

Per-type behavior:
- `Payload`
  - `try_new` returns `Result<Self, core::convert::Infallible>`
  - `new` cannot fail in practice
- `BinaryData`
  - validates maximum allowed binary length
- `Utf8String`
  - validates UTF-8, MQTT invalid characters, and length
- `Topic`
  - validates `Utf8String` constraints plus topic wildcard restrictions (`+`, `#`)

Rationale:
- `try_new` is explicit and library-safe
- `new` is ergonomic for app/test contexts where invalid input is programmer error
- `impl Into<Bytes>` provides broad input compatibility without many dedicated constructors

### 2) Trait Coverage Review and Additions

Keep using trait-based ergonomics for common inputs while avoiding coherence-heavy blanket implementations.

Target trait coverage:
- `Payload`: `From<bytes::Bytes>`, `From<Vec<u8>>`, `From<&[u8]>`, `From<&[u8; N]>`
- `BinaryData`: `TryFrom<bytes::Bytes>`, `TryFrom<Vec<u8>>`, `TryFrom<&[u8]>`, `TryFrom<&[u8; N]>`
- `Utf8String`: `TryFrom<bytes::Bytes>`, `TryFrom<Vec<u8>>`, `TryFrom<&[u8]>`, `TryFrom<&str>`, `TryFrom<String>`
- `Topic`: `TryFrom<Utf8String>`, plus byte/string entry points via `try_new`

Keep existing `AsRef`, `Deref`, `Borrow`, `Into` derivations where they help usability.

Guideline:
- Prefer broad input handling through `try_new/new`.
- Use explicit trait impls for common concrete sources.
- Avoid broad generic trait impls that increase overlap risk.

### 3) `nutype` Retention

Keep `nutype` for now.

Reasoning:
- It already encodes constraints and generated error types correctly.
- The usability gap is constructor/trait ergonomics, not invariant modeling.
- Removing `nutype` now would increase scope and risk for little direct UX gain.

Follow-up trigger for reconsideration:
- If macro-driven generated API or compile-time burden becomes a recurring issue, evaluate a dedicated manual-newtype migration separately.

## API Surface (Planned)

Planned inherent methods (all in `types/basic.rs`):
- `Payload::try_new`, `Payload::new`
- `BinaryData::try_new`, `BinaryData::new`
- `Utf8String::try_new`, `Utf8String::new`
- `Topic::try_new`, `Topic::new`

Method docs requirements:
- `new`: explicitly documents panic behavior and invalid-input conditions.
- `try_new`: documents validation rules and error type.

## Data Flow and Invariants

Construction flow:
1. Input arrives as any type implementing `Into<bytes::Bytes>`.
2. `try_new` runs existing validation path for the target type.
3. `new` calls `try_new(...).expect(...)` with clear panic message.

Invariant guarantees remain unchanged:
- No parser/encoder behavior change by default
- Invalid states remain unrepresentable at the type boundary

## Error Handling

- `try_new`: returns exact existing error types (`BinaryDataError`, `Utf8StringError`, `TopicError`, `Infallible` for payload)
- `new`: panic-only shortcut for non-fallible call style
- No silent sanitization or fallback conversions introduced

## Testing Strategy

Add targeted tests for each type:
- `try_new` accepts all currently valid representative inputs
- `try_new` rejects all existing invalid cases
- `new` succeeds for valid inputs
- `new` panics for invalid inputs (for fallible types)

Compatibility tests:
- Update call sites in existing tests/examples to use new constructors where they simplify usage
- Ensure `cargo test` remains fully green across workspace

## Migration Plan

1. Add constructor APIs and missing trait impls in `basic.rs`.
2. Refactor representative internal call sites (`protocol` + `tokio` + key tests) to demonstrate simpler usage.
3. Run full test suite and preserve behavior.
4. Keep changes focused; no unrelated API redesign.

## Non-Goals

- Removing `nutype` in this change
- Introducing macro DSLs for construction
- Changing MQTT validation semantics
- Broad refactors outside these four types and immediate call sites
