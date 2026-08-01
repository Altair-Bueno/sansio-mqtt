//! Session State must survive a process restart.
//!
//! [MQTT-4.1.0-1] When Session Expiry Interval is greater than zero the Session
//! State outlives the Network Connection, so an application that is restarted
//! (or hard-rebooted) has to be able to take the state out of a [`Client`],
//! persist it, and hand it back to a fresh [`Client`] afterwards.

use bytes::Bytes;
use core::num::NonZero;
use core::time::Duration;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientMessage;
use sansio_mqtt_v5_protocol::ClientSession;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_protocol::ConnectionOptions;
use sansio_mqtt_v5_protocol::DriverEventIn;
use sansio_mqtt_v5_protocol::DriverEventOut;
use sansio_mqtt_v5_protocol::IncomingData;
use sansio_mqtt_v5_protocol::OutboundInflightState;
use sansio_mqtt_v5_protocol::UserWriteIn;
use sansio_mqtt_v5_protocol::UserWriteOut;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnAckProperties;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::GuaranteedQoS;
use sansio_mqtt_v5_types::Payload;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::PublishProperties;
use sansio_mqtt_v5_types::Qos;
use sansio_mqtt_v5_types::Topic;
use sansio_mqtt_v5_types::Utf8String;

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

fn topic(name: &str) -> Topic {
    Topic::try_from(Utf8String::try_from(name).expect("valid utf8")).expect("valid topic")
}

fn connect_options() -> ConnectionOptions {
    ConnectionOptions {
        session_expiry_interval: Some(30),
        clean_start: false,
        ..ConnectionOptions::default()
    }
}

/// Drives a client from Start to Connected, answering with `connack_kind`.
fn drive_to_connected(client: &mut Client<Duration>, connack_kind: ConnAckKind) {
    assert_eq!(
        client.handle_write(UserWriteIn::Connect(connect_options())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some(), "CONNECT should be queued");

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: connack_kind,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&connack),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

/// The full restart path: publish, take the session out, build a new client
/// from it, and confirm the unacknowledged PUBLISH is replayed with DUP set.
#[test]
fn session_taken_from_one_client_resumes_inflight_publish_in_another() {
    let mut original = Client::<Duration>::default();
    drive_to_connected(
        &mut original,
        ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    assert_eq!(
        original.handle_write(UserWriteIn::PublishMessage(ClientMessage {
            topic: topic("restart/topic"),
            qos: Qos::AtLeastOnce,
            payload: Payload::from(&b"survive"[..]),
            ..ClientMessage::default()
        })),
        Ok(())
    );
    assert!(original.poll_write().is_some(), "PUBLISH should be queued");

    // The process dies here; only the session is persisted.
    let session = original.into_session();
    assert_eq!(
        session.on_flight_sent.len(),
        1,
        "the unacknowledged QoS1 PUBLISH must be part of the persisted session"
    );
    assert!(matches!(
        session.on_flight_sent.get(&packet_id),
        Some(OutboundInflightState::Qos1AwaitPubAck { .. })
    ));

    let mut restarted =
        Client::<Duration>::with_settings_and_session(ClientSettings::default(), session);
    drive_to_connected(&mut restarted, ConnAckKind::ResumePreviousSession);

    let replayed = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: true,
        },
        retain: false,
        payload: Payload::from(&b"survive"[..]),
        topic: topic("restart/topic"),
        properties: PublishProperties::default(),
    });
    assert_eq!(
        restarted.poll_write(),
        Some(encode_packet(&replayed)),
        "[MQTT-4.4.0-1] the resumed session must retransmit with DUP=1"
    );
}

/// Every field is reachable so an application can serialize the session with
/// whatever format it likes, and rebuild it field by field afterwards.
#[test]
fn client_session_fields_are_publicly_readable_and_writable() {
    let mut session = ClientSession::default();

    assert!(session.on_flight_sent.is_empty());
    assert!(session.on_flight_received.is_empty());
    assert!(session.pending_subscribe.is_empty());
    assert!(session.pending_unsubscribe.is_empty());
    assert!(session.inbound_topic_aliases.is_empty());

    // The packet-id counter is non-zero by construction: [MQTT-2.2.1-3] forbids
    // a Packet Identifier of 0, so the type rules it out rather than a check.
    let next: NonZero<u16> = session.next_packet_id;
    assert_eq!(next.get(), 1);

    session.next_packet_id = NonZero::new(42).expect("non-zero");
    session.pending_subscribe.insert(next);

    let restored =
        Client::<Duration>::with_settings_and_session(ClientSettings::default(), session.clone());
    assert_eq!(restored.into_session(), session);
}
