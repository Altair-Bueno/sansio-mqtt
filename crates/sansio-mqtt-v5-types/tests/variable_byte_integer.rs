//! Tests for the Variable Byte Integer codec
//! ([§1.5.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901011)):
//! the four-byte upper bound when encoding and decoding, including
//! Subscription Identifiers that do not fit.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers fail the test on setup errors by design"
)]

use alloc::vec::Vec;
use core::num::NonZero;

use encode::Encodable as _;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::EncodeError;
use sansio_mqtt_v5_types::ParserSettings;
use sansio_mqtt_v5_types::Subscribe;
use sansio_mqtt_v5_types::SubscribeProperties;
use sansio_mqtt_v5_types::Subscription;
use sansio_mqtt_v5_types::Utf8String;
use sansio_mqtt_v5_types::variable_byte_integer;
use winnow::Parser as _;
use winnow::error::ContextError;

extern crate alloc;

fn subscribe_with_identifier(identifier: u64) -> ControlPacket {
    ControlPacket::Subscribe(
        Subscribe::builder()
            .packet_id(NonZero::new(1).expect("1 is non-zero"))
            .subscription(
                Subscription::builder()
                    .topic_filter(Utf8String::try_new("a/b").unwrap())
                    .build(),
            )
            .properties(
                SubscribeProperties::builder()
                    .subscription_identifier(
                        NonZero::new(identifier).expect("identifier is non-zero"),
                    )
                    .build(),
            )
            .build(),
    )
}

#[test]
fn encoding_the_largest_variable_byte_integer_uses_four_bytes() {
    let mut encoded = Vec::new();
    subscribe_with_identifier(268_435_455)
        .encode(&mut encoded)
        .expect("268_435_455 is the Variable Byte Integer maximum");

    let packet = ControlPacket::parser::<_, ContextError, ContextError>(&ParserSettings::default())
        .parse(&encoded[..])
        .expect("the encoded SUBSCRIBE parses back");
    assert_eq!(packet, subscribe_with_identifier(268_435_455));
}

#[test]
fn encoding_a_value_above_the_variable_byte_integer_maximum_fails() {
    // [MQTT-1.5.5-1] A Variable Byte Integer uses at most four bytes, so
    // 268_435_456 has no valid encoding.
    let mut encoded = Vec::new();
    let result = subscribe_with_identifier(268_435_456).encode(&mut encoded);

    assert!(
        matches!(
            result,
            Err(EncodeError::VariableByteIntegerTooLarge(268_435_456))
        ),
        "unexpected result {result:?}, encoded {encoded:?}"
    );
}

#[test]
fn parsing_the_largest_variable_byte_integer_succeeds() {
    let value = variable_byte_integer::<_, ContextError>
        .parse(&[0xFF, 0xFF, 0xFF, 0x7F][..])
        .expect("four bytes encode at most 268_435_455");

    assert_eq!(value, 268_435_455_u32);
}
