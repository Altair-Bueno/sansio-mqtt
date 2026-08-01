//! The Maximum Packet Size a client advertises in CONNECT must also bound what
//! its own parser accepts.
//!
//! [MQTT-3.1.2-24] The Client uses Maximum Packet Size to inform the Server
//! that it will not process packets exceeding this limit, so the parser has to
//! enforce the same number the CONNECT advertised — including while the
//! handshake is still in flight, which is exactly when CONNACK and AUTH arrive.

use bytes::Bytes;
use core::num::NonZero;
use core::time::Duration;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_protocol::ConnectionOptions;
use sansio_mqtt_v5_protocol::DriverEventIn;
use sansio_mqtt_v5_protocol::Error;
use sansio_mqtt_v5_protocol::IncomingData;
use sansio_mqtt_v5_protocol::UserWriteIn;
use sansio_mqtt_v5_protocol::UserWriteOut;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnAckProperties;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::Utf8String;

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

/// A CONNACK padded with user properties so it comfortably exceeds 32 bytes.
fn oversized_connack() -> ControlPacket {
    let pad = Utf8String::try_from("padding-value-0123456789").expect("valid utf8");
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            user_properties: (0..4).map(|_| (pad.clone(), pad.clone())).collect(),
            ..ConnAckProperties::default()
        },
    })
}

fn small_connack() -> ControlPacket {
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    })
}

/// Drives a client up to the point where it is awaiting CONNACK.
fn connecting_client(
    settings: ClientSettings,
    maximum_packet_size: Option<NonZero<u32>>,
) -> Client<Duration> {
    let mut client = Client::<Duration>::with_settings(settings);

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            maximum_packet_size,
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some(), "CONNECT should be queued");

    client
}

/// With default settings, `max_incoming_packet_size` is `None`, so only the
/// caller-supplied `ConnectionOptions::maximum_packet_size` bounds the parser.
#[test]
fn connection_options_maximum_packet_size_bounds_the_parser() {
    let mut client = connecting_client(
        ClientSettings::default(),
        Some(NonZero::new(32).expect("non-zero")),
    );

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&oversized_connack()),
            received_at: Duration::ZERO,
        }),
        Err(Error::MalformedPacket),
        "a CONNACK larger than the advertised Maximum Packet Size must be rejected"
    );
}

/// Both local policy and the caller's request are present: the smaller wins,
/// and it has to apply before CONNACK is parsed, not one recompute later.
#[test]
fn maximum_packet_size_bounds_the_parser_during_the_handshake() {
    let mut client = connecting_client(
        ClientSettings {
            max_incoming_packet_size: Some(NonZero::new(512).expect("non-zero")),
            ..ClientSettings::default()
        },
        Some(NonZero::new(32).expect("non-zero")),
    );

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&oversized_connack()),
            received_at: Duration::ZERO,
        }),
        Err(Error::MalformedPacket),
        "the smaller of policy and request must bound the parser before CONNACK"
    );
}

/// The clamp must not reject packets that are actually within the limit.
#[test]
fn connack_within_maximum_packet_size_is_still_accepted() {
    let mut client = connecting_client(
        ClientSettings::default(),
        Some(NonZero::new(32).expect("non-zero")),
    );

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&small_connack()),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

/// No Maximum Packet Size anywhere means no extra clamp beyond
/// `ClientSettings`.
#[test]
fn absent_maximum_packet_size_leaves_the_parser_unclamped() {
    let mut client = connecting_client(ClientSettings::default(), None);

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&oversized_connack()),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}
