# PUBLISH Multiple Subscription Identifiers — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `PublishProperties` and `BrokerMessage` carry zero-or-more
`Subscription Identifier` values per [MQTT-3.3.2.3.8], replacing the current
`Option<NonZero<u64>>` with `Vec<NonZero<u64>>`. SUBSCRIBE stays
`Option<NonZero<u64>>` per [MQTT-3.8.2.1.2].

**Architecture:** Type-level change in two crates. Add a DoS bound
(`max_subscription_identifiers_len`) to `ParserSettings` with a matching
`TooManySubscriptionIdentifiersError`, mirroring the existing
`max_user_properties_len` / `TooManyUserPropertiesError` pair. Parser appends
rather than rejects duplicates. Encoder iterates using the existing
`encode::combinators::Iter` combinator. The wire-level change cascades into
`BrokerMessage` in the protocol crate and into one inbound-PUBLISH mapping site
in `proto.rs`.

**Tech Stack:** Rust (no_std + alloc), Cargo workspace, `winnow` parser
combinators, `thiserror`, `rstest` tests.

**Spec reference:**
`docs/superpowers/specs/2026-04-19-publish-subscription-identifiers-multi-design.md`.

---

## Preflight

- [ ] **Step 0: Ensure a clean baseline**

Run:
`cd /Users/compux72/Developer/sansio-mqtt && cargo build --workspace && cargo test --workspace`

Expected: all green. If not, stop and report — this plan assumes a clean
starting state.

Also check `git status` shows a clean tree (only the new plan file untracked is
OK).

---

### Task 1: Parser setting `max_subscription_identifiers_len`

**Files:**

- Modify: `crates/sansio-mqtt-v5-types/src/parser/mod.rs`
- Modify: `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs` (the
  `settings_default` and `settings_unlimited` tests)

- [ ] **Step 1.1: Extend `settings_default` and `settings_unlimited` tests to
      assert the new field**

Open `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs`. Replace the
bodies of `settings_default` (around line 722) and `settings_unlimited` (around
line 732) so they assert the new field:

```rust
#[test]
fn settings_default() {
    let settings = ParserSettings::default();
    assert_eq!(settings.max_bytes_string, 5 * 1024);
    assert_eq!(settings.max_bytes_binary_data, 5 * 1024);
    assert_eq!(settings.max_remaining_bytes, 1024 * 1024);
    assert_eq!(settings.max_subscriptions_len, 32);
    assert_eq!(settings.max_user_properties_len, 32);
    assert_eq!(settings.max_subscription_identifiers_len, 32);
}

#[test]
fn settings_unlimited() {
    let settings = ParserSettings::unlimited();
    assert_eq!(settings.max_bytes_string, u16::MAX);
    assert_eq!(settings.max_bytes_binary_data, u16::MAX);
    assert_eq!(settings.max_remaining_bytes, u64::MAX);
    assert_eq!(settings.max_subscriptions_len, u32::MAX);
    assert_eq!(settings.max_user_properties_len, usize::MAX);
    assert_eq!(settings.max_subscription_identifiers_len, usize::MAX);
}
```

- [ ] **Step 1.2: Run the failing tests**

Run:
`cargo test -p sansio-mqtt-v5-types --test property_compatibility settings_ -- --nocapture`

Expected: compilation failure —
`no field 'max_subscription_identifiers_len' on type 'ParserSettings'`.

- [ ] **Step 1.3: Add the new field to `ParserSettings`**

Open `crates/sansio-mqtt-v5-types/src/parser/mod.rs`. Replace the
`ParserSettings` struct and the two constructors with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserSettings {
    pub max_bytes_string: u16,
    pub max_bytes_binary_data: u16,
    pub max_remaining_bytes: u64,
    pub max_subscriptions_len: u32,
    pub max_user_properties_len: usize,
    pub max_subscription_identifiers_len: usize,
}

impl ParserSettings {
    #[inline]
    pub const fn new() -> Self {
        Self {
            max_bytes_string: 5 * 1024,              // 5 KiB
            max_bytes_binary_data: 5 * 1024,         // 5 KiB
            max_remaining_bytes: 1024 * 1024,        // 1 MiB
            max_subscriptions_len: 32,               // 32 subscriptions
            max_user_properties_len: 32,             // 32 properties
            max_subscription_identifiers_len: 32,    // 32 identifiers
        }
    }

    #[inline]
    pub const fn unlimited() -> Self {
        Self {
            max_bytes_string: u16::MAX,
            max_bytes_binary_data: u16::MAX,
            max_remaining_bytes: u64::MAX,
            max_subscriptions_len: u32::MAX,
            max_user_properties_len: usize::MAX,
            max_subscription_identifiers_len: usize::MAX,
        }
    }
}
```

- [ ] **Step 1.4: Run the tests to confirm they pass**

Run:
`cargo test -p sansio-mqtt-v5-types --test property_compatibility settings_`

Expected: both `settings_default` and `settings_unlimited` PASS.

- [ ] **Step 1.5: Run the full types-crate test suite**

Run: `cargo test -p sansio-mqtt-v5-types`

Expected: all existing tests still pass. If the protocol crate has a constructor
that uses struct-update syntax or builder chains relying on known fields, some
tests might need tweaking in later tasks — but the types crate alone should
pass.

- [ ] **Step 1.6: Commit**

```bash
git add crates/sansio-mqtt-v5-types/src/parser/mod.rs \
        crates/sansio-mqtt-v5-types/tests/property_compatibility.rs
git commit -m "feat(parser): add max_subscription_identifiers_len setting

Mirrors max_user_properties_len. Default 32, unlimited = usize::MAX."
```

---

### Task 2: `TooManySubscriptionIdentifiersError` type

**Files:**

- Modify: `crates/sansio-mqtt-v5-types/src/types/properties.rs`

- [ ] **Step 2.1: Extend the marker-trait regression test**

Open `crates/sansio-mqtt-v5-types/src/types/properties.rs`. In the
`marker_trait_guards` module, inside `marker_errors_are_not_ordered_or_hashed`,
add three asserts for the new error, and add the three `impl MustNot...` lines
above. The whole module should read:

```rust
#[cfg(test)]
mod marker_trait_guards {
    use super::*;

    trait MustNotImplementHash {}
    impl<T: core::hash::Hash> MustNotImplementHash for T {}

    trait MustNotImplementOrd {}
    impl<T: Ord> MustNotImplementOrd for T {}

    trait MustNotImplementPartialOrd {}
    impl<T: PartialOrd> MustNotImplementPartialOrd for T {}

    impl MustNotImplementHash for TooManyUserPropertiesError {}
    impl MustNotImplementOrd for TooManyUserPropertiesError {}
    impl MustNotImplementPartialOrd for TooManyUserPropertiesError {}

    impl MustNotImplementHash for MissingAuthenticationMethodError {}
    impl MustNotImplementOrd for MissingAuthenticationMethodError {}
    impl MustNotImplementPartialOrd for MissingAuthenticationMethodError {}

    impl MustNotImplementHash for TooManySubscriptionIdentifiersError {}
    impl MustNotImplementOrd for TooManySubscriptionIdentifiersError {}
    impl MustNotImplementPartialOrd for TooManySubscriptionIdentifiersError {}

    fn assert_not_hash<T: MustNotImplementHash>() {}
    fn assert_not_ord<T: MustNotImplementOrd>() {}
    fn assert_not_partial_ord<T: MustNotImplementPartialOrd>() {}

    #[test]
    fn marker_errors_are_not_ordered_or_hashed() {
        assert_not_hash::<TooManyUserPropertiesError>();
        assert_not_ord::<TooManyUserPropertiesError>();
        assert_not_partial_ord::<TooManyUserPropertiesError>();

        assert_not_hash::<MissingAuthenticationMethodError>();
        assert_not_ord::<MissingAuthenticationMethodError>();
        assert_not_partial_ord::<MissingAuthenticationMethodError>();

        assert_not_hash::<TooManySubscriptionIdentifiersError>();
        assert_not_ord::<TooManySubscriptionIdentifiersError>();
        assert_not_partial_ord::<TooManySubscriptionIdentifiersError>();
    }
}
```

- [ ] **Step 2.2: Run the failing test**

Run: `cargo build -p sansio-mqtt-v5-types --tests`

Expected: compile error
`cannot find type 'TooManySubscriptionIdentifiersError' in this scope`.

- [ ] **Step 2.3: Add the error type and the `PropertiesError` variant**

In the same file, add the new error immediately after
`TooManyUserPropertiesError` (around line 86):

```rust
#[derive(Debug, PartialEq, Eq, Clone, Copy, Error)]
#[error("The number of subscription identifiers exceeds the maximum allowed")]
#[repr(transparent)]
pub struct TooManySubscriptionIdentifiersError;
```

Then extend the `PropertiesError` enum (currently lines 97–107) with a new
variant:

```rust
#[derive(Debug, PartialEq, Eq, Clone, Copy, Error)]
pub enum PropertiesError {
    #[error(transparent)]
    DuplicatedProperty(#[from] DuplicatedPropertyError),
    #[error(transparent)]
    TooManyUserProperties(#[from] TooManyUserPropertiesError),
    #[error(transparent)]
    TooManySubscriptionIdentifiers(#[from] TooManySubscriptionIdentifiersError),
    #[error(transparent)]
    MissingAuthenticationMethod(#[from] MissingAuthenticationMethodError),
    #[error(transparent)]
    UnsupportedProperty(#[from] UnsupportedPropertyError),
}
```

- [ ] **Step 2.4: Run the test to verify it passes**

Run:
`cargo test -p sansio-mqtt-v5-types marker_errors_are_not_ordered_or_hashed`

Expected: PASS.

- [ ] **Step 2.5: Commit**

```bash
git add crates/sansio-mqtt-v5-types/src/types/properties.rs
git commit -m "feat(types): add TooManySubscriptionIdentifiersError variant"
```

---

### Task 3: Convert `PublishProperties.subscription_identifier` to `Vec`

This is a single coherent type-system change: the wire type, parser, encoder,
and three test files must all move together before the types crate compiles
again. Treat Steps 3.1–3.6 as one atomic unit of work that concludes in a single
commit.

**Files:**

- Modify: `crates/sansio-mqtt-v5-types/src/types/publish.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/parser/publish.rs`
- Modify: `crates/sansio-mqtt-v5-types/src/encoder/publish.rs`
- Modify: `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs`
- Modify: `crates/sansio-mqtt-v5-types/tests/mirror_encode_and_parse.rs`

- [ ] **Step 3.1: Rename the field in `PublishProperties`**

Open `crates/sansio-mqtt-v5-types/src/types/publish.rs`. Change line 67 from:

```rust
    pub subscription_identifier: Option<NonZero<u64>>,
```

to:

```rust
    /// Zero or more Subscription Identifiers per [MQTT-3.3.2.3.8]. An empty
    /// `Vec` means the property is absent on the wire.
    pub subscription_identifiers: Vec<NonZero<u64>>,
```

- [ ] **Step 3.2: Update the parser to push instead of reject duplicates**

Open `crates/sansio-mqtt-v5-types/src/parser/publish.rs`. Replace the entire
`Property::SubscriptionIdentifier(value)` arm (currently lines 165–174) with:

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

Note: `property_type` is bound earlier in the match scope; it's unused in this
arm (same pattern as the `UserProperty` arm).

- [ ] **Step 3.3: Update the encoder to iterate the Vec**

Open `crates/sansio-mqtt-v5-types/src/encoder/publish.rs`. Replace lines 20–22
(the current `Option`-based mapping) and the tuple position at line 38 so the
whole `encode` implementation reads:

```rust
    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let payload_format_indicator = self
            .payload_format_indicator
            .map(Property::PayloadFormatIndicator);
        let message_expiry_interval = self
            .message_expiry_interval
            .map(Property::MessageExpiryInterval);
        let topic_alias = self.topic_alias.map(Property::TopicAlias);
        let response_topic = self.response_topic.clone().map(Property::ResponseTopic);
        let correlation_data = self.correlation_data.clone().map(Property::CorrelationData);
        let subscription_identifiers = encode::combinators::Iter::new(
            self.subscription_identifiers
                .iter()
                .copied()
                .map(Property::SubscriptionIdentifier),
        );
        let user_properties = encode::combinators::Iter::new(
            self.user_properties
                .iter()
                .cloned()
                .map(|(k, v)| Property::UserProperty(k, v)),
        );
        let content_type = self.content_type.clone().map(Property::ContentType);

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            payload_format_indicator,
            message_expiry_interval,
            topic_alias,
            response_topic,
            correlation_data,
            user_properties,
            subscription_identifiers,
            content_type,
        ))
        .encode(encoder)
    }
```

(The only substantive changes are the new `subscription_identifiers` binding and
the tuple element name — ordering within the tuple is preserved so on-the-wire
property order is unchanged when a single identifier is present.)

- [ ] **Step 3.4: Update the "all valid properties" roundtrip test**

Open `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs`. In
`publish_with_all_valid_properties_roundtrip` (around line 313), replace line
329:

```rust
            subscription_identifier: NonZero::new(42),
```

with:

```rust
            subscription_identifiers: vec![NonZero::new(42).unwrap()],
```

The SUBSCRIBE counterpart (line 352) stays unchanged.

- [ ] **Step 3.5: Invert the previously-invalid repeated-identifiers test and
      add multi-identifier roundtrip coverage**

Open `crates/sansio-mqtt-v5-types/tests/mirror_encode_and_parse.rs`.

**(a)** Delete the entire
`assert_that_parsing_an_invalid_field_on_publish_fails` function, including the
`#[rstest::rstest]` attribute and the single
`#[case::to_repeated_subscription_identifiers_property(…)]` — currently lines
223–244. The byte sequence is spec-legal and must no longer be asserted to fail;
that case is re-added as a success case in step (c). The function had only this
one case, so deleting the whole thing avoids a dead parameterized function with
zero invocations. (If at the time of editing another `#[case::…]` has been added
to this function, preserve the function and only remove the
`to_repeated_subscription_identifiers_property` case attribute.)

**(b)** Update the existing `subscription_identifier: NonZero::new(120)` line
(around line 849) inside
`assert_that_different_packets_can_be_decoded_and_encoded` to:

```rust
            subscription_identifiers: vec![NonZero::new(120).unwrap()],
```

**(c)** Add a new `#[case::…]` to
`assert_that_different_packets_can_be_decoded_and_encoded` exercising two
identifiers. Paste the following immediately before the
`fn assert_that_different_packets_can_be_decoded_and_encoded(…)` function
signature (and directly after the existing last `#[case::…]` block):

```rust
#[case::publish_with_multiple_subscription_identifiers(
    vec![
        61, 22, // Header: PUBLISH with dup=1, qos=ExactlyOnce, retain=1
        0, 4, // Topic length
        116, 101, 115, 116, // Topic ("test")
        0, 10, // Packet ID
        9,  // properties length
        1, 0, // PayloadFormatIndicator = Unspecified
        11, 1, // SubscriptionIdentifier = 1
        11, 255, 255, 255, 127, // SubscriptionIdentifier = 268_435_455 (VBI max)
        116, 101, 115, 116, // Payload ("test")
    ],
    ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: NonZero::new(10).unwrap(),
            qos: GuaranteedQoS::ExactlyOnce,
            dup: true,
        },
        retain: true,
        topic: Topic::new("test"),
        payload: Payload::new([116, 101, 115, 116].as_slice()),
        properties: PublishProperties {
            payload_format_indicator: Some(FormatIndicator::Unspecified),
            message_expiry_interval: None,
            topic_alias: None,
            response_topic: None,
            correlation_data: None,
            user_properties: vec![],
            subscription_identifiers: vec![
                NonZero::new(1).unwrap(),
                NonZero::new(268_435_455).unwrap(),
            ],
            content_type: None,
        },
    })
)]
```

This exercises both parse-multi and encode-multi, preserving the order given in
the `Vec`.

- [ ] **Step 3.6: Build and run the types-crate test suite**

Run:
`cargo build -p sansio-mqtt-v5-types --tests && cargo test -p sansio-mqtt-v5-types`

Expected: all green, including the new multi-identifier case and the
already-updated roundtrip tests. If any test fails, stop and diagnose — do not
proceed.

- [ ] **Step 3.7: Commit**

```bash
git add crates/sansio-mqtt-v5-types/src/types/publish.rs \
        crates/sansio-mqtt-v5-types/src/parser/publish.rs \
        crates/sansio-mqtt-v5-types/src/encoder/publish.rs \
        crates/sansio-mqtt-v5-types/tests/property_compatibility.rs \
        crates/sansio-mqtt-v5-types/tests/mirror_encode_and_parse.rs
git commit -m "feat(types): PUBLISH carries Vec<NonZero<u64>> subscription identifiers

Implements [MQTT-3.3.2.3.8]: a published message can match multiple
subscriptions and must carry one Subscription Identifier property per
match. Parser appends (bounded by max_subscription_identifiers_len);
encoder emits one property per Vec entry."
```

---

### Task 4: Parser bound enforcement test

**Files:**

- Modify: `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs`

- [ ] **Step 4.1: Add two new tests**

Open `crates/sansio-mqtt-v5-types/tests/property_compatibility.rs`. Append the
following two tests at the end of the file (after `settings_unlimited` is a good
location):

```rust
#[test]
fn publish_parser_rejects_subscription_identifiers_exceeding_bound() {
    // 3 subscription identifiers inside one PUBLISH, each encoded as `0x0B 0x01`
    // (property id 11, VBI value 1). With max_subscription_identifiers_len = 2,
    // the third identifier must trigger TooManySubscriptionIdentifiersError.
    let bytes = vec![
        0x30, 17, // Header: PUBLISH, qos=0, retain=0; remaining length = 17
        0, 4, // Topic length
        116, 101, 115, 116, // Topic ("test")
        6,    // properties length
        11, 1, // SubscriptionIdentifier = 1
        11, 1, // SubscriptionIdentifier = 1
        11, 1, // SubscriptionIdentifier = 1
        116, 101, 115, 116, // Payload ("test")
    ];
    let settings = ParserSettings {
        max_subscription_identifiers_len: 2,
        ..ParserSettings::default()
    };
    ControlPacket::parser::<_, ContextError, ContextError>(&settings)
        .parse(&bytes[..])
        .unwrap_err();
}

#[test]
fn publish_parser_accepts_many_subscription_identifiers_under_unlimited_settings() {
    // Same 3-identifier PUBLISH accepted under `unlimited` settings.
    let bytes = vec![
        0x30, 17, // Header: PUBLISH, qos=0, retain=0; remaining length = 17
        0, 4, // Topic length
        116, 101, 115, 116, // Topic ("test")
        6,    // properties length
        11, 1, // SubscriptionIdentifier = 1
        11, 1, // SubscriptionIdentifier = 1
        11, 1, // SubscriptionIdentifier = 1
        116, 101, 115, 116, // Payload ("test")
    ];
    let settings = ParserSettings::unlimited();
    let packet = ControlPacket::parser::<_, ContextError, ContextError>(&settings)
        .parse(&bytes[..])
        .unwrap();
    let ControlPacket::Publish(publish) = packet else {
        panic!("expected PUBLISH, got {packet:?}");
    };
    assert_eq!(publish.properties.subscription_identifiers.len(), 3);
}
```

Ensure `ControlPacket`, `ParserSettings`, `ContextError`, and `Parser` are
already imported at the top of the file (they are — see the existing test
fixture uses them).

- [ ] **Step 4.2: Run the new tests**

Run:
`cargo test -p sansio-mqtt-v5-types --test property_compatibility publish_parser_`

Expected: both tests PASS.

- [ ] **Step 4.3: Run the full types-crate suite as a sanity check**

Run: `cargo test -p sansio-mqtt-v5-types`

Expected: all green.

- [ ] **Step 4.4: Commit**

```bash
git add crates/sansio-mqtt-v5-types/tests/property_compatibility.rs
git commit -m "test(parser): bound enforcement for subscription identifiers"
```

---

### Task 5: Propagate `Vec` shape through `BrokerMessage`

Another atomic type-system change: the protocol crate won't compile until the
`BrokerMessage` field, the `proto.rs` mapping, the `tokio` example, and the
protocol test all move together.

**Files:**

- Modify: `crates/sansio-mqtt-v5-protocol/src/types.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/src/proto.rs`
- Modify: `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`
- Modify: `crates/sansio-mqtt-v5-tokio/examples/cli.rs`

- [ ] **Step 5.1: Update `BrokerMessage`**

Open `crates/sansio-mqtt-v5-protocol/src/types.rs`. Replace line 141:

```rust
    pub subscription_identifier: Option<NonZero<u64>>,
```

with:

```rust
    /// Zero or more Subscription Identifiers per [MQTT-3.3.2.3.8]. An empty
    /// `Vec` means no subscription identifier was attached on the wire.
    pub subscription_identifiers: Vec<NonZero<u64>>,
```

**Leave `SubscribeOptions.subscription_identifier` at line 162 unchanged** —
SUBSCRIBE is 0..1 per [MQTT-3.8.2.1.2].

- [ ] **Step 5.2: Update the `BrokerMessage` construction site in `proto.rs`**

Open `crates/sansio-mqtt-v5-protocol/src/proto.rs`. Replace line 1135:

```rust
            subscription_identifier: properties.subscription_identifier,
```

with:

```rust
            subscription_identifiers: properties.subscription_identifiers,
```

(Nothing else in the `From<Publish> → BrokerMessage` mapping needs to change —
the field now just moves a `Vec`.)

- [ ] **Step 5.3: Outbound PUBLISH construction also needs the renamed field**

In the same `proto.rs`, around line 1283, there is a
`PublishProperties { … subscription_identifier: None, … }` literal for the
outbound side. Replace that line with:

```rust
                    subscription_identifiers: Vec::new(),
```

The client does not populate subscription identifiers on outbound PUBLISH
(they're server-assigned), so the empty `Vec` preserves current behavior.

- [ ] **Step 5.4: Update the single `BrokerMessage` assert in the protocol
      test**

Open `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`. At line 580,
replace:

```rust
            assert_eq!(message.subscription_identifier, None);
```

with:

```rust
            assert!(message.subscription_identifiers.is_empty());
```

All other `subscription_identifier` occurrences in this file are on
`SubscribeOptions` and must stay unchanged.

- [ ] **Step 5.5: Add a regression test for multi-identifier inbound PUBLISH**

In the same test file, immediately after
`inbound_publish_qos0_is_forwarded_to_user_queue` (the function containing the
assertion changed in Step 5.4; its body ends around line 586), append this new
test:

```rust
#[test]
fn inbound_publish_multiple_subscription_identifiers_surface_to_user() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let publish_topic = Topic::try_new("t/multi").expect("valid topic");
    let publish_payload = Payload::new(b"hello".as_slice());
    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: publish_payload.clone(),
        topic: publish_topic.clone(),
        properties: PublishProperties {
            subscription_identifiers: vec![
                NonZero::new(7).unwrap(),
                NonZero::new(42).unwrap(),
            ],
            ..PublishProperties::default()
        },
    });

    assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));

    match client.poll_read() {
        Some(UserWriteOut::ReceivedMessage(message)) => {
            assert_eq!(message.topic, publish_topic);
            assert_eq!(message.payload, publish_payload);
            assert_eq!(
                message.subscription_identifiers,
                vec![NonZero::new(7).unwrap(), NonZero::new(42).unwrap()]
            );
        }
        other => panic!("expected received message, got {other:?}"),
    }
}
```

`NonZero`, `Topic`, `Payload`, `PublishProperties`, `Publish`, `PublishKind`,
`ControlPacket`, `ConnAck`, `ConnAckKind`, `ConnAckProperties`,
`ConnackReasonCode`, `Client`, `DriverEventIn`, `UserWriteOut`, and
`encode_packet` are all already imported at the top of `client_protocol.rs`
(verify against lines 1–18 of that file).

- [ ] **Step 5.6: Update the tokio example**

Open `crates/sansio-mqtt-v5-tokio/examples/cli.rs`. The occurrence at line 63 is
inside a `SubscribeOptions { … }` literal — **do not change it** (SUBSCRIBE
stays `Option`).

Search the file for any other `subscription_identifier:` occurrence that
references a `BrokerMessage` construction. (Run
`grep -n subscription_identifier crates/sansio-mqtt-v5-tokio/examples/cli.rs` to
verify.) If none exist, skip this step.

- [ ] **Step 5.7: Build and run the full workspace**

Run: `cargo build --workspace --tests && cargo test --workspace`

Expected: all green, including the new
`inbound_publish_multiple_subscription_identifiers_surface_to_user` test.

If any protocol or tokio test fails, diagnose before proceeding — the most
likely cause is a missed `SubscribeOptions`/`BrokerMessage` distinction.

- [ ] **Step 5.8: Commit**

```bash
git add crates/sansio-mqtt-v5-protocol/src/types.rs \
        crates/sansio-mqtt-v5-protocol/src/proto.rs \
        crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs \
        crates/sansio-mqtt-v5-tokio/examples/cli.rs
git commit -m "feat(protocol): BrokerMessage carries Vec<NonZero<u64>> subscription identifiers

Propagates the wire-level [MQTT-3.3.2.3.8] shape through the protocol
driver. SubscribeOptions remains Option (SUBSCRIBE is 0..1)."
```

---

### Task 6: Workspace-wide checks

**Files:** none modified.

- [ ] **Step 6.1: Formatting**

Run: `cargo fmt --all -- --check`

Expected: no diffs. If there are diffs, run `cargo fmt --all` and commit them as
a separate `chore(fmt)` commit.

- [ ] **Step 6.2: Clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: no warnings. If any clippy warning surfaces on code touched by this
plan, fix it inline and add to the nearest relevant commit (amend or a small
follow-up commit).

- [ ] **Step 6.3: Full test suite**

Run: `cargo test --workspace`

Expected: all green.

- [ ] **Step 6.4: Sanity-scan downstream for missed sites**

Run this grep to confirm there are no lingering `subscription_identifier`
references pointing at `PublishProperties` or `BrokerMessage`:

```bash
grep -rn 'subscription_identifier\b' crates/ --include='*.rs'
```

Review each hit. The **only** remaining singular-form occurrences should be:

- `SubscribeProperties.subscription_identifier` in
  `crates/sansio-mqtt-v5-types/src/types/subscribe.rs`
- `SubscribeOptions.subscription_identifier` in
  `crates/sansio-mqtt-v5-protocol/src/types.rs`
- All `SubscribeOptions { … subscription_identifier: … }` literals in
  `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` and
  `crates/sansio-mqtt-v5-tokio/examples/cli.rs`
- Parser/encoder code for `Property::SubscriptionIdentifier` (that enum variant
  is singular and stays)
- `allow_subscription_identifiers` in
  `crates/sansio-mqtt-v5-protocol/src/{types,proto}.rs` and related
  protocol-layer field names like `effective_subscription_identifiers_available`
  / `negotiated_subscription_identifiers_available` /
  `subscription_identifiers_available` — unrelated CONNACK-negotiation state

If any other `subscription_identifier` (singular) remains in PUBLISH /
BrokerMessage code paths, fix it before closing this task.

- [ ] **Step 6.5: No commit needed if all checks passed**

If Step 6.1 produced formatting diffs, they were already committed in Step 6.1.
Otherwise there is nothing new to commit. If any clippy fix was required, commit
it with:

```bash
git commit -am "chore: clippy fixes for subscription identifiers change"
```

---

## Self-Review (performed by plan author)

**1. Spec coverage:**

- Wire type change in `PublishProperties` → Task 3.
- Parser append + bound → Task 1 (setting), Task 2 (error), Task 3 (parser arm),
  Task 4 (bound test).
- Encoder iterate → Task 3.3.
- `ParserSettings.max_subscription_identifiers_len` with 32 default / usize::MAX
  unlimited → Task 1.
- `BrokerMessage` field change + `proto.rs` propagation → Task 5.
- `SubscribeOptions` unchanged → called out in Task 5.1 and 5.6.
- Invert `to_repeated_subscription_identifiers_property` → Task 3.5 (a).
- Empty/single/multi roundtrip → Task 3 covers single (preserved in
  `publish_with_all_valid_properties_roundtrip`), multi (new
  `publish_with_multiple_subscription_identifiers` case). Empty Vec path is
  exercised implicitly by every other `PublishProperties::default()` roundtrip
  test in both `property_compatibility.rs` and `mirror_encode_and_parse.rs`
  (they all construct `subscription_identifiers` via `Default` = empty `Vec`).
  An explicit assertion would be redundant.
- Bound enforcement both ways → Task 4.1 (rejection + unlimited acceptance).
- Protocol-layer regression for multi-identifier PUBLISH → Task 5.5.
- `cargo fmt`/`cargo clippy`/full test run → Task 6.
- Rustdoc update citing [MQTT-3.3.2.3.8] → Task 3.1 (doc comment on the new
  field) + Task 5.1 (doc comment on `BrokerMessage`).

**2. Placeholder scan:** no TBD/TODO/"similar to". All code blocks contain the
exact text the editor should write.

**3. Type consistency:**

- `subscription_identifiers: Vec<NonZero<u64>>` used consistently in
  `PublishProperties` and `BrokerMessage`.
- `subscription_identifier: Option<NonZero<u64>>` (singular) retained
  consistently in `SubscribeProperties` and `SubscribeOptions`.
- `Property::SubscriptionIdentifier(NonZero<u64>)` enum variant is untouched.
- `TooManySubscriptionIdentifiersError` and `max_subscription_identifiers_len`
  names match spec and are used consistently in parser, test, and error enum.
