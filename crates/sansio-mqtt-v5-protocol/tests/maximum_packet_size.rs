//! The Maximum Packet Size a client advertises in CONNECT must also bound what
//! its own parser accepts.
//!
//! [MQTT-3.1.2-24] The Client uses Maximum Packet Size to inform the Server
//! that it will not process packets exceeding this limit, so the parser has to
//! enforce the same number the CONNECT advertised — including while the
//! handshake is still in flight, which is exactly when CONNACK and AUTH
//! arrive.
//!
//! [Behaviour change] `maximum_packet_size` is now `ClientSettings`-only:
//! `Command::Connect` no longer carries a per-connect override, so the two
//! tests that used to exercise "caller-supplied value overrides/competes with
//! the local policy" were deleted (see the migration report) rather than
//! rewritten.

use bytes::Bytes;
use bytestring::ByteString;
use core::num::NonZero;
use core::time::Duration;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnAckProperties;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::ParserSettings;
use sansio_mqtt_v5_types::Utf8String;

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

/// A CONNACK padded with user properties so it comfortably exceeds 32 bytes.
fn oversized_connack() -> ControlPacket {
    let pad = Utf8String::try_from("padding-value-0123456789").expect("valid utf8");
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .properties(
                ConnAckProperties::builder()
                    .user_properties((0..4).map(|_| (pad.clone(), pad.clone())).collect())
                    .build(),
            )
            .build(),
    )
}

fn small_connack() -> ControlPacket {
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .build(),
    )
}

fn connect_options() -> ConnectOptions {
    ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .build()
}

/// `ClientSettings` builder helper: `maximum_packet_size` is the only field
/// this file varies; the three mandatory counters are set to the library's
/// own defaults (`ClientSettings` is `#[non_exhaustive]`, so struct-update
/// syntax is unavailable here).
fn settings_with_maximum_packet_size(value: Option<NonZero<u32>>) -> ClientSettings {
    ClientSettings::builder()
        .maybe_maximum_packet_size(value)
        .max_user_properties(32)
        .max_subscription_identifiers(32)
        .max_subscriptions(32)
        .build()
}

fn read(client: &mut Client<Duration>, packet: &ControlPacket) -> Result<(), Error> {
    client.handle_read(IncomingData {
        bytes: &encode_packet(packet),
        received_at: Duration::ZERO,
    })
}

/// Drives a client up to the point where it is awaiting CONNACK.
fn connecting_client(maximum_packet_size: Option<NonZero<u32>>) -> Client<Duration> {
    let mut client =
        Client::<Duration>::new(settings_with_maximum_packet_size(maximum_packet_size));

    assert_eq!(
        client.handle_write(Command::Connect(connect_options())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some(), "CONNECT should be queued");

    client
}

/// `ClientSettings::maximum_packet_size` bounds the parser.
#[test]
fn maximum_packet_size_from_settings_bounds_the_parser() {
    let mut client = connecting_client(Some(NonZero::new(32).expect("non-zero")));

    assert_eq!(
        read(&mut client, &oversized_connack()),
        Err(Error::MalformedPacket),
        "a CONNACK larger than the advertised Maximum Packet Size must be rejected"
    );
}

/// The clamp must not reject packets that are actually within the limit.
#[test]
fn connack_within_maximum_packet_size_is_accepted() {
    let mut client = connecting_client(Some(NonZero::new(32).expect("non-zero")));

    assert_eq!(read(&mut client, &small_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

/// When `maximum_packet_size` is `None`, the parser still caps Remaining
/// Length at the `sansio-mqtt-v5-types` default (1 MiB) rather than being
/// truly unbounded.
#[test]
fn absent_maximum_packet_size_caps_parser_at_default_one_mebibyte() {
    assert_eq!(
        ParserSettings::default().max_remaining_bytes,
        1024 * 1024,
        "the v5-types default Remaining Length cap must be 1 MiB"
    );

    let mut client = connecting_client(None);

    // A small CONNACK is comfortably under the default cap.
    assert_eq!(read(&mut client, &oversized_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    // Prove the cap itself: a PUBLISH fixed header whose Remaining Length
    // variable byte integer (`0x81 0x80 0x40`) decodes to 1 MiB + 1
    // (1_048_577), one byte over the default cap. The parser must reject it
    // from the fixed header alone, before any packet body is needed.
    let oversized_publish_header = Bytes::from_static(&[0x30, 0x81, 0x80, 0x40]);
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: &oversized_publish_header,
            received_at: Duration::ZERO,
        }),
        Err(Error::MalformedPacket),
        "Remaining Length over the default 1 MiB cap must be rejected"
    );
}
