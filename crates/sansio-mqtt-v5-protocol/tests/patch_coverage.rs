//! Paths introduced or reshaped by the cleanup refactor that the existing suite
//! did not reach: the re-buffering read path, the shared
//! acknowledgement-failure teardown, QoS2 replay, and the consolidated
//! protocol-error branches.

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
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::MessageId;
use sansio_mqtt_protocol::Qos as ProtoQos;
use sansio_mqtt_protocol::RejectReason;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription as ProtoSubscription;
use sansio_mqtt_protocol::Will as ProtoWill;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnAckProperties;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::GuaranteedQoS;
use sansio_mqtt_v5_types::Payload;
use sansio_mqtt_v5_types::PingReq;
use sansio_mqtt_v5_types::PubRel;
use sansio_mqtt_v5_types::PubRelReasonCode;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::Topic;
use sansio_mqtt_v5_types::Utf8String;

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

fn wire_topic(name: &str) -> Topic {
    Topic::try_from(Utf8String::try_from(name).expect("valid utf8")).expect("valid topic")
}

fn packet_id(value: u16) -> NonZero<u16> {
    NonZero::new(value).expect("non-zero packet id")
}

fn read(client: &mut Client<Duration>, packet: &ControlPacket) -> Result<(), Error> {
    client.handle_read(IncomingData {
        bytes: &encode_packet(packet),
        received_at: Duration::ZERO,
    })
}

fn connack(properties: ConnAckProperties) -> ControlPacket {
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .properties(properties)
            .build(),
    )
}

fn inbound_publish(id: NonZero<u16>, qos: GuaranteedQoS) -> ControlPacket {
    ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id: id,
                qos,
                dup: false,
            })
            .payload(Payload::from(&b"x"[..]))
            .topic(wire_topic("cov/topic"))
            .build(),
    )
}

fn message(topic: &str, qos: ProtoQos, payload: &[u8]) -> Message {
    Message::builder()
        .topic(ByteString::from(topic))
        .payload(Bytes::copy_from_slice(payload))
        .qos(qos)
        .build()
}

fn connect_options() -> ConnectOptions {
    ConnectOptions::builder()
        .client_id(ByteString::from_static("cov-client"))
        .session_expiry(Duration::from_secs(30))
        .build()
}

/// Drives a default client to Connected, using `properties` in the CONNACK.
fn connected_client(properties: ConnAckProperties) -> Client<Duration> {
    let mut client = Client::<Duration>::default();
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
    assert_eq!(read(&mut client, &connack(properties)), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
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

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: &publish.slice(..split),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(
        client.poll_read().is_none(),
        "a partial packet must not be delivered"
    );

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: &publish.slice(split..),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(
        client.poll_read(),
        Some(Event::MessageRequiresAcknowledgement(..))
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
            bytes: &(buffer),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    for _ in 0..3 {
        assert!(matches!(
            client.poll_read(),
            Some(Event::MessageRequiresAcknowledgement(..))
        ));
    }
    assert!(client.poll_read().is_none());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: &fourth.slice(2..),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );
    assert!(matches!(
        client.poll_read(),
        Some(Event::MessageRequiresAcknowledgement(..))
    ));
}

/// An acknowledgement that cannot be sent within the broker's Maximum Packet
/// Size fails the connection, via the shared ack-failure teardown.
#[test]
fn acknowledgement_exceeding_broker_maximum_packet_size_fails_the_connection() {
    let mut client = connected_client(
        ConnAckProperties::builder()
            // Smaller than any PUBACK, so the acknowledgement cannot be sent.
            .maximum_packet_size(NonZero::new(2).expect("non-zero"))
            .build(),
    );

    assert_eq!(
        read(
            &mut client,
            &inbound_publish(packet_id(1), GuaranteedQoS::AtLeastOnce),
        ),
        Ok(())
    );
    let id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected an ack-required message, got {other:?}"),
    };

    assert_eq!(
        client.handle_write(Command::Acknowledge(id)),
        Err(Error::ProtocolError),
        "an unsendable PUBACK leaves the QoS1 exchange unresolvable"
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

/// Deciding twice on the same message is API misuse, not a protocol violation:
/// it is reported without touching the connection.
#[test]
fn deciding_twice_on_a_message_is_an_invalid_state_transition() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        read(
            &mut client,
            &inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce),
        ),
        Ok(())
    );
    let id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected an ack-required message, got {other:?}"),
    };

    // First decision moves the QoS2 exchange on to awaiting PUBREL.
    assert_eq!(client.handle_write(Command::Acknowledge(id)), Ok(()));
    assert!(client.poll_write().is_some(), "PUBREC should be queued");

    assert_eq!(
        client.handle_write(Command::Acknowledge(id)),
        Err(Error::InvalidStateTransition),
        "the peer did nothing wrong, so this is not a protocol error"
    );
    assert_eq!(
        client.handle_write(Command::Reject(id, RejectReason::UnspecifiedError)),
        Err(Error::InvalidStateTransition)
    );

    // The connection is untouched: no DISCONNECT, no close, still usable.
    assert!(client.poll_write().is_none());
    assert!(client.poll_event().is_none());
    assert!(client.poll_read().is_none());

    let pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(packet_id(1))
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrel), Ok(()));
    assert!(client.poll_write().is_some(), "PUBCOMP should be queued");
}

/// Deciding on a packet id that was never delivered is likewise API misuse.
#[test]
fn deciding_on_an_undelivered_packet_id_is_an_invalid_state_transition() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_write(Command::Acknowledge(MessageId::new(packet_id(9)))),
        Err(Error::InvalidStateTransition)
    );
    assert!(
        client.poll_event().is_none(),
        "an unknown id must not close the socket"
    );
}

/// Acknowledging a QoS2 message moves the exchange to awaiting PUBREL.
#[test]
fn acknowledging_a_qos2_message_moves_it_to_awaiting_pubrel() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        read(
            &mut client,
            &inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce),
        ),
        Ok(())
    );
    let id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected an ack-required message, got {other:?}"),
    };

    assert_eq!(client.handle_write(Command::Acknowledge(id)), Ok(()));
    assert!(client.poll_write().is_some(), "PUBREC should be queued");

    // The server may now complete the exchange.
    let pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(packet_id(1))
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrel), Ok(()));
    assert!(client.poll_write().is_some(), "PUBCOMP should be queued");
}

/// A QoS1 PUBLISH reusing a packet id already held by a QoS2 exchange is a
/// protocol error.
#[test]
fn qos1_publish_reusing_a_qos2_packet_id_is_a_protocol_error() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        read(
            &mut client,
            &inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce),
        ),
        Ok(())
    );
    assert!(client.poll_read().is_some());

    assert_eq!(
        read(
            &mut client,
            &inbound_publish(packet_id(1), GuaranteedQoS::AtLeastOnce),
        ),
        Err(Error::ProtocolError)
    );
}

/// And the mirror image: QoS2 reusing an id held by a QoS1 exchange.
#[test]
fn qos2_publish_reusing_a_qos1_packet_id_is_a_protocol_error() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        read(
            &mut client,
            &inbound_publish(packet_id(1), GuaranteedQoS::AtLeastOnce),
        ),
        Ok(())
    );
    assert!(client.poll_read().is_some());

    assert_eq!(
        read(
            &mut client,
            &inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce),
        ),
        Err(Error::ProtocolError)
    );
}

/// PUBREL for an id still awaiting the application's decision is a protocol
/// error.
#[test]
fn pubrel_before_the_application_decides_is_a_protocol_error() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        read(
            &mut client,
            &inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce),
        ),
        Ok(())
    );
    assert!(client.poll_read().is_some());

    let pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(packet_id(1))
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrel), Err(Error::ProtocolError));
}

/// A packet the client must never receive from a server is a protocol error.
#[test]
fn server_bound_packet_received_while_connected_is_a_protocol_error() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        read(&mut client, &ControlPacket::PingReq(PingReq {})),
        Err(Error::ProtocolError)
    );
}

/// A second SocketConnected while already Connected is driver misuse.
#[test]
fn socket_connected_while_already_connected_is_an_invalid_state_transition() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_event(DriverEvent::SocketConnected),
        Err(Error::InvalidStateTransition)
    );
    assert!(
        client.poll_event().is_none(),
        "the established connection must be left alone"
    );
}

/// A socket error while Connected resets state and asks the driver to close.
#[test]
fn socket_error_while_connected_resets_and_closes() {
    let mut client = connected_client(ConnAckProperties::default());

    assert_eq!(
        client.handle_event(DriverEvent::SocketError),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
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
        client.handle_write(Command::Publish {
            token: 1,
            message: message("cov/qos2", ProtoQos::ExactlyOnce, b"q2"),
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some(), "PUBLISH should be queued");

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(Event::Disconnected(None))
    ));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some(), "CONNECT should be queued");
    assert_eq!(
        read(
            &mut client,
            &ControlPacket::ConnAck(
                ConnAck::builder()
                    .kind(ConnAckKind::ResumePreviousSession)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let replayed = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id: packet_id(1),
                qos: GuaranteedQoS::ExactlyOnce,
                dup: true,
            })
            .payload(Payload::from(&b"q2"[..]))
            .topic(wire_topic("cov/qos2"))
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&replayed)));
}

/// A redelivered QoS2 PUBLISH while awaiting PUBREL re-sends PUBREC rather than
/// delivering the message twice.
#[test]
fn redelivered_qos2_publish_awaiting_pubrel_resends_pubrec() {
    let mut client = connected_client(ConnAckProperties::default());

    let publish = inbound_publish(packet_id(1), GuaranteedQoS::ExactlyOnce);
    assert_eq!(read(&mut client, &publish), Ok(()));
    let id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected an ack-required message, got {other:?}"),
    };
    assert_eq!(client.handle_write(Command::Acknowledge(id)), Ok(()));
    let first_pubrec = client.poll_write().expect("PUBREC should be queued");

    // [MQTT-4.3.3-2] The server may redeliver until it sees PUBREC.
    assert_eq!(read(&mut client, &publish), Ok(()));
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

    let sub = ProtoSubscription::builder()
        .filter(ByteString::from_static("$share/group/cov"))
        .build();
    assert_eq!(
        client.handle_write(Command::Subscribe(
            SubscribeOptions::builder().subscriptions(vec![sub]).build(),
        )),
        Ok(())
    );
    assert!(client.poll_write().is_some(), "SUBSCRIBE should be queued");
}

/// [MQTT-3.8.3-4] A Shared Subscription cannot be combined with No Local.
#[test]
fn shared_subscription_with_no_local_is_rejected() {
    let mut client = connected_client(ConnAckProperties::default());

    let mut sub = ProtoSubscription::builder()
        .filter(ByteString::from_static("$share/group/cov"))
        .build();
    sub.no_local = true;
    assert_eq!(
        client.handle_write(Command::Subscribe(
            SubscribeOptions::builder().subscriptions(vec![sub]).build(),
        )),
        Err(Error::ProtocolError)
    );
}

/// A Will whose message expiry cannot fit in the wire format fails CONNECT
/// construction and leaves the client in Connecting, able to retry.
#[test]
fn unencodable_will_fails_connect_and_allows_a_retry() {
    let mut client = Client::<Duration>::new(ClientSettings::default());

    let will = ProtoWill::builder()
        .topic(ByteString::from_static("cov/will"))
        .payload(Bytes::new())
        .message_expiry(Duration::from_secs(u64::from(u32::MAX) + 1))
        .build();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("cov-client"))
        .will(will)
        .build();

    assert_eq!(client.handle_write(Command::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::OpenSocket)
    ));

    assert_eq!(
        client.handle_event(DriverEvent::SocketConnected),
        Err(Error::ProtocolError),
        "a Will that cannot be encoded must fail the CONNECT"
    );
    assert!(client.poll_write().is_none(), "no CONNECT should be queued");

    // Still Connecting with connect_sent = false, so a retry re-attempts
    // CONNECT.
    assert_eq!(
        client.handle_event(DriverEvent::SocketConnected),
        Err(Error::ProtocolError)
    );
}
