# MQTT v5 Types Non-Exhaustive Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `#[non_exhaustive]` to the public `sansio-mqtt-v5-types` API
surface to reduce breaking changes when new fields/variants are introduced by
future spec revisions.

**Architecture:** Perform an allowlist/denylist-driven API hardening pass over
all public structs/enums in `sansio-mqtt-v5-types`, then fix internal
tests/usages that rely on exhaustive construction/matching. Keep exclusions
explicit in this plan (authoritative scope), and verify compatibility by running
protocol/tokio/workspace tests.

**Tech Stack:** Rust (`no_std` + `alloc`), Cargo test/fmt/clippy, MQTT v5 packet
type model in `crates/sansio-mqtt-v5-types`.

---

## Scope Lists (Authoritative)

### Inclusions: Add `#[non_exhaustive]`

#### `crates/sansio-mqtt-v5-types/src/types/`

- `ControlPacket`
- `Connect`
- `ConnectHeaderFlags`
- `Will`
- `WillProperties`
- `ConnectProperties`
- `ConnAck`
- `ConnAckKind`
- `ConnAckHeaderFlags`
- `ConnAckProperties`
- `Publish`
- `PublishKind`
- `PublishHeaderFlags`
- `PublishHeaderFlagsKind`
- `PublishProperties`
- `PubAck`
- `PubAckHeaderFlags`
- `PubAckProperties`
- `PubRec`
- `PubRecHeaderFlags`
- `PubRecProperties`
- `PubRel`
- `PubRelHeaderFlags`
- `PubRelProperties`
- `PubComp`
- `PubCompHeaderFlags`
- `PubCompProperties`
- `Subscribe`
- `SubscribeHeaderFlags`
- `Subscription`
- `SubscribeProperties`
- `SubAck`
- `SubAckProperties`
- `SubAckHeaderFlags`
- `Unsubscribe`
- `UnsubscribeHeaderFlags`
- `UnsubscribeProperties`
- `UnsubAck`
- `UnsubAckHeaderFlags`
- `UnsubAckProperties`
- `Disconnect`
- `DisconnectHeaderFlags`
- `DisconnectProperties`
- `Auth`
- `AuthHeaderFlags`
- `AuthProperties`
- `PingReq`
- `PingReqHeaderFlags`
- `PingResp`
- `PingRespHeaderFlags`
- `Reserved`
- `ReservedHeaderFlags`
- `Property`
- `AuthenticationKind`
- `ConnectReasonCode`
- `ConnackReasonCode`
- `PublishReasonCode`
- `PubAckReasonCode`
- `PubRecReasonCode`
- `PubRelReasonCode`
- `PubCompReasonCode`
- `SubAckReasonCode`
- `UnsubAckReasonCode`
- `DisconnectReasonCode`
- `AuthReasonCode`
- `RetainHandling`
- `FormatIndicator`
- `Qos`
- `MaximumQoS`
- `GuaranteedQoS`

#### Other public API files

- `ParserSettings` (`crates/sansio-mqtt-v5-types/src/parser/mod.rs`)

### Exclusions: Do **NOT** add `#[non_exhaustive]`

- All error types/enums/structs across crate, including but not limited to:
  - `EncodeError`
  - `PropertiesError`
  - `PayloadError`, `BinaryDataError`, `Utf8StringError`, `TopicError`
  - `InvalidPropertyTypeError`, `DuplicatedPropertyError`,
    `UnsupportedPropertyError`
  - `TooManyUserPropertiesError`, `MissingAuthenticationMethodError`
  - `InvalidControlPacketTypeError`, `InvalidReasonCode`,
    `InvalidRetainHandlingError`, `UnknownFormatIndicatorError`,
    `InvalidQosError`
- `Utf8String`
- `Topic`
- `Payload`
- `BinaryData`
- `VariableByteInteger`

### Guardrail

If a public struct/enum is found that is in neither list, STOP and add it
explicitly to either inclusion or exclusion list before implementation.

### Task 1: Add `#[non_exhaustive]` to inclusion list types

**Files:**

- Modify: `crates/sansio-mqtt-v5-types/src/types/*.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/parser/mod.rs`

- [ ] **Step 1: Write a failing scope test to enforce inclusion/exclusion
      counts**

Add a test module in `crates/sansio-mqtt-v5-types/src/lib.rs` (or dedicated test
file) that fails until all inclusions are marked. Use
`core::mem::size_of`/compile checks as needed, or simpler: add a rustdoc compile
test snippet verifying external exhaustive matches fail for one representative
included enum and one included struct.

Representative compile-fail doc tests:

```rust,compile_fail
use sansio_mqtt_v5_types::ControlPacket;

fn bad_match(packet: ControlPacket) {
    match packet {
        ControlPacket::PingReq(_) => {}
    }
}
```

```rust,compile_fail
use sansio_mqtt_v5_types::Connect;

fn bad_construct() -> Connect {
    Connect {
        protocol_name: "MQTT".try_into().unwrap(),
        protocol_version: 5,
        clean_start: false,
        client_identifier: None,
        will: None,
        user_name: None,
        password: None,
        keep_alive: None,
        properties: Default::default(),
    }
}
```

- [ ] **Step 2: Run test/doc checks to verify RED**

Run:

```bash
cargo test -p sansio-mqtt-v5-types --doc
```

Expected: FAIL before annotations are applied.

- [ ] **Step 3: Apply `#[non_exhaustive]` to all inclusion-list public
      structs/enums**

For each included item, add attribute immediately above type declaration:

```rust
#[non_exhaustive]
pub enum ControlPacket { ... }
```

```rust
#[non_exhaustive]
pub struct Connect { ... }
```

Do not touch exclusions.

- [ ] **Step 4: Run doc checks to verify GREEN for representative behavior**

Run:

```bash
cargo test -p sansio-mqtt-v5-types --doc
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/sansio-mqtt-v5-types/src/types crates/sansio-mqtt-v5-types/src/parser/mod.rs crates/sansio-mqtt-v5-types/src/lib.rs
git commit -m "refactor(v5-types): mark public packet and reason types non-exhaustive"
```

### Task 2: Fix internal exhaustive matches/construction affected by non-exhaustive

**Files:**

- Modify: `crates/sansio-mqtt-v5-types/src/**/*.rs` (tests/docs/examples as
  needed)
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` (if direct
  construction affected)
- Modify: `crates/sansio-mqtt-v5-tokio/tests/*.rs` (if direct construction
  affected)

- [ ] **Step 1: Write/enable failing compile path in tests that use exhaustive
      construction**

Run targeted compile checks first to surface breakages:

```bash
cargo test -p sansio-mqtt-v5-types
```

Expected: FAIL on exhaustive struct literals/matches where outside defining
module.

- [ ] **Step 2: Fix callsites with stable patterns**

Use these transformations:

```rust
// before (exhaustive match)
match packet {
    ControlPacket::ConnAck(a) => ...,
    ControlPacket::Publish(p) => ...,
}

// after
match packet {
    ControlPacket::ConnAck(a) => ...,
    ControlPacket::Publish(p) => ...,
    _ => ...,
}
```

```rust
// before (struct literal from other module/crate)
Connect { ... }

// after
// use constructor/helper already provided in crate,
// or move construction into defining module tests if construction API is intentionally absent.
```

- [ ] **Step 3: Run crate tests to verify GREEN**

Run:

```bash
cargo test -p sansio-mqtt-v5-types
```

Expected: PASS.

- [ ] **Step 4: Commit Task 2**

```bash
git add crates/sansio-mqtt-v5-types crates/sansio-mqtt-v5-protocol/tests crates/sansio-mqtt-v5-tokio/tests
git commit -m "fix(v5-types): adapt internal matches and constructions for non-exhaustive api"
```

### Task 3: Verify exclusion list remained untouched

**Files:**

- Modify: none expected (verification only)

- [ ] **Step 1: Run grep checks for excluded types**

Run:

```bash
rg "#\[non_exhaustive\]\npub struct (Utf8String|Topic|Payload|BinaryData)" crates/sansio-mqtt-v5-types/src
rg "#\[non_exhaustive\]\npub struct VariableByteInteger" crates/sansio-mqtt-v5-types/src
rg "#\[non_exhaustive\]\npub (struct|enum) .*Error" crates/sansio-mqtt-v5-types/src
```

Expected: no matches.

- [ ] **Step 2: Commit only if corrective change was necessary**

If any accidental annotation is found and removed:

```bash
git add crates/sansio-mqtt-v5-types/src
git commit -m "fix(v5-types): keep non-exhaustive exclusions intact"
```

### Task 4: Full verification and cleanup

**Files:**

- Modify: only if final fixes are required

- [ ] **Step 1: Run full verification suite**

```bash
cargo test -p sansio-mqtt-v5-types
cargo test -p sansio-mqtt-v5-protocol
cargo test -p sansio-mqtt-v5-tokio
cargo test -q
```

Expected: PASS.

- [ ] **Step 2: Run formatting and linting**

```bash
cargo fmt
cargo clippy
```

Expected: no new warnings beyond baseline.

- [ ] **Step 3: Commit final verification fixes (if needed)**

```bash
git add crates/sansio-mqtt-v5-types crates/sansio-mqtt-v5-protocol crates/sansio-mqtt-v5-tokio
git commit -m "test(v5-types): verify non-exhaustive api hardening across workspace"
```

## Coverage Check

- Inclusion list fully annotated: Task 1.
- Exclusion list untouched: Task 3.
- Internal callsites adapted for new non-exhaustive behavior: Task 2.
- Workspace compatibility validated: Task 4.
