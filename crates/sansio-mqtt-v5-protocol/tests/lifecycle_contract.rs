//! Socket lifecycle and error-semantics contract between the protocol and its
//! driver.
//!
//! - The driver reports only what the protocol did not ask for: `OpenSocket` is
//!   answered by `SocketConnected` or `SocketError`; `CloseSocket` gets no
//!   reply; an unsolicited EOF or I/O error is reported as `SocketClosed` /
//!   `SocketError` after the driver has already dropped the socket. A loss
//!   reported while `CloseSocket` is still pending concerns the socket being
//!   closed, never a connection attempt queued after it.
//! - `Err` from the protocol means only "this input was rejected and nothing
//!   changed". Connection failures the protocol handles itself return `Ok(())`
//!   and surface as `Event::Disconnected`.

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
use sansio_mqtt_protocol::Qos;
use sansio_mqtt_protocol::ReasonCode;
use sansio_mqtt_protocol::Timestamped;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnAckProperties;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::PubAck;
use sansio_mqtt_v5_types::PubAckReasonCode;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::Topic;
use sansio_mqtt_v5_types::Utf8String;

/// Stamps a protocol input with `Duration::ZERO`, the instant these tests
/// treat as the start of the connection.
fn at_zero<Value>(value: Value) -> Timestamped<Value, Duration> {
    Timestamped {
        value,
        at: Duration::ZERO,
    }
}

const DISCONNECT_NORMAL: [u8; 2] = [0xE0, 0x00];
const DISCONNECT_MALFORMED: [u8; 4] = [0xE0, 0x02, 0x81, 0x00];
const DISCONNECT_PROTOCOL_ERROR: [u8; 4] = [0xE0, 0x02, 0x82, 0x00];
const DISCONNECT_KEEP_ALIVE_TIMEOUT: [u8; 4] = [0xE0, 0x02, 0x8D, 0x00];

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

fn feed(client: &mut Client<Duration>, bytes: &[u8]) -> Result<(), Error> {
    client.handle_read(IncomingData {
        bytes,
        received_at: Duration::ZERO,
    })
}

fn read(client: &mut Client<Duration>, packet: &ControlPacket) -> Result<(), Error> {
    feed(client, &encode_packet(packet))
}

fn connack(reason_code: ConnackReasonCode) -> ControlPacket {
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other { reason_code })
            .build(),
    )
}

fn connack_with_properties(properties: ConnAckProperties) -> ControlPacket {
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .properties(properties)
            .build(),
    )
}

fn stray_puback() -> ControlPacket {
    ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(NonZero::new(42).unwrap())
            .reason_code(PubAckReasonCode::Success)
            .build(),
    )
}

fn qos0_publish(topic: &str) -> ControlPacket {
    ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .topic(Topic::try_from(Utf8String::try_from(topic).unwrap()).unwrap())
            .payload(sansio_mqtt_v5_types::Payload::new(b"x".as_slice()))
            .build(),
    )
}

fn connect_options() -> ConnectOptions {
    ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .build()
}

fn message(qos: Qos, payload: &[u8]) -> Message {
    Message::builder()
        .topic(ByteString::from_static("t"))
        .payload(Bytes::copy_from_slice(payload))
        .qos(qos)
        .build()
}

fn publish(token: u64, qos: Qos, payload: &[u8]) -> Command {
    Command::Publish {
        token,
        message: message(qos, payload),
    }
}

fn drain_writes(client: &mut Client<Duration>) -> Vec<Bytes> {
    core::iter::from_fn(|| client.poll_write()).collect()
}

fn drain_actions(client: &mut Client<Duration>) -> Vec<DriverAction> {
    core::iter::from_fn(|| client.poll_event()).collect()
}

fn drain_events(client: &mut Client<Duration>) -> Vec<Event> {
    core::iter::from_fn(|| client.poll_read()).collect()
}

/// Issues Connect and asserts the single `OpenSocket` it requests.
fn request_socket(client: &mut Client<Duration>, options: ConnectOptions) {
    assert_eq!(
        client.handle_write(at_zero(Command::Connect(options))),
        Ok(())
    );
    assert_eq!(drain_actions(client), vec![DriverAction::OpenSocket]);
}

/// Connect + SocketConnected; drains the CONNECT bytes.
fn open_connecting(client: &mut Client<Duration>, options: ConnectOptions) {
    request_socket(client, options);
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketConnected)),
        Ok(())
    );
    assert_eq!(drain_writes(client).len(), 1, "exactly one CONNECT");
}

fn connected_with(
    settings: ClientSettings,
    options: ConnectOptions,
    ack: ControlPacket,
) -> Client<Duration> {
    let mut client = Client::<Duration>::new(settings);
    open_connecting(&mut client, options);
    assert_eq!(read(&mut client, &ack), Ok(()));
    assert_eq!(drain_events(&mut client), vec![Event::Connected]);
    client
}

fn connected() -> Client<Duration> {
    connected_with(
        ClientSettings::default(),
        connect_options(),
        connack(ConnackReasonCode::Success),
    )
}

/// Asserts the client accepts a fresh connection attempt, proving it is back
/// in a disconnected state with no socket outstanding.
fn assert_can_reconnect(client: &mut Client<Duration>) {
    request_socket(client, connect_options());
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketConnected)),
        Ok(())
    );
    let writes = drain_writes(client);
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0][0], 0x10,
        "first bytes on a new socket are CONNECT"
    );
}

// ── Unsolicited socket loss ────────────────────────────────────────────────

#[test]
fn socket_error_while_connected_emits_disconnected_and_does_not_close_socket() {
    let mut client = connected();

    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketError)),
        Ok(())
    );

    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(drain_actions(&mut client), vec![]);
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_can_reconnect(&mut client);
}

#[test]
fn socket_closed_while_connected_emits_disconnected_and_does_not_close_socket() {
    let mut client = connected();

    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketClosed)),
        Ok(())
    );

    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(drain_actions(&mut client), vec![]);
    assert_can_reconnect(&mut client);
}

#[test]
fn socket_loss_while_awaiting_connack_emits_disconnected_and_does_not_close_socket() {
    for evt in [DriverEvent::SocketError, DriverEvent::SocketClosed] {
        let mut client = Client::<Duration>::default();
        open_connecting(&mut client, connect_options());

        assert_eq!(client.handle_event(at_zero(evt)), Ok(()), "{evt:?}");

        assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
        assert_eq!(drain_actions(&mut client), vec![]);
        assert_can_reconnect(&mut client);
    }
}

#[test]
fn failed_open_socket_emits_disconnected() {
    let mut client = Client::<Duration>::default();
    request_socket(&mut client, connect_options());

    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketError)),
        Ok(())
    );

    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(drain_actions(&mut client), vec![]);
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_can_reconnect(&mut client);
}

#[test]
fn socket_loss_without_connection_attempt_is_a_no_op() {
    for evt in [DriverEvent::SocketError, DriverEvent::SocketClosed] {
        let mut client = Client::<Duration>::default();

        assert_eq!(client.handle_event(at_zero(evt)), Ok(()), "{evt:?}");

        assert_eq!(drain_events(&mut client), vec![]);
        assert_eq!(drain_actions(&mut client), vec![]);
        assert_can_reconnect(&mut client);
    }
}

#[test]
fn socket_loss_while_already_disconnected_is_a_no_op() {
    for evt in [DriverEvent::SocketError, DriverEvent::SocketClosed] {
        let mut client = connected();
        assert_eq!(
            client.handle_event(at_zero(DriverEvent::SocketClosed)),
            Ok(())
        );
        assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);

        assert_eq!(client.handle_event(at_zero(evt)), Ok(()), "{evt:?}");

        assert_eq!(
            drain_events(&mut client),
            vec![],
            "no duplicate Disconnected"
        );
        assert_eq!(drain_actions(&mut client), vec![]);
        assert_can_reconnect(&mut client);
    }
}

#[test]
fn socket_loss_after_protocol_initiated_close_is_a_no_op() {
    let mut client = connected();
    assert_eq!(client.handle_write(at_zero(Command::Disconnect)), Ok(()));
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);

    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketError)),
        Ok(())
    );

    assert_eq!(drain_events(&mut client), vec![]);
}

#[test]
fn unsent_bytes_are_discarded_when_the_socket_is_lost() {
    let mut client = connected();
    assert_eq!(
        client.handle_write(at_zero(publish(1, Qos::AtMostOnce, b"lost"))),
        Ok(())
    );

    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketError)),
        Ok(())
    );

    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
}

/// The driver writes the DISCONNECT before performing `CloseSocket`; if that
/// write fails it reports the loss while `CloseSocket` is still queued. The
/// report concerns the socket being closed, not the attempt the application
/// started right after, which must still go ahead.
#[test]
fn loss_of_the_closing_socket_is_not_attributed_to_a_new_connection_attempt() {
    for evt in [DriverEvent::SocketError, DriverEvent::SocketClosed] {
        let mut client = connected();
        assert_eq!(client.handle_write(at_zero(Command::Disconnect)), Ok(()));
        assert_eq!(
            client.handle_write(at_zero(Command::Connect(connect_options()))),
            Ok(())
        );
        assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);

        assert_eq!(client.handle_event(at_zero(evt)), Ok(()), "{evt:?}");

        assert_eq!(drain_events(&mut client), vec![], "{evt:?}");
        assert_eq!(
            drain_writes(&mut client),
            Vec::<Bytes>::new(),
            "DISCONNECT belonged to the lost socket"
        );
        assert_eq!(
            drain_actions(&mut client),
            vec![DriverAction::CloseSocket, DriverAction::OpenSocket]
        );
        assert_eq!(
            client.handle_event(at_zero(DriverEvent::SocketConnected)),
            Ok(())
        );
        let writes = drain_writes(&mut client);
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0][0], 0x10, "CONNECT on the new socket");
    }
}

#[test]
fn loss_of_the_closing_socket_discards_every_byte_queued_for_it() {
    let mut client = connected();
    for token in 1..=3 {
        assert_eq!(
            client.handle_write(at_zero(publish(token, Qos::AtMostOnce, b"x"))),
            Ok(())
        );
    }
    assert_eq!(client.handle_write(at_zero(Command::Disconnect)), Ok(()));
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);

    // The first PUBLISH fails to write.
    assert_eq!(client.poll_write().map(|bytes| bytes[0]), Some(0x30));
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketError)),
        Ok(())
    );

    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_events(&mut client), vec![]);
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
    assert_can_reconnect(&mut client);
}

#[test]
fn socket_loss_while_disconnected_discards_unsent_bytes() {
    let mut client = connected();
    assert_eq!(client.handle_write(at_zero(Command::Disconnect)), Ok(()));
    // A driver that already took `CloseSocket` but still holds the DISCONNECT.
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);

    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketClosed)),
        Ok(())
    );

    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
}

// ── Invariant (b): no bytes after CloseSocket until the next SocketConnected ─

#[test]
fn graceful_disconnect_queues_disconnect_bytes_before_close_socket_and_nothing_after() {
    let mut client = connected();

    assert_eq!(client.handle_write(at_zero(Command::Disconnect)), Ok(()));

    assert_eq!(
        drain_writes(&mut client),
        vec![Bytes::from_static(&DISCONNECT_NORMAL)]
    );
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);

    // Every command and timer is now inert on the wire.
    assert_eq!(
        client.handle_write(at_zero(publish(1, Qos::AtLeastOnce, b"x"))),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(
        client.handle_write(at_zero(Command::Disconnect)),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(client.handle_timeout(Duration::from_secs(1_000)), Ok(()));
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_actions(&mut client), vec![]);

    // A new Connect does not put bytes on the wire before SocketConnected.
    request_socket(&mut client, connect_options());
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
}

#[test]
fn reads_without_an_open_socket_are_rejected_without_side_effects() {
    let mut client = Client::<Duration>::default();
    assert_eq!(
        read(&mut client, &qos0_publish("t")),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_actions(&mut client), vec![]);
    assert_eq!(drain_events(&mut client), vec![]);

    // Same while the socket is requested but not yet connected.
    request_socket(&mut client, connect_options());
    assert_eq!(
        read(&mut client, &connack(ConnackReasonCode::Success)),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(drain_events(&mut client), vec![]);
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketConnected)),
        Ok(())
    );
    assert_eq!(drain_writes(&mut client).len(), 1);
}

#[test]
fn packets_after_a_fatal_one_in_the_same_read_are_discarded() {
    let mut client = connected();
    let mut bytes = encode_packet(&stray_puback()).to_vec();
    bytes.extend_from_slice(&encode_packet(&qos0_publish("t")));
    // A trailing partial packet must not survive into the next connection.
    bytes.extend_from_slice(&[0x30, 0x10, 0x00]);

    assert_eq!(feed(&mut client, &bytes), Ok(()));

    assert_eq!(
        drain_events(&mut client),
        vec![Event::Disconnected(Some(ReasonCode::ProtocolError))]
    );
    assert_eq!(
        drain_writes(&mut client),
        vec![Bytes::from_static(&DISCONNECT_PROTOCOL_ERROR)]
    );
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);

    // Reconnect and confirm the leftover partial packet was dropped: a full
    // CONNACK now parses cleanly.
    assert_can_reconnect(&mut client);
    assert_eq!(
        read(&mut client, &connack(ConnackReasonCode::Success)),
        Ok(())
    );
    assert_eq!(drain_events(&mut client), vec![Event::Connected]);
}

// ── Invariant (a): no OpenSocket while a socket is open ────────────────────

#[test]
fn connect_while_a_connection_exists_is_rejected_without_open_socket() {
    let mut client = Client::<Duration>::default();
    request_socket(&mut client, connect_options());
    assert_eq!(
        client.handle_write(at_zero(Command::Connect(connect_options()))),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(drain_actions(&mut client), vec![]);

    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketConnected)),
        Ok(())
    );
    assert_eq!(
        client.handle_write(at_zero(Command::Connect(connect_options()))),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(drain_actions(&mut client), vec![]);

    let mut client = connected();
    assert_eq!(
        client.handle_write(at_zero(Command::Connect(connect_options()))),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(drain_actions(&mut client), vec![]);
}

#[test]
fn unrequested_socket_connected_is_rejected_without_side_effects() {
    // Never asked for a socket.
    let mut client = Client::<Duration>::default();
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketConnected)),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_can_reconnect(&mut client);

    // Already connected.
    let mut client = connected();
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketConnected)),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(
        client.handle_write(at_zero(publish(1, Qos::AtMostOnce, b"x"))),
        Ok(())
    );

    // Disconnected after a previous connection: stored options must not be
    // replayed on a socket the protocol never requested.
    let mut client = connected();
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketClosed)),
        Ok(())
    );
    assert_eq!(
        client.handle_event(at_zero(DriverEvent::SocketConnected)),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
}

#[test]
fn disconnect_before_open_socket_is_polled_withdraws_the_request() {
    let mut client = Client::<Duration>::default();
    assert_eq!(
        client.handle_write(at_zero(Command::Connect(connect_options()))),
        Ok(())
    );

    assert_eq!(client.handle_write(at_zero(Command::Disconnect)), Ok(()));

    assert_eq!(drain_actions(&mut client), vec![]);
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_can_reconnect(&mut client);
}

#[test]
fn disconnect_after_open_socket_is_polled_cancels_it_with_close_socket() {
    let mut client = Client::<Duration>::default();
    request_socket(&mut client, connect_options());

    assert_eq!(client.handle_write(at_zero(Command::Disconnect)), Ok(()));

    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
}

// ── Handled connection failures: Ok + Event::Disconnected ──────────────────

#[test]
fn malformed_packet_disconnects_with_reason_and_returns_ok() {
    let mut client = connected();

    // PUBLISH with a reserved QoS of 3.
    assert_eq!(feed(&mut client, &[0x36, 0x00]), Ok(()));

    assert_eq!(
        drain_events(&mut client),
        vec![Event::Disconnected(Some(ReasonCode::MalformedPacket))]
    );
    assert_eq!(
        drain_writes(&mut client),
        vec![Bytes::from_static(&DISCONNECT_MALFORMED)]
    );
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
}

#[test]
fn protocol_violation_disconnects_with_reason_and_returns_ok() {
    let mut client = connected();

    assert_eq!(read(&mut client, &stray_puback()), Ok(()));

    assert_eq!(
        drain_events(&mut client),
        vec![Event::Disconnected(Some(ReasonCode::ProtocolError))]
    );
    assert_eq!(
        drain_writes(&mut client),
        vec![Bytes::from_static(&DISCONNECT_PROTOCOL_ERROR)]
    );
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
}

#[test]
fn protocol_violation_during_handshake_disconnects_and_returns_ok() {
    let mut client = Client::<Duration>::default();
    open_connecting(&mut client, connect_options());

    assert_eq!(read(&mut client, &stray_puback()), Ok(()));

    assert_eq!(
        drain_events(&mut client),
        vec![Event::Disconnected(Some(ReasonCode::ProtocolError))]
    );
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
}

#[test]
fn rejected_connack_disconnects_with_broker_reason_and_returns_ok() {
    let mut client = Client::<Duration>::default();
    open_connecting(&mut client, connect_options());

    assert_eq!(
        read(&mut client, &connack(ConnackReasonCode::NotAuthorized)),
        Ok(())
    );

    assert_eq!(
        drain_events(&mut client),
        vec![Event::Disconnected(Some(ReasonCode::NotAuthorized))]
    );
    // [MQTT-3.2.2-7] The server closes the connection; the client sends
    // nothing more on it.
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
    assert_can_reconnect(&mut client);
}

#[test]
fn connect_timeout_disconnects_and_returns_ok() {
    let mut client = Client::<Duration>::default();
    open_connecting(&mut client, connect_options());

    assert_eq!(client.handle_timeout(Duration::from_secs(30)), Ok(()));

    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
    assert_can_reconnect(&mut client);
}

#[test]
fn timeout_without_connection_attempt_is_a_no_op() {
    let mut client = Client::<Duration>::default();

    assert_eq!(client.handle_timeout(Duration::from_secs(30)), Ok(()));

    assert_eq!(drain_events(&mut client), vec![]);
    assert_eq!(drain_actions(&mut client), vec![]);
}

#[test]
fn keep_alive_timeout_disconnects_with_reason_and_returns_ok() {
    let mut client = connected_with(
        ClientSettings::default(),
        ConnectOptions::builder()
            .client_id(ByteString::from_static("test-client"))
            .keep_alive(NonZero::new(10).unwrap())
            .build(),
        connack(ConnackReasonCode::Success),
    );
    // Idle interval elapses: PINGREQ.
    assert_eq!(client.handle_timeout(Duration::from_secs(10)), Ok(()));
    assert_eq!(
        drain_writes(&mut client),
        vec![Bytes::from_static(&[0xC0, 0x00])]
    );

    // No PINGRESP: keep-alive timeout.
    assert_eq!(client.handle_timeout(Duration::from_secs(15)), Ok(()));

    assert_eq!(
        drain_events(&mut client),
        vec![Event::Disconnected(Some(ReasonCode::KeepAliveTimeout))]
    );
    assert_eq!(
        drain_writes(&mut client),
        vec![Bytes::from_static(&DISCONNECT_KEEP_ALIVE_TIMEOUT)]
    );
    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
}

// ── Rejected input leaves state unchanged ──────────────────────────────────

#[test]
fn invalid_connect_options_are_rejected_without_requesting_a_socket() {
    let mut client = Client::<Duration>::default();
    let too_long = ByteString::from("x".repeat(usize::from(u16::MAX) + 1));

    assert_eq!(
        client.handle_write(at_zero(Command::Connect(
            ConnectOptions::builder().client_id(too_long).build()
        ))),
        Err(Error::StringTooLong)
    );

    assert_eq!(drain_actions(&mut client), vec![]);
    assert_can_reconnect(&mut client);
}

#[test]
fn rejected_publish_does_not_consume_a_packet_identifier() {
    let mut client = connected_with(
        ClientSettings::default(),
        connect_options(),
        connack_with_properties(
            ConnAckProperties::builder()
                .maximum_packet_size(NonZero::new(64).unwrap())
                .build(),
        ),
    );

    assert_eq!(
        client.handle_write(at_zero(publish(1, Qos::AtLeastOnce, &[0u8; 128]))),
        Err(Error::PacketTooLarge)
    );
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());

    assert_eq!(
        client.handle_write(at_zero(publish(2, Qos::AtLeastOnce, b"ok"))),
        Ok(())
    );
    let writes = drain_writes(&mut client);
    assert_eq!(writes.len(), 1);
    // Fixed header (2) + topic length (2) + "t" (1) → packet id at [5..7].
    assert_eq!(
        &writes[0][5..7],
        &[0x00, 0x01],
        "first packet id is still 1"
    );
}
