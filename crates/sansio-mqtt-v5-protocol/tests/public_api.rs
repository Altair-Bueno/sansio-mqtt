//! New tests for the version-neutral public surface, per the task-3 brief:
//! token-based publish acknowledgement, outbound validation errors, the
//! never-alias outbound PUBLISH invariant, and a compile-time
//! `MqttProtocol` conformance check.

use bytes::Bytes;
use bytestring::ByteString;
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
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::MqttProtocol;
use sansio_mqtt_protocol::Qos as ProtoQos;
use sansio_mqtt_protocol::ReasonCode;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::PubAck;
use sansio_mqtt_v5_types::PubAckReasonCode;
use sansio_mqtt_v5_types::PubComp;
use sansio_mqtt_v5_types::PubCompReasonCode;
use sansio_mqtt_v5_types::PubRec;
use sansio_mqtt_v5_types::PubRecReasonCode;
use winnow::Parser;
use winnow::error::ContextError;

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

fn read(client: &mut Client<Duration>, packet: &ControlPacket) -> Result<(), Error> {
    client.handle_read(IncomingData {
        bytes: encode_packet(packet),
        received_at: Duration::ZERO,
    })
}

fn connect_options() -> ConnectOptions {
    ConnectOptions::builder()
        .client_id(ByteString::from_static("public-api-client"))
        .build()
}

fn message(topic: &str, qos: ProtoQos, payload: &[u8]) -> Message {
    Message::builder()
        .topic(ByteString::from(topic))
        .payload(Bytes::copy_from_slice(payload))
        .qos(qos)
        .build()
}

/// Drives `client` to `Connected` with default `ConnectOptions`.
fn connect(client: &mut Client<Duration>) {
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

    let connack = ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .build(),
    );
    assert_eq!(read(client, &connack), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

/// QoS 1 publish with an app-chosen token: PUBACK echoes back that same token,
/// not the wire Packet Identifier.
#[test]
fn qos1_publish_acknowledged_event_carries_the_app_token() {
    let mut client = Client::<Duration>::default();
    connect(&mut client);

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 42,
            message: message("public/qos1", ProtoQos::AtLeastOnce, b"hi"),
        }),
        Ok(())
    );
    let publish_bytes = client.poll_write().expect("PUBLISH should be queued");
    let packet_id = match ControlPacket::parser::<_, ContextError, ContextError>(
        &sansio_mqtt_v5_types::ParserSettings::default(),
    )
    .parse(publish_bytes.as_ref())
    .expect("publish packet should decode")
    {
        ControlPacket::Publish(publish) => match publish.kind {
            sansio_mqtt_v5_types::PublishKind::Repetible { packet_id, .. } => packet_id,
            other => panic!("expected a Packet Identifier, got {other:?}"),
        },
        other => panic!("expected PUBLISH, got {other:?}"),
    };

    let puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(PubAckReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &puback), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishAcknowledged {
            token: 42,
            reason: ReasonCode::Success,
        })
    );
}

/// QoS 2 publish with an app-chosen token: PUBCOMP echoes back that token via
/// `PublishCompleted`.
#[test]
fn qos2_publish_completed_event_carries_the_app_token() {
    let mut client = Client::<Duration>::default();
    connect(&mut client);

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 42,
            message: message("public/qos2", ProtoQos::ExactlyOnce, b"hi"),
        }),
        Ok(())
    );
    let publish_bytes = client.poll_write().expect("PUBLISH should be queued");
    let packet_id = match ControlPacket::parser::<_, ContextError, ContextError>(
        &sansio_mqtt_v5_types::ParserSettings::default(),
    )
    .parse(publish_bytes.as_ref())
    .expect("publish packet should decode")
    {
        ControlPacket::Publish(publish) => match publish.kind {
            sansio_mqtt_v5_types::PublishKind::Repetible { packet_id, .. } => packet_id,
            other => panic!("expected a Packet Identifier, got {other:?}"),
        },
        other => panic!("expected PUBLISH, got {other:?}"),
    };

    let pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrec), Ok(()));
    assert!(client.poll_write().is_some(), "PUBREL should be queued");

    let pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(PubCompReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubcomp), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishCompleted {
            token: 42,
            reason: ReasonCode::Success,
        })
    );
}

/// Two in-flight QoS 1 publishes with different app tokens, acknowledged out
/// of order, must each report their own token.
#[test]
fn two_in_flight_publishes_acknowledged_out_of_order_report_the_right_tokens() {
    let mut client = Client::<Duration>::default();
    connect(&mut client);

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 100,
            message: message("public/first", ProtoQos::AtLeastOnce, b"a"),
        }),
        Ok(())
    );
    let first_bytes = client.poll_write().expect("first PUBLISH should be queued");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 200,
            message: message("public/second", ProtoQos::AtLeastOnce, b"b"),
        }),
        Ok(())
    );
    let second_bytes = client
        .poll_write()
        .expect("second PUBLISH should be queued");

    let packet_id_of = |bytes: &Bytes| match ControlPacket::parser::<_, ContextError, ContextError>(
        &sansio_mqtt_v5_types::ParserSettings::default(),
    )
    .parse(bytes.as_ref())
    .expect("publish packet should decode")
    {
        ControlPacket::Publish(publish) => match publish.kind {
            sansio_mqtt_v5_types::PublishKind::Repetible { packet_id, .. } => packet_id,
            other => panic!("expected a Packet Identifier, got {other:?}"),
        },
        other => panic!("expected PUBLISH, got {other:?}"),
    };
    let first_packet_id = packet_id_of(&first_bytes);
    let second_packet_id = packet_id_of(&second_bytes);

    // Acknowledge out of order: the second publish first.
    assert_eq!(
        read(
            &mut client,
            &ControlPacket::PubAck(
                PubAck::builder()
                    .packet_id(second_packet_id)
                    .reason_code(PubAckReasonCode::Success)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishAcknowledged {
            token: 200,
            reason: ReasonCode::Success,
        })
    );

    assert_eq!(
        read(
            &mut client,
            &ControlPacket::PubAck(
                PubAck::builder()
                    .packet_id(first_packet_id)
                    .reason_code(PubAckReasonCode::Success)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishAcknowledged {
            token: 100,
            reason: ReasonCode::Success,
        })
    );
}

/// A wildcard character makes a Topic Name invalid ([MQTT-4.7.1-1],
/// [MQTT-4.7.1-2]); `handle_write` reports it without emitting any bytes.
#[test]
fn publish_with_wildcard_topic_is_invalid_topic_and_emits_no_bytes() {
    let mut client = Client::<Duration>::default();
    connect(&mut client);

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("a/+/b", ProtoQos::AtMostOnce, b"x"),
        }),
        Err(Error::InvalidTopic)
    );
    assert_eq!(client.poll_write(), None);
}

/// [MQTT-1.5.4-1] A Topic Name longer than the wire's `u16` length limit is
/// reported as `StringTooLong`.
#[test]
fn publish_with_oversized_topic_is_string_too_long() {
    let mut client = Client::<Duration>::default();
    connect(&mut client);

    let topic = "t".repeat(70_000);
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message(&topic, ProtoQos::AtMostOnce, b"x"),
        }),
        Err(Error::StringTooLong)
    );
    assert_eq!(client.poll_write(), None);
}

/// [MQTT-3.8.3-3] A SUBSCRIBE MUST carry at least one subscription.
#[test]
fn subscribe_with_empty_subscriptions_is_empty_subscribe() {
    let mut client = Client::<Duration>::default();
    connect(&mut client);

    assert_eq!(
        client.handle_write(Command::Subscribe(SubscribeOptions::builder().build())),
        Err(Error::EmptySubscribe)
    );
    assert_eq!(client.poll_write(), None);
}

/// Outbound PUBLISH always carries the full Topic Name; the Topic Alias
/// property is never set, regardless of how many times the same topic is
/// published.
#[test]
fn outbound_publish_never_carries_a_topic_alias() {
    let mut client = Client::<Duration>::default();
    connect(&mut client);

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("public/no-alias", ProtoQos::AtMostOnce, b"x"),
        }),
        Ok(())
    );
    let publish_bytes = client.poll_write().expect("PUBLISH should be queued");

    let publish = match ControlPacket::parser::<_, ContextError, ContextError>(
        &sansio_mqtt_v5_types::ParserSettings::default(),
    )
    .parse(publish_bytes.as_ref())
    .expect("publish packet should decode")
    {
        ControlPacket::Publish(publish) => publish,
        other => panic!("expected PUBLISH, got {other:?}"),
    };

    assert!(publish.properties.topic_alias.is_none());
}

#[test]
fn client_satisfies_mqtt_protocol_trait() {
    fn drive(_: impl MqttProtocol<Duration>) {}
    drive(Client::<Duration>::default());
}
