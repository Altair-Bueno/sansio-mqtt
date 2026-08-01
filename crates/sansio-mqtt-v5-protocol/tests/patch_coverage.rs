//! Paths introduced or reshaped by the cleanup refactor that the existing suite
//! did not reach: the re-buffering read path, the shared
//! acknowledgement-failure teardown, QoS2 replay, and the consolidated
//! protocol-error branches.

use bytes::Bytes;
use core::num::NonZero;
use core::time::Duration;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientMessage;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_protocol::ConnectionOptions;
use sansio_mqtt_v5_protocol::DriverEventIn;
use sansio_mqtt_v5_protocol::DriverEventOut;
use sansio_mqtt_v5_protocol::Error;
use sansio_mqtt_v5_protocol::IncomingData;
use sansio_mqtt_v5_protocol::OutboundInflightState;
use sansio_mqtt_v5_protocol::SubscribeOptions;
use sansio_mqtt_v5_protocol::UserWriteIn;
use sansio_mqtt_v5_protocol::UserWriteOut;
use sansio_mqtt_v5_protocol::Will;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnAckProperties;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::GuaranteedQoS;
use sansio_mqtt_v5_types::Payload;
use sansio_mqtt_v5_types::PingReq;
use sansio_mqtt_v5_types::PubRel;
use sansio_mqtt_v5_types::PubRelProperties;
use sansio_mqtt_v5_types::PubRelReasonCode;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::PublishProperties;
use sansio_mqtt_v5_types::Qos;
use sansio_mqtt_v5_types::RetainHandling;
use sansio_mqtt_v5_types::Subscription;
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

fn packet_id(value: u16) -> NonZero<u16> {
    NonZero::new(value).expect("non-zero packet id")
}

fn connack(properties: ConnAckProperties) -> ControlPacket {
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties,
    })
}

fn inbound_publish(id: NonZero<u16>, qos: GuaranteedQoS) -> ControlPacket {
    ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: id,
            qos,
            dup: false,
        },
        retain: false,
        payload: Payload::from(&b"x"[..]),
        topic: topic("cov/topic"),
        properties: PublishProperties::default(),
    })
}

/// Drives a default client to Connected, using `properties` in the CONNACK.
fn connected_client(properties: ConnAckProperties) -> Client<Duration> {
    let mut client = Client::<Duration>::default();
    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            session_expiry_interval: Some(30),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some(), "CONNECT should be queued");
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&connack(properties)),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
    client
}

/// A packet split across two reads must be buffered and reassembled, exercising
/// the retained-buffer branch of `handle_read`.
#[test]
fn packet_split_across_two_reads_is_reassembled() {
    let mut client = connected_client(ConnAckProperties::default());

    let publish = encode_packet(&inbound_publish(packet_id(1), GuaranteedQoS::AtLeastOnce));
    let split = publish.len() / 2;
    assert!(
        split > 0 && split < publish.len(),
        "needs a real split point"
    );

    // First half: incomplete, nothing delivered, remainder retained.
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: publish.slice(..split),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(
        client.poll_read().is_none(),
        "a partial packet must not be delivered"
    );

    // Second half completes it.
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: publish.slice(split..),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(..))
    ));
}

/// Three packets arriving as one chunk with a trailing partial fourth: the
/// whole ones are dispatched and only the tail is retained.
#[test]
fn trailing_partial_packet_is_retained_across_reads() {
    let mut client = connected_client(ConnAckProperties::default());

    let mut buffer = Vec::new();
    for id in 1..=3u16 {
        buffer.extend_from_slice(&encode_packet(&inbound_publish(
            packet_id(id),
            GuaranteedQoS::AtLeastOnce,
        )));
    }
    let fourth = encode_packet(&inbound_publish(packet_id(4), GuaranteedQoS::AtLeastOnce));
    buffer.extend_from_slice(&fourth[..2]);

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: Bytes::from(buffer),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    for _ in 0..3 {
        assert!(matches!(
            client.poll_read(),
            Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(..))
        ));
    }
    assert!(client.poll_read().is_none());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: fourth.slice(2..),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(..))
    ));
}

/// An acknowledgement that cannot be sent within the broker's Maximum Packet
/// Size fails the connection, via the shared ack-failure teardown.
#[test]
fn acknowledgement_exceeding_broker_maximum_packet_size_fails_the_connection() {
    let mut client = connected_client(ConnAckProperties {
        // Smaller than any PUBACK, so the acknowledgement cannot be sent.
        maximum_packet_size: Some(NonZero::new(2).expect("non-zero")),
        ..ConnAckProperties::default()
    });

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&inbound_publish(packet_id(1), GuaranteedQoS::AtLeastOnce)),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    let id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
        other => panic!("expected an ack-required message, got {other:?}"),
    };

    assert_eq!(
        client.handle_write(UserWriteIn::AcknowledgeMessage(id)),
        Err(Error::ProtocolError),
        "an unsendable PUBACK leaves the QoS1 exchange unresolvable"
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::CloseSocket)
    ));
}

/// Acknowledging a QoS2 message moves the exchange to awaiting PUBREL.
///
/// The sibling branch — a second decision on the same packet id — is
/// unreachable from outside the crate: [`InboundMessageId`] has no public
/// constructor and is neither `Copy` nor `Clone`, so an application cannot hold
/// an id past the single `AcknowledgeMessage`/`RejectMessage` that consumes it.
/// That branch stays defensive until the type gains a public constructor.
#[test]
fn acknowledging_a_qos2_message_moves_it_to_awaiting_pubrel() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce)),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    let id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
        other => panic!("expected an ack-required message, got {other:?}"),
    };

    assert_eq!(
        client.handle_write(UserWriteIn::AcknowledgeMessage(id)),
        Ok(())
    );
    assert!(client.poll_write().is_some(), "PUBREC should be queued");

    // The server may now complete the exchange.
    let pubrel = ControlPacket::PubRel(PubRel {
        packet_id: packet_id(1),
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&pubrel),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some(), "PUBCOMP should be queued");
}

/// A QoS1 PUBLISH reusing a packet id already held by a QoS2 exchange is a
/// protocol error.
#[test]
fn qos1_publish_reusing_a_qos2_packet_id_is_a_protocol_error() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce)),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(client.poll_read().is_some());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&inbound_publish(packet_id(1), GuaranteedQoS::AtLeastOnce)),
            received_at: Duration::ZERO,
        }),
        Err(Error::ProtocolError)
    );
}

/// And the mirror image: QoS2 reusing an id held by a QoS1 exchange.
#[test]
fn qos2_publish_reusing_a_qos1_packet_id_is_a_protocol_error() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&inbound_publish(packet_id(1), GuaranteedQoS::AtLeastOnce)),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(client.poll_read().is_some());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce)),
            received_at: Duration::ZERO,
        }),
        Err(Error::ProtocolError)
    );
}

/// PUBREL for an id still awaiting the application's decision is a protocol
/// error.
#[test]
fn pubrel_before_the_application_decides_is_a_protocol_error() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce)),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(client.poll_read().is_some());

    let pubrel = ControlPacket::PubRel(PubRel {
        packet_id: packet_id(1),
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&pubrel),
            received_at: Duration::ZERO,
        }),
        Err(Error::ProtocolError)
    );
}

/// A packet the client must never receive from a server is a protocol error.
#[test]
fn server_bound_packet_received_while_connected_is_a_protocol_error() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&ControlPacket::PingReq(PingReq {})),
            received_at: Duration::ZERO,
        }),
        Err(Error::ProtocolError)
    );
}

/// A socket error while Connected resets state and asks the driver to close.
#[test]
fn socket_error_while_connected_resets_and_closes() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_event(DriverEventIn::SocketError),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::CloseSocket)
    ));
}

/// `close` in the Start state resets without emitting anything.
#[test]
fn close_in_start_state_resets_quietly() {
    let mut client = Client::<Duration>::default();

    assert_eq!(client.close(), Ok(()));
    assert!(client.poll_read().is_none());
    assert!(client.poll_write().is_none());
    assert!(client.poll_event().is_none());
}

/// A QoS2 exchange awaiting PUBREC is replayed with DUP=1 on session resume.
#[test]
fn resumed_session_replays_qos2_publish_awaiting_pubrec() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(ClientMessage {
            topic: topic("cov/qos2"),
            qos: Qos::ExactlyOnce,
            payload: Payload::from(&b"q2"[..]),
            ..ClientMessage::default()
        })),
        Ok(())
    );
    assert!(client.poll_write().is_some(), "PUBLISH should be queued");
    assert!(matches!(
        client.session().on_flight_sent.get(&packet_id(1)),
        Some(OutboundInflightState::Qos2AwaitPubRec { .. })
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected(None))
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some(), "CONNECT should be queued");
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&ControlPacket::ConnAck(ConnAck {
                kind: ConnAckKind::ResumePreviousSession,
                properties: ConnAckProperties::default(),
            })),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let replayed = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: packet_id(1),
            qos: GuaranteedQoS::ExactlyOnce,
            dup: true,
        },
        retain: false,
        payload: Payload::from(&b"q2"[..]),
        topic: topic("cov/qos2"),
        properties: PublishProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&replayed)));
}

/// A Topic Alias is rejected when the server advertised no alias capacity.
#[test]
fn outbound_topic_alias_without_server_capacity_is_rejected() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(ClientMessage {
            topic: topic("cov/alias"),
            topic_alias: Some(packet_id(1)),
            ..ClientMessage::default()
        })),
        Err(Error::ProtocolError),
        "[MQTT-3.2.2-17] Topic Alias Maximum of 0 forbids aliases"
    );
}

/// A redelivered QoS2 PUBLISH while awaiting PUBREL re-sends PUBREC rather than
/// delivering the message twice.
#[test]
fn redelivered_qos2_publish_awaiting_pubrel_resends_pubrec() {
    let mut client = connected_client(ConnAckProperties::default());

    let publish = inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce);
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&publish),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    let id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
        other => panic!("expected an ack-required message, got {other:?}"),
    };
    assert_eq!(
        client.handle_write(UserWriteIn::AcknowledgeMessage(id)),
        Ok(())
    );
    let first_pubrec = client.poll_write().expect("PUBREC should be queued");

    // [MQTT-4.3.3-2] The server may redeliver until it sees PUBREC.
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: encode_packet(&publish),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert_eq!(
        client.poll_write(),
        Some(first_pubrec),
        "the same PUBREC must be repeated"
    );
    assert!(
        client.poll_read().is_none(),
        "the message must not be delivered to the application twice"
    );
}

/// A Shared Subscription without No Local is permitted.
#[test]
fn shared_subscription_without_no_local_is_accepted() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(SubscribeOptions {
            subscription: Subscription {
                topic_filter: Utf8String::try_from("$share/group/cov").expect("valid utf8"),
                qos: Qos::AtMostOnce,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendRetained,
            },
            extra_subscriptions: Vec::new(),
            subscription_identifier: None,
            user_properties: Vec::new(),
        })),
        Ok(())
    );
    assert!(client.poll_write().is_some(), "SUBSCRIBE should be queued");
}

/// [MQTT-3.8.3-4] A Shared Subscription cannot be combined with No Local.
#[test]
fn shared_subscription_with_no_local_is_rejected() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(SubscribeOptions {
            subscription: Subscription {
                topic_filter: Utf8String::try_from("$share/group/cov").expect("valid utf8"),
                qos: Qos::AtMostOnce,
                no_local: true,
                retain_as_published: false,
                retain_handling: RetainHandling::SendRetained,
            },
            extra_subscriptions: Vec::new(),
            subscription_identifier: None,
            user_properties: Vec::new(),
        })),
        Err(Error::ProtocolError)
    );
}

/// A Will whose message expiry cannot fit in the wire format fails CONNECT
/// construction and leaves the client in Connecting, able to retry.
#[test]
fn unencodable_will_fails_connect_and_allows_a_retry() {
    let mut client = Client::<Duration>::with_settings(ClientSettings::default());

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            will: Some(Will {
                topic: topic("cov/will"),
                message_expiry_interval: Some(Duration::from_secs(u64::from(u32::MAX) + 1)),
                ..Will::default()
            }),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));

    assert_eq!(
        client.handle_event(DriverEventIn::SocketConnected),
        Err(Error::ProtocolError),
        "a Will that cannot be encoded must fail the CONNECT"
    );
    assert!(client.poll_write().is_none(), "no CONNECT should be queued");

    // Still Connecting with connect_sent = false, so a retry re-attempts CONNECT.
    assert_eq!(
        client.handle_event(DriverEventIn::SocketConnected),
        Err(Error::ProtocolError)
    );
}
