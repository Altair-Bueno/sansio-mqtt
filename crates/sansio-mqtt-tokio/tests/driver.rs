//! End-to-end check of the tokio driver against the real v5 client, using an
//! in-memory duplex stream as the "broker" side of the socket.

use std::io;
use std::time::Duration;

use bytes::Bytes;
use bytes::BytesMut;
use encode::Encodable;
use sansio_mqtt_protocol::ByteString;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::ParserSettings;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::DuplexStream;
use tokio::io::duplex;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use winnow::Parser;
use winnow::error::ContextError;
use winnow::error::ErrMode;
use winnow::stream::Partial;

const STEP: Duration = Duration::from_secs(2);

fn encode(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

/// Reads from the broker side until one complete control packet parses.
async fn read_packet(broker: &mut DuplexStream, buffer: &mut BytesMut) -> ControlPacket {
    let settings = ParserSettings::default();
    loop {
        let parsed = {
            let mut input = Partial::new(&buffer[..]);
            let before = input.len();
            match ControlPacket::parser::<_, ErrMode<ContextError>, ErrMode<ContextError>>(
                &settings,
            )
            .parse_next(&mut input)
            {
                Ok(packet) => Ok(Some((packet, before - input.len()))),
                Err(ErrMode::Incomplete(_)) => Ok(None),
                Err(error) => Err(error),
            }
        };
        match parsed {
            Ok(Some((packet, consumed))) => {
                let _ = buffer.split_to(consumed);
                return packet;
            }
            Ok(None) => {
                let read = timeout(STEP, broker.read_buf(buffer))
                    .await
                    .expect("broker read timed out")
                    .expect("broker read failed");
                assert_ne!(read, 0, "client closed the socket early");
            }
            Err(error) => panic!("undecodable packet from client: {error:?}"),
        }
    }
}

#[tokio::test]
async fn connects_publishes_and_disconnects_when_commands_close() {
    let (client_side, mut broker) = duplex(64 * 1024);
    let mut pending_socket = Some(client_side);
    let connector = move || {
        let socket = pending_socket
            .take()
            .expect("driver must connect exactly once");
        async move { Ok::<_, io::Error>(BufReader::new(socket)) }
    };
    let (command_tx, command_rx) = mpsc::channel(8);
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let driver = tokio::spawn(
        Driver::new(
            Client::<Instant>::default(),
            connector,
            command_rx,
            event_tx,
        )
        .run(),
    );
    let mut inbound = BytesMut::new();

    command_tx
        .send(Command::Connect(
            ConnectOptions::builder()
                .client_id(ByteString::from_static("driver-test"))
                .build(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        read_packet(&mut broker, &mut inbound).await,
        ControlPacket::Connect(_)
    ));

    let connack = ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .build(),
    );
    broker.write_all(&encode(&connack)).await.unwrap();
    assert!(matches!(
        timeout(STEP, event_rx.recv())
            .await
            .expect("no Connected event"),
        Some(Event::Connected)
    ));

    command_tx
        .send(Command::Publish {
            token: 7,
            message: Message::builder()
                .topic(ByteString::from_static("a/b"))
                .payload(Bytes::from_static(b"hello"))
                .build(),
        })
        .await
        .unwrap();
    match read_packet(&mut broker, &mut inbound).await {
        ControlPacket::Publish(publish) => {
            let topic: &str = &publish.topic;
            assert_eq!(topic, "a/b");
            assert_eq!(&publish.payload[..], b"hello");
        }
        other => panic!("expected PUBLISH, got {other:?}"),
    }

    drop(command_tx);
    assert!(matches!(
        read_packet(&mut broker, &mut inbound).await,
        ControlPacket::Disconnect(_)
    ));
    timeout(STEP, driver)
        .await
        .expect("driver did not exit after the command channel closed")
        .expect("driver task panicked")
        .expect("driver returned an error");
    assert!(matches!(
        event_rx.recv().await,
        Some(Event::Disconnected(None))
    ));
    assert!(
        event_rx.recv().await.is_none(),
        "driver dropped the event sender"
    );
}

#[tokio::test]
async fn exits_cleanly_when_commands_close_after_an_explicit_disconnect() {
    let (client_side, mut broker) = duplex(64 * 1024);
    let mut pending_socket = Some(client_side);
    let connector = move || {
        let socket = pending_socket
            .take()
            .expect("driver must connect exactly once");
        async move { Ok::<_, io::Error>(BufReader::new(socket)) }
    };
    let (command_tx, command_rx) = mpsc::channel(8);
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let driver = tokio::spawn(
        Driver::new(
            Client::<Instant>::default(),
            connector,
            command_rx,
            event_tx,
        )
        .run(),
    );
    let mut inbound = BytesMut::new();

    command_tx
        .send(Command::Connect(
            ConnectOptions::builder()
                .client_id(ByteString::from_static("driver-test"))
                .build(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        read_packet(&mut broker, &mut inbound).await,
        ControlPacket::Connect(_)
    ));

    let connack = ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .build(),
    );
    broker.write_all(&encode(&connack)).await.unwrap();
    assert!(matches!(
        timeout(STEP, event_rx.recv())
            .await
            .expect("no Connected event"),
        Some(Event::Connected)
    ));

    command_tx.send(Command::Disconnect).await.unwrap();
    assert!(matches!(
        read_packet(&mut broker, &mut inbound).await,
        ControlPacket::Disconnect(_)
    ));
    assert!(matches!(
        timeout(STEP, event_rx.recv())
            .await
            .expect("no Disconnected event"),
        Some(Event::Disconnected(None))
    ));

    drop(command_tx);
    timeout(STEP, driver)
        .await
        .expect("driver did not exit after the command channel closed")
        .expect("driver task panicked")
        .expect("driver returned an error");
}
