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
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::ParserSettings;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::DuplexStream;
use tokio::io::duplex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
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

type Connector = Box<dyn FnMut() -> std::future::Ready<io::Result<BufReader<DuplexStream>>> + Send>;

/// Spawns a driver whose connector hands out a single in-memory socket and
/// returns the broker side of it.
fn spawn_driver() -> (
    JoinHandle<()>,
    mpsc::Sender<Command>,
    mpsc::Receiver<Event>,
    DuplexStream,
) {
    let (client_side, broker) = duplex(64 * 1024);
    let mut pending_socket = Some(client_side);
    let connector: Connector = Box::new(move || {
        let socket = pending_socket
            .take()
            .expect("driver must connect exactly once");
        std::future::ready(Ok(BufReader::new(socket)))
    });
    let (command_tx, command_rx) = mpsc::channel(8);
    let (event_tx, event_rx) = mpsc::channel(8);
    let mut driver = Driver::new(
        Client::<Instant>::default(),
        connector,
        command_rx,
        event_tx,
    );
    let handle = tokio::spawn(async move { driver.run().await });
    (handle, command_tx, event_rx, broker)
}

fn connect_command() -> Command {
    Command::Connect(
        ConnectOptions::builder()
            .client_id(ByteString::from_static("driver-test"))
            .build(),
    )
}

/// Drives the handshake from the broker side until the application observes
/// `Event::Connected`.
async fn complete_handshake(
    broker: &mut DuplexStream,
    inbound: &mut BytesMut,
    event_rx: &mut mpsc::Receiver<Event>,
) {
    assert!(matches!(
        read_packet(broker, inbound).await,
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
}

async fn expect_driver_exit(driver: JoinHandle<()>) {
    timeout(STEP, driver)
        .await
        .expect("driver did not exit after the command channel closed")
        .expect("driver task panicked");
}

#[tokio::test]
async fn connects_publishes_and_disconnects_when_commands_close() {
    let (driver, command_tx, mut event_rx, mut broker) = spawn_driver();
    let mut inbound = BytesMut::new();

    command_tx.send(connect_command()).await.unwrap();
    complete_handshake(&mut broker, &mut inbound, &mut event_rx).await;

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
    expect_driver_exit(driver).await;
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
    let (driver, command_tx, mut event_rx, mut broker) = spawn_driver();
    let mut inbound = BytesMut::new();

    command_tx.send(connect_command()).await.unwrap();
    complete_handshake(&mut broker, &mut inbound, &mut event_rx).await;

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
    expect_driver_exit(driver).await;
}

#[tokio::test]
async fn broker_closing_the_connection_reports_disconnected_and_releases_the_socket() {
    let (driver, command_tx, mut event_rx, mut broker) = spawn_driver();
    let mut inbound = BytesMut::new();

    command_tx.send(connect_command()).await.unwrap();
    complete_handshake(&mut broker, &mut inbound, &mut event_rx).await;

    broker.shutdown().await.unwrap();
    assert!(matches!(
        timeout(STEP, event_rx.recv())
            .await
            .expect("no Disconnected event after EOF"),
        Some(Event::Disconnected(None))
    ));

    // With the socket released and no more commands, the driver must stop.
    drop(command_tx);
    expect_driver_exit(driver).await;
}

#[tokio::test]
async fn broker_protocol_violation_sends_disconnect_and_closes_without_stopping() {
    let (driver, command_tx, mut event_rx, mut broker) = spawn_driver();
    let mut inbound = BytesMut::new();

    command_tx.send(connect_command()).await.unwrap();
    complete_handshake(&mut broker, &mut inbound, &mut event_rx).await;

    // PINGREQ is server-bound; receiving it is a Protocol Error, which the
    // client answers with DISCONNECT 0x82 before closing [MQTT-4.13.1-1].
    broker.write_all(&[0xC0, 0x00]).await.unwrap();
    match read_packet(&mut broker, &mut inbound).await {
        ControlPacket::Disconnect(disconnect) => {
            assert_eq!(disconnect.reason_code, DisconnectReasonCode::ProtocolError);
        }
        other => panic!("expected DISCONNECT, got {other:?}"),
    }
    let mut rest = Vec::new();
    timeout(STEP, broker.read_to_end(&mut rest))
        .await
        .expect("driver did not close the socket")
        .unwrap();
    assert!(
        !driver.is_finished(),
        "a protocol error must not stop the driver"
    );

    drop(command_tx);
    expect_driver_exit(driver).await;
}

#[tokio::test]
async fn rejected_command_does_not_stop_the_driver() {
    let (driver, command_tx, mut event_rx, mut broker) = spawn_driver();
    let mut inbound = BytesMut::new();

    // Not connected yet: the client refuses the publish.
    command_tx
        .send(Command::Publish {
            token: 1,
            message: Message::builder()
                .topic(ByteString::from_static("a/b"))
                .payload(Bytes::from_static(b"early"))
                .build(),
        })
        .await
        .unwrap();

    command_tx.send(connect_command()).await.unwrap();
    complete_handshake(&mut broker, &mut inbound, &mut event_rx).await;

    drop(command_tx);
    expect_driver_exit(driver).await;
}
