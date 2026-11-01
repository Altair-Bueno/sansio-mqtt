//! End-to-end check of the tokio driver against the real v5 client, using an
//! in-memory duplex stream as the "broker" side of the socket.
//!
//! The harness plays the application: it owns the driver and awaits
//! [`Driver::next`] inside a `select!` with incoming commands, so every
//! submitted command cancels an in-flight `next` call.

use std::collections::VecDeque;
use std::io;
use std::num::NonZero;
use std::pin::Pin;
use std::time::Duration;

use bytes::Bytes;
use bytes::BytesMut;
use encode::Encodable;
use sansio_mqtt_protocol::ByteString;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::ReasonCode;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::ParserSettings;
use sansio_mqtt_v5_types::PingReq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::DuplexStream;
use tokio::io::duplex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Instant;
use tokio::time::timeout;
use winnow::Parser;
use winnow::error::ContextError;
use winnow::error::ErrMode;
use winnow::stream::Partial;

const STEP: Duration = Duration::from_secs(2);
const BUFFER: usize = 64 * 1024;

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

/// How the connector answers one connection attempt.
#[derive(Debug, Clone, Copy)]
enum Dial {
    Accept,
    Refuse,
    /// The attempt never completes, like a TCP connect to a black hole.
    Hang,
}

type Socket = BufReader<DuplexStream>;
type Dialing = Pin<Box<dyn Future<Output = io::Result<Socket>> + Send>>;

/// Commands submitted back to back, with no `next` call in between.
type Request = (Vec<Command>, oneshot::Sender<Vec<Result<(), Error>>>);

struct Harness {
    commands: mpsc::Sender<Request>,
    events: mpsc::Receiver<Event>,
    /// Broker sides of the accepted connections, current one first.
    brokers: VecDeque<DuplexStream>,
    inbound: BytesMut,
}

impl Harness {
    fn spawn(dials: &[Dial]) -> Self {
        Self::spawn_with_buffer(dials, BUFFER)
    }

    fn spawn_with_buffer(dials: &[Dial], buffer: usize) -> Self {
        Self::spawn_with(dials, buffer, ClientSettings::default())
    }

    fn spawn_with_settings(dials: &[Dial], settings: ClientSettings) -> Self {
        Self::spawn_with(dials, BUFFER, settings)
    }

    fn spawn_with(dials: &[Dial], buffer: usize, settings: ClientSettings) -> Self {
        let mut attempts: VecDeque<Dialing> = VecDeque::new();
        let mut brokers = VecDeque::new();
        for dial in dials {
            match dial {
                Dial::Accept => {
                    let (client_side, broker) = duplex(buffer);
                    attempts.push_back(Box::pin(std::future::ready(Ok(BufReader::new(
                        client_side,
                    )))));
                    brokers.push_back(broker);
                }
                Dial::Refuse => attempts.push_back(Box::pin(std::future::ready(Err(
                    io::ErrorKind::ConnectionRefused.into(),
                )))),
                Dial::Hang => attempts.push_back(Box::pin(std::future::pending())),
            }
        }
        let connector = move || {
            attempts
                .pop_front()
                .expect("driver connected more often than the test allows")
        };

        let mut driver = Driver::new(Client::<Instant>::new(settings), connector);
        let (commands, mut requests) = mpsc::channel::<Request>(1);
        let (event_tx, events) = mpsc::channel(64);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Commands first, so a test can queue commands before the
                    // driver reacts to anything it did to the broker side.
                    biased;
                    Some((commands, reply)) = requests.recv() => {
                        let results = commands
                            .into_iter()
                            .map(|command| driver.submit(command))
                            .collect();
                        let _ = reply.send(results);
                    }
                    event = driver.next() => if event_tx.send(event).await.is_err() { break },
                }
            }
        });

        Self {
            commands,
            events,
            brokers,
            inbound: BytesMut::new(),
        }
    }

    async fn submit(&self, command: Command) -> Result<(), Error> {
        let mut results = self.submit_all(vec![command]).await;
        results.pop().expect("one result per command")
    }

    /// Submits `commands` back to back, without the driver running in
    /// between.
    async fn submit_all(&self, commands: Vec<Command>) -> Vec<Result<(), Error>> {
        let (reply, results) = oneshot::channel();
        self.commands.send((commands, reply)).await.unwrap();
        results.await.expect("driver task stopped")
    }

    async fn event(&mut self) -> Event {
        self.event_within(STEP).await
    }

    async fn event_within(&mut self, limit: Duration) -> Event {
        timeout(limit, self.events.recv())
            .await
            .expect("no event from the driver")
            .expect("driver task stopped")
    }

    fn broker(&mut self) -> &mut DuplexStream {
        self.brokers.front_mut().expect("no open broker connection")
    }

    async fn packet(&mut self) -> ControlPacket {
        let broker = self.brokers.front_mut().expect("no open broker connection");
        read_packet(broker, &mut self.inbound).await
    }

    /// Asserts the client closes the current connection without sending
    /// anything else, then moves on to the next one.
    async fn expect_closed(&mut self) {
        let mut rest = Vec::new();
        timeout(STEP, self.broker().read_to_end(&mut rest))
            .await
            .expect("client did not close the socket")
            .unwrap();
        assert!(rest.is_empty(), "unexpected bytes before close: {rest:?}");
        self.next_connection();
    }

    fn next_connection(&mut self) {
        self.brokers.pop_front();
        self.inbound.clear();
    }

    /// Submits CONNECT and completes the handshake from the broker side.
    async fn connect(&mut self) {
        self.submit(connect_command()).await.unwrap();
        self.accept_connect().await;
    }

    /// Reads CONNECT on the current connection and accepts it.
    async fn accept_connect(&mut self) {
        assert!(matches!(self.packet().await, ControlPacket::Connect(_)));

        let connack = ControlPacket::ConnAck(
            ConnAck::builder()
                .kind(ConnAckKind::Other {
                    reason_code: ConnackReasonCode::Success,
                })
                .build(),
        );
        self.broker().write_all(&encode(&connack)).await.unwrap();
        assert_eq!(self.event().await, Event::Connected);
    }
}

fn connect_command() -> Command {
    Command::Connect(
        ConnectOptions::builder()
            .client_id(ByteString::from_static("driver-test"))
            .build(),
    )
}

fn publish_command(token: u64, payload: Bytes) -> Command {
    Command::Publish {
        token,
        message: Message::builder()
            .topic(ByteString::from_static("a/b"))
            .payload(payload)
            .build(),
    }
}

#[tokio::test]
async fn publishes_and_disconnects_gracefully() {
    let mut harness = Harness::spawn(&[Dial::Accept]);
    harness.connect().await;

    harness
        .submit(publish_command(7, Bytes::from_static(b"hello")))
        .await
        .unwrap();
    match harness.packet().await {
        ControlPacket::Publish(publish) => {
            let topic: &str = &publish.topic;
            assert_eq!(topic, "a/b");
            assert_eq!(&publish.payload[..], b"hello");
        }
        other => panic!("expected PUBLISH, got {other:?}"),
    }

    harness.submit(Command::Disconnect).await.unwrap();
    assert!(matches!(
        harness.packet().await,
        ControlPacket::Disconnect(_)
    ));
    harness.expect_closed().await;
    assert_eq!(harness.event().await, Event::Disconnected(None));
}

#[tokio::test]
async fn broker_closing_the_connection_reports_disconnected_and_allows_reconnecting() {
    let mut harness = Harness::spawn(&[Dial::Accept, Dial::Accept]);
    harness.connect().await;

    harness.broker().shutdown().await.unwrap();
    assert_eq!(harness.event().await, Event::Disconnected(None));

    // The driver released the dead socket, so a new connection can be opened.
    harness.next_connection();
    harness.connect().await;
}

#[tokio::test]
async fn broker_protocol_violation_sends_disconnect_closes_and_allows_reconnecting() {
    let mut harness = Harness::spawn(&[Dial::Accept, Dial::Accept]);
    harness.connect().await;

    // PINGREQ is server-bound; receiving it is a Protocol Error, which the
    // client answers with DISCONNECT 0x82 before closing [MQTT-4.13.1-1].
    let pingreq = encode(&ControlPacket::PingReq(PingReq {}));
    harness.broker().write_all(&pingreq).await.unwrap();
    match harness.packet().await {
        ControlPacket::Disconnect(disconnect) => {
            assert_eq!(disconnect.reason_code, DisconnectReasonCode::ProtocolError);
        }
        other => panic!("expected DISCONNECT, got {other:?}"),
    }
    harness.expect_closed().await;
    assert_eq!(
        harness.event().await,
        Event::Disconnected(Some(ReasonCode::ProtocolError))
    );

    harness.connect().await;
}

#[tokio::test]
async fn rejected_command_is_returned_to_the_caller() {
    let mut harness = Harness::spawn(&[Dial::Accept]);

    // Not connected yet: the client refuses the publish.
    assert!(
        harness
            .submit(publish_command(1, Bytes::from_static(b"early")))
            .await
            .is_err()
    );

    harness.connect().await;
}

#[tokio::test]
async fn refused_connection_reports_disconnected_and_allows_retrying() {
    let mut harness = Harness::spawn(&[Dial::Refuse, Dial::Accept]);

    harness.submit(connect_command()).await.unwrap();
    assert_eq!(harness.event().await, Event::Disconnected(None));

    harness.connect().await;
}

#[tokio::test]
async fn cancelling_next_mid_write_does_not_corrupt_outgoing_packets() {
    // A tiny duplex buffer makes the PUBLISH write block until the broker
    // reads, so every following `submit` cancels `next` mid-write.
    let mut harness = Harness::spawn_with_buffer(&[Dial::Accept], 64);
    harness.connect().await;

    let payload = Bytes::from(vec![0xAB; 4096]);
    harness
        .submit(publish_command(1, payload.clone()))
        .await
        .unwrap();
    for filter in ["x/1", "x/2", "x/3"] {
        harness
            .submit(Command::Subscribe(
                SubscribeOptions::builder()
                    .subscriptions(vec![Subscription::builder().filter(filter).build()])
                    .build(),
            ))
            .await
            .unwrap();
    }

    match harness.packet().await {
        ControlPacket::Publish(publish) => assert_eq!(publish.payload[..], payload[..]),
        other => panic!("expected PUBLISH, got {other:?}"),
    }
    for _ in 0..3 {
        assert!(matches!(
            harness.packet().await,
            ControlPacket::Subscribe(_)
        ));
    }
}

#[tokio::test]
async fn failed_disconnect_write_does_not_fail_the_following_connect() {
    let mut harness = Harness::spawn(&[Dial::Accept, Dial::Accept]);
    harness.connect().await;

    // The broker vanishes; the driver only finds out when writing the
    // DISCONNECT, after the application already asked to reconnect.
    harness.next_connection();
    let results = harness
        .submit_all(vec![Command::Disconnect, connect_command()])
        .await;
    assert_eq!(results, vec![Ok(()), Ok(())]);

    assert_eq!(harness.event().await, Event::Disconnected(None));
    harness.accept_connect().await;
}

#[tokio::test]
async fn failed_write_discards_the_rest_of_the_closing_socket_bytes() {
    let mut harness = Harness::spawn(&[Dial::Accept, Dial::Accept]);
    harness.connect().await;

    harness.next_connection();
    let results = harness
        .submit_all(vec![
            publish_command(1, Bytes::from_static(b"one")),
            publish_command(2, Bytes::from_static(b"two")),
            publish_command(3, Bytes::from_static(b"three")),
            Command::Disconnect,
        ])
        .await;
    assert!(results.iter().all(Result::is_ok), "{results:?}");

    assert_eq!(harness.event().await, Event::Disconnected(None));
    harness.connect().await;
}

#[tokio::test(start_paused = true)]
async fn keep_alive_timeout_closes_a_connection_whose_peer_stopped_reading() {
    let mut harness = Harness::spawn_with(
        &[Dial::Accept],
        64,
        ClientSettings::builder()
            .keep_alive(NonZero::new(1))
            .build(),
    );
    harness.submit(connect_command()).await.unwrap();
    harness.accept_connect().await;

    // The broker stops reading, so this PUBLISH never fits in the socket
    // buffer and its write stalls.
    harness
        .submit(publish_command(1, Bytes::from(vec![0xAB; 4096])))
        .await
        .unwrap();

    assert_eq!(
        harness.event_within(Duration::from_secs(60)).await,
        Event::Disconnected(Some(ReasonCode::KeepAliveTimeout))
    );
    // The driver gave up on the stalled socket and dropped it.
    let mut rest = Vec::new();
    timeout(STEP, harness.broker().read_to_end(&mut rest))
        .await
        .expect("client did not close the socket")
        .unwrap();
}

fn settings_with_connect_timeout(connect_timeout: Duration) -> ClientSettings {
    ClientSettings::builder()
        .connect_timeout(connect_timeout)
        .build()
}

#[tokio::test(start_paused = true)]
async fn connection_attempt_that_never_completes_times_out_and_allows_retrying() {
    let connect_timeout = Duration::from_secs(5);
    let mut harness = Harness::spawn_with_settings(
        &[Dial::Hang, Dial::Accept],
        settings_with_connect_timeout(connect_timeout),
    );
    let started = Instant::now();

    harness.submit(connect_command()).await.unwrap();

    assert_eq!(
        harness.event_within(Duration::from_secs(60)).await,
        Event::Disconnected(None)
    );
    assert!(started.elapsed() >= connect_timeout);
    // The abandoned attempt released the connection slot.
    harness.connect().await;
}

#[tokio::test(start_paused = true)]
async fn broker_that_never_sends_connack_times_out_and_the_socket_is_closed() {
    let connect_timeout = Duration::from_secs(5);
    let mut harness = Harness::spawn_with_settings(
        &[Dial::Accept, Dial::Accept],
        settings_with_connect_timeout(connect_timeout),
    );
    let started = Instant::now();

    harness.submit(connect_command()).await.unwrap();
    assert!(matches!(harness.packet().await, ControlPacket::Connect(_)));

    assert_eq!(
        harness.event_within(Duration::from_secs(60)).await,
        Event::Disconnected(None)
    );
    assert!(started.elapsed() >= connect_timeout);
    // No DISCONNECT: no session was ever established [MQTT-3.2.2-7].
    harness.expect_closed().await;
    harness.connect().await;
}
