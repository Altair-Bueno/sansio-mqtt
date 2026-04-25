# PUBLISH multiple Subscription Identifiers — design

Date: 2026-04-19

## Problem

`PublishProperties.subscription_identifier` is typed as `Option<NonZero<u64>>`,
which can carry at most one identifier. The MQTT v5 specification requires
PUBLISH to carry zero or more Subscription Identifiers:

> [MQTT-3.3.2.3.8] Multiple Subscription Identifiers will be included if the
> publication is the result of a match to more than one subscription, in this
> case their order is not significant.

The current parser actively rejects PUBLISH packets that contain more than one
`Subscription Identifier` property with `DuplicatedPropertyError`. This is wrong
under the spec — servers legitimately include multiple identifiers when a single
message matches multiple subscriptions, and clients MUST accept them.

SUBSCRIBE is unaffected. [MQTT-3.8.2.1.2] states:

> It is a Protocol Error to include the Subscription Identifier more than once.

So SUBSCRIBE's `Option<NonZero<u64>>` shape is spec-correct and stays.

## Scope

Crates touched:

- `sansio-mqtt-v5-types` — wire-level type, parser, encoder, parser settings,
  tests.
- `sansio-mqtt-v5-protocol` — `BrokerMessage` (inbound PUBLISH surface) and the
  protocol driver path that builds it from the wire type.
- `sansio-mqtt-v5-tokio` — example call site.

Not touched:

- `SubscribeProperties.subscription_identifier` in `types/subscribe.rs` — stays
  `Option<NonZero<u64>>`.
- `SubscribeOptions.subscription_identifier` in `protocol/src/types.rs` — stays
  `Option<NonZero<u64>>`.
- `ConnAckProperties.subscription_identifiers_available` — unrelated CONNACK
  capability flag.

## Design

### Wire type (`sansio-mqtt-v5-types`)

`PublishProperties.subscription_identifier: Option<NonZero<u64>>` becomes:

```rust
pub subscription_identifiers: Vec<NonZero<u64>>,
```

Field renamed to plural. Empty `Vec` means "property absent on the wire" (the
encoder emits nothing). The type makes the invariant "PUBLISH carries 0..N
subscription identifiers" directly representable — invalid states (such as
"absent but with a value") are unrepresentable.

### Parser (`parser/publish.rs`)

Replace the duplicate-rejection arm with append-with-bound, mirroring the
pattern used for `user_properties`:

```rust
Property::SubscriptionIdentifier(value) => {
    if properties.subscription_identifiers.len()
        >= parser_settings.max_subscription_identifiers_len
    {
        return Err(PropertiesError::from(
            TooManySubscriptionIdentifiersError,
        ));
    }
    properties.subscription_identifiers.push(value);
}
```

A new error variant `TooManySubscriptionIdentifiersError` is added alongside the
existing `TooManyUserPropertiesError`, following the same structure.

### Parser settings (`parser/mod.rs`)

A new field on `ParserSettings`:

```rust
pub max_subscription_identifiers_len: usize,
```

Defaults:

- Default constructor: `32` (matches `max_user_properties_len`).
- `no_limits` constructor: `usize::MAX`.

Rationale: without a bound, the property section (VBI-length-prefixed up to ~268
MB) permits tens of millions of identifiers, a DoS vector. Every other
variable-length property already has a `max_*_len`. A dedicated field (rather
than reusing `max_user_properties_len`) is consistent with the rest of the
settings surface and lets operators tune it independently.

### Encoder (`encoder/publish.rs`)

The current `Option`-to-single-`Property` mapping becomes an iterator over all
entries, analogous to how `user_properties` is encoded via
`encode::combinators::Iter`:

```rust
let subscription_identifiers = encode::combinators::Iter::new(
    self.subscription_identifiers
        .iter()
        .copied()
        .map(Property::SubscriptionIdentifier),
);
```

Empty Vec → zero bytes emitted. Single entry → one property emitted. Many → one
property per entry. Property ordering within the `LengthPrefix` tuple is
preserved.

### Protocol layer (`sansio-mqtt-v5-protocol`)

`BrokerMessage.subscription_identifier: Option<NonZero<u64>>` becomes
`subscription_identifiers: Vec<NonZero<u64>>` in `src/types.rs`. The driver code
in `src/proto.rs` that copies this field from the parsed wire
`PublishProperties` into `BrokerMessage` (around line 1135) is updated to
move/clone the `Vec` instead of the `Option`.

`SubscribeOptions.subscription_identifier` at `src/types.rs:162` is unchanged
(SUBSCRIBE is 0..1).

### Downstream call sites

- `crates/sansio-mqtt-v5-tokio/examples/cli.rs` constructs a
  `BrokerMessage`-shaped literal with `subscription_identifier: None`; updated
  to `subscription_identifiers: Vec::new()`.
- `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` has many
  `subscription_identifier: None` / `Some(..)` sites for `BrokerMessage`;
  updated in bulk to `Vec::new()` or `vec![..]`. `SubscribeOptions` sites are
  unchanged.
- `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs` references to
  `PublishProperties.subscription_identifier` (lines 329, 352) updated to the
  plural `Vec` form.

## Test Strategy

1. **Invert the existing hostile test.** `tests/mirror_encode_and_parse.rs:224`
   currently lists `to_repeated_subscription_identifiers_property` under
   `assert_that_parsing_an_invalid_field_on_publish_fails`. The second
   identifier in that byte vector uses the VBI max-value form, which is legal.
   Move this case into the encode-parse-mirror success cases so that the decoded
   `PublishProperties` has `subscription_identifiers` of length two in the
   expected order.

2. **New coverage for the roundtrip.** Add at least three encode/parse roundtrip
   cases:
   - Empty `Vec` → property absent on the wire.
   - Single-entry `Vec` → one `Subscription Identifier` property on the wire.
   - Multi-entry `Vec` → one property per entry, in the order present in the
     `Vec`.

3. **Bound enforcement.** Add a parser test: a PUBLISH containing
   `max_subscription_identifiers_len + 1` subscription-identifier properties
   fails with `TooManySubscriptionIdentifiersError`. Also add a test that the
   `no_limits` settings accept a count that exceeds the default.

4. **Protocol-layer regression.** Update or extend
   `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` so that at least
   one `BrokerMessage` assertion checks `subscription_identifiers` as a `Vec`
   (including an inbound PUBLISH with two identifiers being propagated into the
   `BrokerMessage`).

5. **SUBSCRIBE untouched.** No changes required to SUBSCRIBE parsing/encoding
   tests; the duplicate-identifier-in-SUBSCRIBE-rejection behavior is still
   correct per [MQTT-3.8.2.1.2] and must remain covered by existing tests.

## Workspace-Wide Checks

After implementation, the workspace must pass `cargo fmt`, `cargo clippy`, and
`cargo test` per the project checklist. Documentation (rustdoc on the renamed
field) is updated to cite [MQTT-3.3.2.3.8] and note that empty `Vec` = property
absent.

## Out of Scope

- Any change to how the protocol layer exposes subscription identifiers to
  user-facing API surfaces beyond `BrokerMessage` (e.g., no new filtering
  helpers or lookup tables by identifier).
- Any change to SUBSCRIBE, CONNACK `subscription_identifiers_available`, or the
  `allow_subscription_identifiers` client setting.
- Any change to the wire format of the `Subscription Identifier` property itself
  (still a single Variable Byte Integer per occurrence).
