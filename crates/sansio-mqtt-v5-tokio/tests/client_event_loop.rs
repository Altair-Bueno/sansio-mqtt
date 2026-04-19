use core::num::NonZero;

use encode::Encodable;
use sansio_mqtt_v5_protocol::{BrokerMessage, ClientMessage, UserWriteIn, UserWriteOut};
use sansio_mqtt_v5_tokio::{Client, ConnectOptions, Event, EventLoop};
use sansio_mqtt_v5_types::{
    ConnAck, ConnAckKind, ConnAckProperties, ConnackReasonCode, ControlPacket, PubAckReasonCode,
    PubCompReasonCode, PubRecReasonCode,
};
use sansio_mqtt_v5_types::{
    GuaranteedQoS, Payload, Publish, PublishKind, PublishProperties, Qos, Topic, Utf8String,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[tokio::test]
async fn connect_api_exposes_split_handles() {
    let _ = core::any::TypeId::of::<Client>();
    let _ = core::any::TypeId::of::<EventLoop>();
    let _ = core::any::TypeId::of::<Event>();
    let _ = core::any::TypeId::of::<ConnectOptions>();
}

#[tokio::test]
async fn client_publish_enqueues_command() {
    let (tx, mut rx) = mpsc::channel::<UserWriteIn>(1);
    let client = Client::new_for_test(tx);
    let topic = Topic::try_from(Utf8String::try_from("topic/test").expect("valid utf8"))
        .expect("valid topic");

    client
        .publish(ClientMessage {
            topic,
            qos: Qos::AtMostOnce,
            payload: Payload::from(&b"hello"[..]),
            ..ClientMessage::default()
        })
        .await
        .expect("publish command enqueued");

    let command = rx.recv().await.expect("one command expected");
    assert!(matches!(command, UserWriteIn::PublishMessage(_)));
}

#[tokio::test]
async fn maps_received_message_variants() {
    let message = BrokerMessage::default();

    let without_ack = Event::from_protocol_output(UserWriteOut::ReceivedMessage(message.clone()));
    assert!(matches!(without_ack, Event::Message(BrokerMessage { .. })));

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let broker_addr = listener.local_addr().expect("listener addr");

    let packet_id = NonZero::new(7).expect("non-zero packet id");
    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accepts client");
        let mut connect_bytes = [0_u8; 1024];
        let read = socket
            .read(&mut connect_bytes)
            .await
            .expect("reads connect frame");
        assert!(read > 0, "connect frame expected");

        let mut connack = Vec::new();
        ControlPacket::ConnAck(ConnAck {
            kind: ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            },
            properties: ConnAckProperties::default(),
        })
        .encode(&mut connack)
        .expect("encodes connack");
        socket.write_all(&connack).await.expect("sends connack");

        let mut publish = Vec::new();
        ControlPacket::Publish(Publish {
            kind: PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            },
            retain: false,
            payload: Payload::from(&b"hello"[..]),
            topic: Topic::try_new("topic/test").expect("valid topic"),
            properties: PublishProperties::default(),
        })
        .encode(&mut publish)
        .expect("encodes publish");
        socket.write_all(&publish).await.expect("sends publish");
    });

    let (_client, mut event_loop) = sansio_mqtt_v5_tokio::connect(ConnectOptions {
        addr: broker_addr,
        ..ConnectOptions::default()
    })
    .await
    .expect("connects to local broker harness");

    let event = event_loop.poll().await.expect("poll returns connected");
    assert!(matches!(event, Event::Connected));

    let with_ack = event_loop.poll().await.expect("poll returns message");
    assert!(matches!(
        with_ack,
        Event::MessageWithRequiredAcknowledgement(_, BrokerMessage { .. })
    ));

    server_task.await.expect("server task joins");
}

#[test]
fn maps_publish_dropped_and_delivery_events_tuple_variants() {
    let packet_id = NonZero::new(7).expect("non-zero packet id");

    let connected = Event::from_protocol_output(UserWriteOut::Connected);
    let disconnected = Event::from_protocol_output(UserWriteOut::Disconnected);
    let acknowledged = Event::from_protocol_output(UserWriteOut::PublishAcknowledged(
        packet_id,
        PubAckReasonCode::Success,
    ));
    let completed = Event::from_protocol_output(UserWriteOut::PublishCompleted(
        packet_id,
        PubCompReasonCode::Success,
    ));
    let dropped = Event::from_protocol_output(UserWriteOut::PublishDroppedDueToSessionNotResumed(
        packet_id,
    ));
    let dropped_by_broker =
        Event::from_protocol_output(UserWriteOut::PublishDroppedDueToBrokerRejectedPubRec(
            packet_id,
            PubRecReasonCode::PacketIdentifierInUse,
        ));

    assert!(matches!(connected, Event::Connected));
    assert!(matches!(disconnected, Event::Disconnected));
    assert!(matches!(
        acknowledged,
        Event::PublishAcknowledged(id, reason)
            if id == packet_id && reason == PubAckReasonCode::Success
    ));
    assert!(matches!(
        completed,
        Event::PublishCompleted(id, reason)
            if id == packet_id && reason == PubCompReasonCode::Success
    ));
    assert!(matches!(
        dropped,
        Event::PublishDroppedDueToSessionNotResumed(got) if got == packet_id
    ));
    assert!(matches!(
        dropped_by_broker,
        Event::PublishDroppedDueToBrokerRejectedPubRec(id, reason)
            if id == packet_id && reason == PubRecReasonCode::PacketIdentifierInUse
    ));
}

#[tokio::test]
async fn poll_emits_connected_after_connack_flow() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let broker_addr = listener.local_addr().expect("listener addr");

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accepts client");
        let mut connect_bytes = [0_u8; 1024];
        let read = socket
            .read(&mut connect_bytes)
            .await
            .expect("reads connect frame");
        assert!(read > 0, "connect frame expected");

        let mut connack = Vec::new();
        ControlPacket::ConnAck(ConnAck {
            kind: ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            },
            properties: ConnAckProperties::default(),
        })
        .encode(&mut connack)
        .expect("encodes connack");

        socket.write_all(&connack).await.expect("sends connack");
    });

    let (mut _client, mut event_loop) = sansio_mqtt_v5_tokio::connect(ConnectOptions {
        addr: broker_addr,
        ..ConnectOptions::default()
    })
    .await
    .expect("connects to local broker harness");

    let event = event_loop.poll().await.expect("poll returns event");
    assert!(matches!(event, Event::Connected));

    server_task.await.expect("server task joins");
}
