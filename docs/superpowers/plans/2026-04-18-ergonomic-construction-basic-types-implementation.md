# Ergonomic Construction for Basic MQTT Value Types Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a consistent `new`/`try_new` constructor model and stronger trait ergonomics for `Payload`, `BinaryData`, `Utf8String`, and `Topic` without changing validation behavior.

**Architecture:** Keep `nutype` and existing invariants, then layer ergonomic inherent constructors and concrete trait impls in `basic.rs`. Validate behavior with focused constructor tests and small call-site migrations in protocol/tokio/tests.

**Tech Stack:** Rust (`no_std` + `alloc`), `bytes::Bytes`, `nutype`, Cargo tests.

---

### Task 1: Add Constructor API Surface in `basic.rs`

**Files:**
- Modify: `crates/sansio-mqtt-v5-types/src/types/basic.rs`
- Test: `crates/sansio-mqtt-v5-types/tests/basic_construction.rs` (new)

- [ ] **Step 1: Write failing tests for the new constructor shape**

Create `crates/sansio-mqtt-v5-types/tests/basic_construction.rs`:

```rust
use core::convert::Infallible;
use sansio_mqtt_v5_types::{BinaryData, Payload, Topic, Utf8String};

#[test]
fn payload_try_new_returns_ok_infallible() {
    let value = Payload::try_new("abc").expect("payload is always valid");
    let _err_type: Result<Payload, Infallible> = Ok(value);
}

#[test]
fn payload_new_accepts_into_bytes_inputs() {
    let p1 = Payload::new("abc");
    let p2 = Payload::new(Vec::from(&b"abc"[..]));
    assert_eq!(p1.as_ref(), p2.as_ref());
}

#[test]
fn binary_data_try_new_rejects_too_large() {
    let oversized = vec![0u8; (u16::MAX as usize) + 1];
    assert!(BinaryData::try_new(oversized).is_err());
}

#[test]
#[should_panic(expected = "BinaryData::new received invalid MQTT binary data")]
fn binary_data_new_panics_on_invalid() {
    let oversized = vec![0u8; (u16::MAX as usize) + 1];
    let _ = BinaryData::new(oversized);
}

#[test]
fn utf8_string_try_new_accepts_valid_str() {
    let s = Utf8String::try_new("hello/world").expect("valid utf8 string");
    assert_eq!(s.as_ref(), "hello/world");
}

#[test]
#[should_panic(expected = "Utf8String::new received invalid MQTT utf8 string")]
fn utf8_string_new_panics_on_invalid() {
    let _ = Utf8String::new("\0");
}

#[test]
fn topic_try_new_accepts_valid_topic() {
    let t = Topic::try_new("sensor/temperature").expect("valid topic");
    assert_eq!(t.as_ref(), "sensor/temperature");
}

#[test]
#[should_panic(expected = "Topic::new received invalid MQTT topic")]
fn topic_new_panics_on_invalid_topic_filter_chars() {
    let _ = Topic::new("sensor/+");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p sansio-mqtt-v5-types --test basic_construction`

Expected: FAIL with missing associated items (`new`/`try_new`) on one or more types.

- [ ] **Step 3: Add `new`/`try_new` inherent constructors with `Into<Bytes>`**

In `crates/sansio-mqtt-v5-types/src/types/basic.rs`, add inherent impls:

```rust
impl Payload {
    #[inline]
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, core::convert::Infallible> {
        Ok(Self::new(value.into()))
    }

    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::from(value.into())
    }
}

impl BinaryData {
    #[inline]
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, BinaryDataError> {
        Self::try_from(value.into())
    }

    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value)
            .expect("BinaryData::new received invalid MQTT binary data")
    }
}

impl Utf8String {
    #[inline]
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, Utf8StringError> {
        Self::try_from(value.into())
    }

    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value)
            .expect("Utf8String::new received invalid MQTT utf8 string")
    }
}

impl Topic {
    #[inline]
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, TopicError> {
        let utf8 = Utf8String::try_new(value).map_err(TopicError::from)?;
        Self::try_from(utf8)
    }

    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value).expect("Topic::new received invalid MQTT topic")
    }
}
```

Add rustdoc comments above each `new`/`try_new` describing panic/error behavior.

- [ ] **Step 4: Run targeted tests to verify pass**

Run: `cargo test -p sansio-mqtt-v5-types --test basic_construction`

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/sansio-mqtt-v5-types/src/types/basic.rs crates/sansio-mqtt-v5-types/tests/basic_construction.rs
git commit -m "feat(types): add unified new/try_new constructors"
```

### Task 2: Complete Trait Coverage for Common Input Forms

**Files:**
- Modify: `crates/sansio-mqtt-v5-types/src/types/basic.rs`
- Test: `crates/sansio-mqtt-v5-types/tests/basic_construction.rs`

- [ ] **Step 1: Add failing tests for missing conversions**

Append to `crates/sansio-mqtt-v5-types/tests/basic_construction.rs`:

```rust
#[test]
fn utf8_string_try_from_non_static_str() {
    let owned = String::from("dynamic/topic");
    let value = Utf8String::try_from(owned.as_str()).expect("valid utf8");
    assert_eq!(value.as_ref(), "dynamic/topic");
}

#[test]
fn binary_data_try_from_non_static_slice() {
    let input = [1u8, 2, 3, 4];
    let value = BinaryData::try_from(&input[..]).expect("valid binary data");
    assert_eq!(value.as_ref().as_ref(), &input);
}

#[test]
fn payload_from_non_static_slice() {
    let input = [9u8, 8, 7];
    let value = Payload::from(&input[..]);
    assert_eq!(value.as_ref().as_ref(), &input);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p sansio-mqtt-v5-types --test basic_construction`

Expected: FAIL for missing `TryFrom<&str>`/`TryFrom<&[u8]>`/`From<&[u8]>` where currently absent.

- [ ] **Step 3: Implement missing `From`/`TryFrom` impls**

In `crates/sansio-mqtt-v5-types/src/types/basic.rs`, add concrete impls:

```rust
impl<'a> From<&'a [u8]> for Payload {
    #[inline]
    fn from(value: &'a [u8]) -> Self {
        Self::from(bytes::Bytes::copy_from_slice(value))
    }
}

impl<'a> TryFrom<&'a [u8]> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Self::try_from(bytes::Bytes::copy_from_slice(value))
    }
}

impl<'a> TryFrom<&'a [u8]> for Utf8String {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Self::try_from(bytes::Bytes::copy_from_slice(value))
    }
}

impl<'a> TryFrom<&'a str> for Utf8String {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::try_from(bytes::Bytes::copy_from_slice(value.as_bytes()))
    }
}
```

Keep existing `'static` impls if needed for compatibility, but prefer lifetime-generic versions.

- [ ] **Step 4: Run tests and full crate tests**

Run:
- `cargo test -p sansio-mqtt-v5-types --test basic_construction`
- `cargo test -p sansio-mqtt-v5-types`

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add crates/sansio-mqtt-v5-types/src/types/basic.rs crates/sansio-mqtt-v5-types/tests/basic_construction.rs
git commit -m "feat(types): extend conversion trait coverage for basic types"
```

### Task 3: Migrate Representative Call Sites and Validate Workspace

**Files:**
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`
- Modify: `crates/sansio-mqtt-v5-tokio/examples/echo.rs`
- Modify: `crates/sansio-mqtt-v5-types/tests/mirror_encode_and_parse.rs`
- Modify: `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs`

- [ ] **Step 1: Write a small migration-focused failing test update**

In `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs`, update one existing constructor chain to the new API to force compiler guidance:

```rust
// Before:
// topic: Utf8String::try_from("test/topic").unwrap().try_into().unwrap(),

// After:
topic: Topic::try_new("test/topic").unwrap(),
```

Repeat for one `BinaryData` and one `Utf8String` example:

```rust
correlation_data: BinaryData::try_new(&[1, 2][..]).ok(),
content_type: Utf8String::try_new("text/plain").ok(),
```

- [ ] **Step 2: Run targeted tests and verify migration compiles**

Run:
- `cargo test -p sansio-mqtt-v5-types --test property_compatibility`
- `cargo test -p sansio-mqtt-v5-types --test mirror_encode_and_parse`

Expected: PASS.

- [ ] **Step 3: Apply representative migration across protocol/tokio/tests**

Perform focused replacements (not mandatory exhaustive rewrite):

```rust
// protocol test style update
let topic = Topic::try_new("replay/topic").expect("valid topic");

// tokio example
subscription: Utf8String::new("echo/+/in"),

// types tests
payload: Payload::new(&[1, 2, 3, 4][..]),
```

Guideline: migrate repetitive constructor chains where readability clearly improves; avoid unrelated churn.

- [ ] **Step 4: Run full verification**

Run:
- `cargo fmt`
- `cargo test -q`
- `cargo clippy -p sansio-mqtt-v5-types --all-targets`

Expected:
- Format clean
- Full test suite PASS
- Clippy passes for modified crate (or only pre-existing warnings outside scope)

- [ ] **Step 5: Commit Task 3**

```bash
git add crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs crates/sansio-mqtt-v5-tokio/examples/echo.rs crates/sansio-mqtt-v5-types/tests/mirror_encode_and_parse.rs crates/sansio-mqtt-v5-types/tests/property_compatibility.rs
git commit -m "refactor: adopt ergonomic constructors in representative call sites"
```

### Task 4: Final Documentation and Regression Guardrails

**Files:**
- Modify: `crates/sansio-mqtt-v5-types/src/types/basic.rs`

- [ ] **Step 1: Add rustdoc examples for each type constructor pair**

For each type, include `new` and `try_new` examples:

```rust
/// Creates a validated `Utf8String`.
///
/// # Errors
/// Returns `Utf8StringError` if bytes are not valid MQTT UTF-8.
pub fn try_new(...)

/// Creates a `Utf8String`, panicking on invalid input.
///
/// # Panics
/// Panics if input violates MQTT UTF-8 constraints.
pub fn new(...)
```

- [ ] **Step 2: Run docs and tests check**

Run:
- `cargo test -p sansio-mqtt-v5-types`

Expected: PASS.

- [ ] **Step 3: Final commit**

```bash
git add crates/sansio-mqtt-v5-types/src/types/basic.rs
git commit -m "docs(types): document ergonomic constructor behavior"
```

## Spec Coverage Check

- Unified `new`/`try_new` with `Into<Bytes>`: covered by Task 1.
- Trait coverage review/additions: covered by Task 2.
- Keep `nutype` and preserve invariants: explicitly preserved across all tasks.
- Migration of representative call sites: covered by Task 3.
- Panic/error documentation: covered by Task 4.

## Placeholder Scan

No `TODO`, `TBD`, or undefined “handle appropriately” instructions. All code-changing steps include explicit code blocks and runnable commands.

## Type Consistency Check

Plan consistently uses:
- `try_new(value: impl Into<bytes::Bytes>) -> Result<..., ...>`
- `new(value: impl Into<bytes::Bytes>) -> ...`

No mixed naming variants (`from_str`, `from_slice`) are required for this iteration.
