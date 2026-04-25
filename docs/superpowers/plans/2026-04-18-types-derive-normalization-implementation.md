# sansio-mqtt-v5-types Derive Normalization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize derives in `sansio-mqtt-v5-types` with per-type decisions
that maximize ergonomic utility while enforcing explicit constraints (no
`Default` on control packets, no `Hash/Ord/PartialOrd` on empty marker types).

**Architecture:** Execute derive normalization in two passes: first create a
per-type derive decision matrix and enforce hard constraints, then apply derive
updates module-by-module with targeted compile/test gates. Keep behavior
unchanged and treat this as API-shape hygiene only.

**Tech Stack:** Rust (no_std + alloc), Cargo, clippy, workspace tests.

---

### Task 1: Build Per-Type Derive Decision Matrix and Lock Constraints

**Files:**

- Create: `docs/superpowers/checklists/2026-04-18-types-derive-matrix.md`
- Modify: `crates/sansio-mqtt-v5-types/src/types/control_packet.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/types/basic.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/types/properties.rs`

- [ ] **Step 1: Write failing guard test for hard constraints**

Add compile-time style guard tests to
`crates/sansio-mqtt-v5-types/src/types/control_packet.rs` and
`crates/sansio-mqtt-v5-types/src/types/basic.rs` under `#[cfg(test)]`:

```rust
#[cfg(test)]
mod derive_guards {
    use super::*;

    // If this compiles, ControlPacket intentionally does NOT implement Default.
    #[test]
    fn control_packet_does_not_derive_default() {
        fn assert_not_default<T: core::fmt::Debug>() {}
        assert_not_default::<ControlPacket>();
    }
}
```

And for empty marker errors (example in `basic.rs`):

```rust
#[cfg(test)]
mod marker_trait_guards {
    use super::*;

    #[test]
    fn marker_errors_are_not_ordered_or_hashed() {
        // Intentionally compile-only shape check by trait omission policy;
        // no runtime assertion needed.
        let _ = PayloadError;
        let _ = BinaryDataError;
        let _ = Utf8StringError;
        let _ = TopicError;
    }
}
```

- [ ] **Step 2: Run targeted tests expecting initial mismatch/failure if current
      derives violate constraints**

Run:

```bash
cargo test -p sansio-mqtt-v5-types derive_guards -- --nocapture
```

Expected: If constraints are violated, compile/test should fail and indicate
conflicting derive usage. If already aligned, proceed and document pass.

- [ ] **Step 3: Create per-type derive matrix document**

Create `docs/superpowers/checklists/2026-04-18-types-derive-matrix.md` with one
line per type in `src/types/*.rs`:

```markdown
| Type          | Current Derives | Planned Derives | Rationale                            |
| ------------- | --------------- | --------------- | ------------------------------------ |
| ControlPacket | ...             | ...             | No Default by rule                   |
| PayloadError  | ...             | ...             | Empty marker; no Hash/Ord/PartialOrd |
```

Include all types touched by the current branch changes first (`auth.rs`,
`basic.rs`, `connack.rs`, `connect.rs`, `control_packet.rs`, `properties.rs`,
`reason_code.rs`, `subscribe.rs`) and then complete remaining type files.

- [ ] **Step 4: Apply only hard-constraint derive fixes now**

Make minimal code edits:

- Ensure `ControlPacket` and control packet types do not derive `Default`.
- Ensure empty marker/unit error types do not derive `Hash`, `Ord`,
  `PartialOrd`.

Do not yet broaden ergonomic derives in this step.

- [ ] **Step 5: Run focused validation**

Run:

```bash
cargo test -p sansio-mqtt-v5-types
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

```bash
git add docs/superpowers/checklists/2026-04-18-types-derive-matrix.md crates/sansio-mqtt-v5-types/src/types/control_packet.rs crates/sansio-mqtt-v5-types/src/types/basic.rs crates/sansio-mqtt-v5-types/src/types/properties.rs
git commit -m "chore(types): enforce derive constraints for packets and markers"
```

### Task 2: Apply Per-Type Ergonomic Derive Updates (Core Type Modules)

**Files:**

- Modify: `crates/sansio-mqtt-v5-types/src/types/basic.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/types/connect.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/types/connack.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/types/auth.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/types/properties.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/types/reason_code.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/types/subscribe.rs`

- [ ] **Step 1: Write failing compile gate tests for ergonomic derives expected
      on selected value types**

Add tests in `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs`:

```rust
#[test]
fn value_types_support_hash_and_order_where_semantic() {
    use core::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let a = sansio_mqtt_v5_types::Utf8String::new("a");
    let b = sansio_mqtt_v5_types::Utf8String::new("b");
    assert!(a < b);

    let mut hasher = DefaultHasher::new();
    a.hash(&mut hasher);
    let _ = hasher.finish();
}
```

Add equivalent checks for one reason-code enum that should remain
hashable/orderable only if matrix says yes.

- [ ] **Step 2: Run targeted tests and capture failures**

Run:

```bash
cargo test -p sansio-mqtt-v5-types --test property_compatibility value_types_support_hash_and_order_where_semantic -- --nocapture
```

Expected: FAIL where derives are missing according to matrix.

- [ ] **Step 3: Apply derive updates module-by-module using matrix decisions**

For each module listed above:

- Update `#[derive(...)]` exactly per matrix.
- Prefer adding `Eq/Hash/Copy/Ord/PartialOrd` only when semantically justified
  and accepted by constraints.
- Leave behavior and fields unchanged.

After each module edit, run:

```bash
cargo test -p sansio-mqtt-v5-types --test property_compatibility -- --nocapture
```

- [ ] **Step 4: Full types crate validation**

Run:

```bash
cargo test -p sansio-mqtt-v5-types
cargo clippy -p sansio-mqtt-v5-types --all-targets
```

Expected: PASS (allowing only pre-existing warnings unrelated to derive policy,
but record them).

- [ ] **Step 5: Commit Task 2**

```bash
git add crates/sansio-mqtt-v5-types/src/types/basic.rs crates/sansio-mqtt-v5-types/src/types/connect.rs crates/sansio-mqtt-v5-types/src/types/connack.rs crates/sansio-mqtt-v5-types/src/types/auth.rs crates/sansio-mqtt-v5-types/src/types/properties.rs crates/sansio-mqtt-v5-types/src/types/reason_code.rs crates/sansio-mqtt-v5-types/src/types/subscribe.rs crates/sansio-mqtt-v5-types/tests/property_compatibility.rs
git commit -m "refactor(types): normalize derives for core value and packet types"
```

### Task 3: Complete Remaining Type Modules and Cross-Crate Validation

**Files:**

- Modify: remaining `crates/sansio-mqtt-v5-types/src/types/*.rs` not covered in
  Task 2
- Modify: `docs/superpowers/checklists/2026-04-18-types-derive-matrix.md`

- [ ] **Step 1: Add/adjust one failing test for a remaining type derive
      expectation**

In `crates/sansio-mqtt-v5-types/tests/mirror_encode_and_parse.rs`, add a tiny
compile-use assertion for a type from remaining modules:

```rust
#[test]
fn remaining_types_have_expected_clone_and_eq_shape() {
    let a = sansio_mqtt_v5_types::PingReq { properties: Default::default() };
    let b = a.clone();
    assert_eq!(a, b);
}
```

Adjust target type if `PingReq` shape differs; use any remaining module type
from matrix.

- [ ] **Step 2: Run targeted test expecting fail if derive missing**

Run:

```bash
cargo test -p sansio-mqtt-v5-types --test mirror_encode_and_parse remaining_types_have_expected_clone_and_eq_shape -- --nocapture
```

Expected: FAIL if matrix-required derive is missing.

- [ ] **Step 3: Apply derive updates to remaining type modules per matrix**

Update all remaining `types/*.rs` derive lists to match matrix decisions. Keep
edits strictly to derive macros unless one compile fix is required by trait
bounds.

Also update matrix status column to `Applied` for each type.

- [ ] **Step 4: Run full workspace verification**

Run:

```bash
cargo fmt
cargo test -q
cargo clippy -p sansio-mqtt-v5-types --all-targets
cargo clippy -p sansio-mqtt-v5-protocol --all-targets
cargo test -p sansio-mqtt-v5-tokio
```

Expected: No regressions.

- [ ] **Step 5: Commit Task 3**

```bash
git add crates/sansio-mqtt-v5-types/src/types docs/superpowers/checklists/2026-04-18-types-derive-matrix.md crates/sansio-mqtt-v5-types/tests/mirror_encode_and_parse.rs
git commit -m "refactor(types): complete derive normalization across type modules"
```

### Task 4: Final Policy Consistency Pass and Developer Notes

**Files:**

- Modify: `docs/superpowers/checklists/2026-04-18-types-derive-matrix.md`

- [ ] **Step 1: Add final summary section to matrix doc**

Append:

```markdown
## Final Summary

- Control packet types checked: no `Default`
- Empty marker types checked: no `Hash`/`Ord`/`PartialOrd`
- Per-type decisions applied and validated
```

- [ ] **Step 2: Verify no accidental `Default` on control packets**

Run:

```bash
rg "derive\(.*Default.*\)" crates/sansio-mqtt-v5-types/src/types/control_packet.rs crates/sansio-mqtt-v5-types/src/types/*packet*.rs
```

Expected: no control packet derive lines containing `Default`.

- [ ] **Step 3: Verify empty marker types don’t derive hash/order traits**

Run:

```bash
rg "derive\(.*(Hash|PartialOrd|Ord).*\)" crates/sansio-mqtt-v5-types/src/types/basic.rs crates/sansio-mqtt-v5-types/src/types/properties.rs crates/sansio-mqtt-v5-types/src/types/control_packet.rs
```

Expected: no matches on empty marker types.

- [ ] **Step 4: Commit Task 4**

```bash
git add docs/superpowers/checklists/2026-04-18-types-derive-matrix.md
git commit -m "docs(types): record final derive normalization decisions"
```

## Spec Coverage Check

- Per-type review approach: covered by Task 1 matrix + module-by-module tasks.
- Ergonomic utility bias: covered by Task 2 and Task 3 targeted derive
  additions.
- Constraint “no Default on control packets”: enforced in Task 1 and verified in
  Task 4.
- Constraint “no Hash/Ord/PartialOrd on empty types”: enforced in Task 1 and
  verified in Task 4.

## Placeholder Scan

- No placeholders (`TODO`/`TBD`) in tasks.
- All command steps have concrete commands and expected outcomes.
- Code-edit steps are explicit about affected files and scope.

## Type Consistency Check

- Uses consistent naming (`ControlPacket`, marker error types, derive matrix
  terminology).
- Constraints and verification commands align with spec and current codebase
  layout.
