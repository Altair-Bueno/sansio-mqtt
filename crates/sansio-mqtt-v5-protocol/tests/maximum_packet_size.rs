//! The Maximum Packet Size a client advertises in CONNECT must also bound what
//! its own parser accepts.
//!
//! [MQTT-3.1.2-24] The Client uses Maximum Packet Size to inform the Server
//! that it will not process packets exceeding this limit, so the parser has to
//! enforce the same number the CONNECT advertised — including while the
//! handshake is still in flight, which is exactly when CONNACK and AUTH
//! arrive.
//!
//! The limit is always advertised: there is no "unlimited" setting the parser
//! could silently contradict.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail the test on setup errors by design"
)]

use bytes::Bytes;
use bytestring::ByteString;
use core::num::NonZero;
use core::time::Duration;
use encode::Encodable as _;
use sansio::Protocol as _;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::ReasonCode;
use sansio_mqtt_protocol::Timestamped;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_wire::ConnAck;
use sansio_mqtt_v5_wire::ConnAckKind;
use sansio_mqtt_v5_wire::ConnAckProperties;
use sansio_mqtt_v5_wire::ConnackReasonCode;
use sansio_mqtt_v5_wire::Connect;
use sansio_mqtt_v5_wire::ControlPacket;
use sansio_mqtt_v5_wire::Disconnect;
use sansio_mqtt_v5_wire::DisconnectReasonCode;
use sansio_mqtt_v5_wire::ParserSettings;
use sansio_mqtt_v5_wire::Utf8String;
use winnow::Parser as _;
use winnow::error::ContextError;

/// Stamps a protocol input with `Duration::ZERO`, the instant these tests
/// treat as the start of the connection.
fn at_zero<Value>(value: Value) -> Timestamped<Value, Duration> {
    Timestamped {
        value,
        at: Duration::ZERO,
    }
}

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

/// Default settings with only `maximum_packet_size` changed.
fn settings_with_maximum_packet_size(value: NonZero<u32>) -> ClientSettings {
    ClientSettings::builder().maximum_packet_size(value).build()
}

fn read(client: &mut Client<Duration>, packet: &ControlPacket) -> Result<(), Error> {
    client.handle_read(IncomingData {
        bytes: &encode_packet(packet),
        received_at: Duration::ZERO,
    })
}

/// Asserts the client answered an oversized packet with DISCONNECT 0x95
/// (Packet too large) and reported it to the application.
///
/// MQTT v5.0 §3.1.2.11.4: if a Client receives a packet whose size exceeds
/// its Maximum Packet Size, this is a Protocol Error and the Client uses
/// DISCONNECT with Reason Code 0x95 (Packet too large).
fn assert_disconnected_packet_too_large(client: &mut Client<Duration>) {
    let expected = encode_packet(&ControlPacket::Disconnect(
        Disconnect::builder()
            .reason_code(DisconnectReasonCode::PacketTooLarge)
            .build(),
    ));
    assert_eq!(client.poll_write(), Some(expected));
    assert_eq!(client.poll_event(), Some(DriverAction::CloseSocket));
    assert_eq!(
        client.poll_read(),
        Some(Event::Disconnected(Some(ReasonCode::PacketTooLarge)))
    );
}

/// Drives a client up to the point where it is awaiting CONNACK.
fn connecting_client(maximum_packet_size: NonZero<u32>) -> Client<Duration> {
    connecting_client_with(settings_with_maximum_packet_size(maximum_packet_size)).0
}

/// Like [`connecting_client`], also returning the CONNECT the client sent.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "`ControlPacket` is `#[non_exhaustive]`; every packet but CONNECT fails the test"
)]
fn connecting_client_with(settings: ClientSettings) -> (Client<Duration>, Connect) {
    let mut client = Client::<Duration>::new(settings);

    assert_eq!(
        client.handle_write(at_zero(Command::Connect(connect_options()))),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::OpenSocket)
    ));
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketConnected)),
        Ok(())
    );
    let connect = client.poll_write().expect("CONNECT should be queued");
    let connect =
        match ControlPacket::parser::<_, ContextError, ContextError>(&ParserSettings::default())
            .parse(connect.as_ref())
            .expect("CONNECT should decode")
        {
            ControlPacket::Connect(connect) => connect,
            other => panic!("expected CONNECT, got {other:?}"),
        };

    (client, connect)
}

/// `ClientSettings::maximum_packet_size` bounds the parser.
#[test]
fn maximum_packet_size_from_settings_bounds_the_parser() {
    let mut client = connecting_client(NonZero::new(32).expect("non-zero"));

    assert_eq!(
        read(&mut client, &oversized_connack()),
        Ok(()),
        "a CONNACK larger than the advertised Maximum Packet Size must be rejected"
    );
    assert_disconnected_packet_too_large(&mut client);
}

/// Maximum Packet Size counts the whole packet, fixed header included, so a
/// packet exactly at the limit is accepted.
#[test]
fn packet_exactly_at_maximum_packet_size_is_accepted() {
    let size = u32::try_from(encode_packet(&small_connack()).len()).expect("fits");
    let mut client = connecting_client(NonZero::new(size).expect("non-zero"));

    assert_eq!(read(&mut client, &small_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

/// One byte over the limit is rejected, even though its Remaining Length
/// alone would fit.
#[test]
fn packet_one_byte_over_maximum_packet_size_is_rejected() {
    let size = u32::try_from(encode_packet(&small_connack()).len()).expect("fits");
    let mut client = connecting_client(NonZero::new(size - 1).expect("non-zero"));

    assert_eq!(read(&mut client, &small_connack()), Ok(()));
    assert_disconnected_packet_too_large(&mut client);
}

/// The clamp must not reject packets that are actually within the limit.
#[test]
fn connack_within_maximum_packet_size_is_accepted() {
    let mut client = connecting_client(NonZero::new(32).expect("non-zero"));

    assert_eq!(read(&mut client, &small_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

/// The client always advertises the Maximum Packet Size its parser enforces,
/// so a compliant server never sends it a packet it cannot process
/// [MQTT-3.1.2-24]; by default that is 1 MiB.
#[test]
fn default_maximum_packet_size_is_advertised_and_enforced() {
    let default = ClientSettings::DEFAULT_MAXIMUM_PACKET_SIZE;
    assert_eq!(default.get(), 1024 * 1024);
    assert_eq!(ClientSettings::default().maximum_packet_size, default);

    let (mut client, connect) = connecting_client_with(ClientSettings::default());
    assert_eq!(connect.properties.maximum_packet_size, Some(default));

    assert_eq!(read(&mut client, &small_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    // A PUBLISH fixed header whose Remaining Length (`0x81 0x80 0x40`,
    // 1_048_577) alone exceeds the limit is rejected from the header, before
    // any packet body is needed.
    let oversized_publish_header = Bytes::from_static(&[0x30, 0x81, 0x80, 0x40]);
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: &oversized_publish_header,
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert_disconnected_packet_too_large(&mut client);
}
