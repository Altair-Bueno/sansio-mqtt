//! Tests for [`DecodeError`] — the concrete parser error type.
//!
//! The point of a concrete error type is that the *classification*
//! survives parsing, so a caller can pick the Reason Code that
//! [§4.13 — Handling errors](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901252)
//! requires for the DISCONNECT it must send. These tests pin that
//! mapping: with an opaque error type every case below would collapse
//! to a single indistinguishable failure.
//!
//! Every fixture is a minimal CONNECT — protocol name `MQTT`, version
//! 5, Clean Start, keep alive 60, empty Client Identifier — differing
//! only in its property section.

use rstest::rstest;
use sansio_mqtt_v5_types::*;
use winnow::Parser;

/// Decodes `bytes`, asserting it fails, and returns the [`DecodeError`].
fn decode_err(bytes: &[u8]) -> DecodeError {
    decode_err_with(bytes, &ParserSettings::default())
}

fn decode_err_with(bytes: &[u8], settings: &ParserSettings) -> DecodeError {
    ControlPacket::parser::<_, DecodeError, DecodeError>(settings)
        .parse(bytes)
        .expect_err("fixture is expected to be an invalid packet")
        .into_inner()
}

/// The same CONNECT with an empty property section MUST parse, so every
/// failure below is attributable to the property under test rather than
/// to the framing.
#[test]
fn baseline_connect_is_valid() {
    let bytes = [16, 13, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 0, 0, 0];
    let settings = ParserSettings::default();

    let packet = ControlPacket::parser::<_, DecodeError, DecodeError>(&settings)
        .parse(&bytes[..])
        .expect("a CONNECT with no properties is valid");
    assert!(matches!(packet, ControlPacket::Connect(_)));
}

/// A property identifier that is not valid for its packet type is a
/// Malformed Packet: "A Control Packet which contains an Identifier
/// which is not valid for its packet type … is a Malformed Packet"
/// ([§2.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901029)).
#[rstest]
#[case::content_type(&[16, 20, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 7, 3, 0, 4, 116, 101, 115, 116, 0, 0])]
#[case::reason_string(&[16, 20, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 7, 31, 0, 4, 116, 101, 115, 116, 0, 0])]
#[case::topic_alias(&[16, 16, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 3, 35, 0, 100, 0, 0])]
fn property_not_allowed_for_packet_is_malformed(#[case] bytes: &[u8]) {
    let error = decode_err(bytes);

    assert!(
        matches!(
            error,
            DecodeError::Properties(PropertiesError::UnsupportedProperty(_))
        ),
        "expected UnsupportedProperty, got {error:?}"
    );
    assert_eq!(
        error.disconnect_reason_code(),
        DisconnectReasonCode::MalformedPacket
    );
    assert!(error.is_malformed_packet());
}

/// A property repeated when it may appear at most once is a Protocol
/// Error, e.g. "It is a Protocol Error to include the Session Expiry
/// Interval more than once"
/// ([§3.1.2.11.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901048)).
#[rstest]
#[case::session_expiry_interval(&[16, 23, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 10, 17, 0, 0, 0, 100, 17, 0, 0, 0, 100, 0, 0])]
#[case::receive_maximum(&[16, 19, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 6, 33, 0, 100, 33, 0, 100, 0, 0])]
fn duplicated_property_is_protocol_error(#[case] bytes: &[u8]) {
    let error = decode_err(bytes);

    assert!(
        matches!(
            error,
            DecodeError::Properties(PropertiesError::DuplicatedProperty(_))
        ),
        "expected DuplicatedProperty, got {error:?}"
    );
    assert_eq!(
        error.disconnect_reason_code(),
        DisconnectReasonCode::ProtocolError
    );
    assert!(!error.is_malformed_packet());
}

/// An identifier outside the set defined by §2.2.2.2 is a Malformed
/// Packet. `0x7F` is not an assigned property identifier.
#[test]
fn unknown_property_identifier_is_malformed() {
    let error = decode_err(&[16, 15, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 2, 127, 1, 0, 0]);

    assert!(
        matches!(error, DecodeError::InvalidPropertyType(_)),
        "expected InvalidPropertyType, got {error:?}"
    );
    assert_eq!(
        error.disconnect_reason_code(),
        DisconnectReasonCode::MalformedPacket
    );
}

/// Authentication Data without Authentication Method is a Protocol
/// Error ([§3.1.2.11.10](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901056)).
#[test]
fn authentication_data_without_method_is_protocol_error() {
    let error = decode_err(&[
        16, 20, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 7, 22, 0, 4, 1, 2, 3, 4, 0, 0,
    ]);

    assert!(
        matches!(
            error,
            DecodeError::Properties(PropertiesError::MissingAuthenticationMethod(_))
        ),
        "expected MissingAuthenticationMethod, got {error:?}"
    );
    assert_eq!(
        error.disconnect_reason_code(),
        DisconnectReasonCode::ProtocolError
    );
}

/// Control Packet Type 0 is Reserved / Forbidden
/// ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)),
/// so it cannot be parsed at all.
#[test]
fn reserved_packet_type_is_malformed() {
    let error = decode_err(&[0, 0]);

    assert!(
        matches!(error, DecodeError::InvalidControlPacketType(_)),
        "expected InvalidControlPacketType, got {error:?}"
    );
    assert_eq!(
        error.disconnect_reason_code(),
        DisconnectReasonCode::MalformedPacket
    );
}

/// A caller-configured ceiling is not a spec violation, so it maps to
/// Implementation specific error rather than Malformed / Protocol Error.
///
/// The fixture carries two User Properties, which is valid by default
/// and rejected only once the limit is lowered to one.
#[test]
fn exceeding_a_parser_limit_is_implementation_specific() {
    let bytes: &[u8] = &[
        16, 27, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 14, 38, 0, 1, 97, 0, 1, 98, 38, 0, 1, 99, 0, 1,
        100, 0, 0,
    ];
    let settings = ParserSettings {
        max_user_properties_len: 1,
        ..ParserSettings::default()
    };

    let error = decode_err_with(bytes, &settings);

    assert!(
        matches!(
            error,
            DecodeError::Properties(PropertiesError::TooManyUserProperties(_))
        ),
        "expected TooManyUserProperties, got {error:?}"
    );
    assert_eq!(
        error.disconnect_reason_code(),
        DisconnectReasonCode::ImplementationSpecificError
    );

    // The same packet is fine once the ceiling admits both entries.
    let permissive = ParserSettings::default();
    ControlPacket::parser::<_, DecodeError, DecodeError>(&permissive)
        .parse(bytes)
        .expect("two user properties are valid under the default limit");
}

/// The whole reason this type exists: two failures that an opaque error
/// type reports identically must be distinguishable here, and must
/// produce different Reason Codes on the wire.
#[test]
fn malformed_and_protocol_errors_are_distinguishable() {
    let malformed = decode_err(&[
        16, 20, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 7, 3, 0, 4, 116, 101, 115, 116, 0, 0,
    ]);
    let protocol = decode_err(&[
        16, 19, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 6, 33, 0, 100, 33, 0, 100, 0, 0,
    ]);

    assert_ne!(malformed, protocol);
    assert_eq!(
        malformed.disconnect_reason_code(),
        DisconnectReasonCode::MalformedPacket
    );
    assert_eq!(
        protocol.disconnect_reason_code(),
        DisconnectReasonCode::ProtocolError
    );
}

/// A well-formed packet still decodes with `DecodeError` as the error
/// type — the new type must not narrow what parses successfully.
#[test]
fn valid_packet_still_parses() {
    let settings = ParserSettings::default();
    let packet = ControlPacket::parser::<_, DecodeError, DecodeError>(&settings)
        .parse(&[192, 0][..])
        .expect("PINGREQ is a valid packet");

    assert!(matches!(packet, ControlPacket::PingReq(_)));
}
