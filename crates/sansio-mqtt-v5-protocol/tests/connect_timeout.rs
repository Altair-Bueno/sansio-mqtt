//! The connect deadline bounding every connection attempt.
//!
//! "If the Client does not receive a CONNACK packet from the Server within a
//! reasonable amount of time, the Client SHOULD close the Network Connection"
//! (MQTT v5.0 §3.2). The deadline is armed by the `Command::Connect` that
//! starts the attempt, covers both opening the socket and waiting for CONNACK,
//! and is replaced by the keep-alive deadline once CONNACK arrives.
#![expect(
    clippy::expect_used,
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
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::PingReq;

/// When the tests issue `Command::Connect`; non-zero so a deadline computed
/// from the wrong instant shows up.
const T0: Duration = Duration::from_secs(100);

fn at<Value>(value: Value, at: Duration) -> Timestamped<Value, Duration> {
    Timestamped { value, at }
}

fn connect_options() -> ConnectOptions {
    ConnectOptions::builder()
        .client_id(ByteString::from_static("connect-timeout"))
        .build()
}

fn read(client: &mut Client<Duration>, packet: &ControlPacket, received_at: Duration) {
    let mut bytes = Vec::new();
    packet.encode(&mut bytes).expect("packet should encode");
    assert_eq!(
        client.handle_read(IncomingData {
            bytes: &bytes,
            received_at,
        }),
        Ok(())
    );
}

fn connack(reason_code: ConnackReasonCode) -> ControlPacket {
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other { reason_code })
            .build(),
    )
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

fn connect(client: &mut Client<Duration>, options: ConnectOptions) {
    assert_eq!(
        client.handle_write(at(Command::Connect(options), T0)),
        Ok(())
    );
}

/// Connect at `T0`, then the driver opens the socket at `T0 + 1s`; drains the
/// `OpenSocket` and the CONNECT bytes.
fn awaiting_connack(client: &mut Client<Duration>, options: ConnectOptions) {
    connect(client, options);
    assert_eq!(drain_actions(client), vec![DriverAction::OpenSocket]);
    assert_eq!(
        client.handle_event(at(
            DriverEvent::SocketConnected,
            T0 + Duration::from_secs(1)
        )),
        Ok(())
    );
    assert_eq!(drain_writes(client).len(), 1, "exactly one CONNECT");
}

fn default_deadline() -> Duration {
    T0 + ClientSettings::DEFAULT_CONNECT_TIMEOUT
}

// ── Arming ────────────────────────────────────────────────────────────────

#[test]
fn default_connect_timeout_is_thirty_seconds() {
    assert_eq!(
        ClientSettings::DEFAULT_CONNECT_TIMEOUT,
        Duration::from_secs(30)
    );
    assert_eq!(
        ClientSettings::default().connect_timeout,
        ClientSettings::DEFAULT_CONNECT_TIMEOUT
    );
}

#[test]
fn connect_arms_the_default_connect_deadline_from_the_command_instant() {
    let mut client = Client::<Duration>::default();
    assert_eq!(client.poll_timeout(), None);

    connect(&mut client, connect_options());

    assert_eq!(client.poll_timeout(), Some(default_deadline()));
}

#[test]
fn connect_arms_a_custom_connect_deadline() {
    let mut client = Client::<Duration>::new(
        ClientSettings::builder()
            .connect_timeout(Duration::from_secs(5))
            .build(),
    );

    connect(&mut client, connect_options());

    assert_eq!(client.poll_timeout(), Some(T0 + Duration::from_secs(5)));
}

#[test]
fn rejected_connect_arms_no_deadline() {
    let mut client = Client::<Duration>::default();
    let too_long = ByteString::from("x".repeat(usize::from(u16::MAX) + 1));

    assert_eq!(
        client.handle_write(at(
            Command::Connect(ConnectOptions::builder().client_id(too_long).build()),
            T0
        )),
        Err(Error::StringTooLong)
    );

    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn socket_connected_keeps_the_deadline_armed_by_connect() {
    let mut client = Client::<Duration>::default();

    awaiting_connack(&mut client, connect_options());

    assert_eq!(client.poll_timeout(), Some(default_deadline()));
}

// ── Firing ────────────────────────────────────────────────────────────────

#[test]
fn timeout_before_open_socket_is_polled_withdraws_it_and_disconnects() {
    let mut client = Client::<Duration>::default();
    connect(&mut client, connect_options());

    assert_eq!(client.handle_timeout(default_deadline()), Ok(()));

    assert_eq!(drain_actions(&mut client), vec![]);
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn timeout_while_the_socket_is_opening_cancels_it_and_disconnects() {
    let mut client = Client::<Duration>::default();
    connect(&mut client, connect_options());
    assert_eq!(drain_actions(&mut client), vec![DriverAction::OpenSocket]);

    assert_eq!(client.handle_timeout(default_deadline()), Ok(()));

    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
    assert_eq!(drain_writes(&mut client), Vec::<Bytes>::new());
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn timeout_while_awaiting_connack_closes_the_socket_without_disconnect() {
    let mut client = Client::<Duration>::default();
    awaiting_connack(&mut client, connect_options());

    assert_eq!(client.handle_timeout(default_deadline()), Ok(()));

    assert_eq!(drain_actions(&mut client), vec![DriverAction::CloseSocket]);
    assert_eq!(
        drain_writes(&mut client),
        Vec::<Bytes>::new(),
        "no DISCONNECT before CONNACK"
    );
    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn a_new_attempt_after_a_connect_timeout_arms_a_fresh_deadline() {
    let mut client = Client::<Duration>::default();
    awaiting_connack(&mut client, connect_options());
    assert_eq!(client.handle_timeout(default_deadline()), Ok(()));
    drain_actions(&mut client);
    drain_events(&mut client);

    let retry_at = default_deadline() + Duration::from_secs(1);
    assert_eq!(
        client.handle_write(at(Command::Connect(connect_options()), retry_at)),
        Ok(())
    );

    assert_eq!(drain_actions(&mut client), vec![DriverAction::OpenSocket]);
    assert_eq!(
        client.poll_timeout(),
        Some(retry_at + ClientSettings::DEFAULT_CONNECT_TIMEOUT)
    );
}

// ── Clearing ──────────────────────────────────────────────────────────────

#[test]
fn connack_without_keep_alive_clears_the_connect_deadline() {
    let mut client = Client::<Duration>::new(ClientSettings::builder().keep_alive(None).build());
    awaiting_connack(&mut client, connect_options());

    read(
        &mut client,
        &connack(ConnackReasonCode::Success),
        T0 + Duration::from_secs(2),
    );

    assert_eq!(drain_events(&mut client), vec![Event::Connected]);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn connack_with_keep_alive_replaces_the_connect_deadline_with_keep_alive() {
    let mut client = Client::<Duration>::new(
        ClientSettings::builder()
            .keep_alive(NonZero::new(10))
            .build(),
    );
    awaiting_connack(&mut client, connect_options());
    let received_at = T0 + Duration::from_secs(2);

    read(
        &mut client,
        &connack(ConnackReasonCode::Success),
        received_at,
    );

    assert_eq!(drain_events(&mut client), vec![Event::Connected]);
    assert_eq!(
        client.poll_timeout(),
        Some(received_at + Duration::from_secs(10))
    );
}

#[test]
fn refused_connack_clears_the_connect_deadline() {
    let mut client = Client::<Duration>::default();
    awaiting_connack(&mut client, connect_options());

    read(&mut client, &connack(ConnackReasonCode::NotAuthorized), T0);

    assert_eq!(
        drain_events(&mut client),
        vec![Event::Disconnected(Some(ReasonCode::NotAuthorized))]
    );
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn protocol_error_during_the_handshake_clears_the_connect_deadline() {
    let mut client = Client::<Duration>::default();
    awaiting_connack(&mut client, connect_options());

    read(&mut client, &ControlPacket::PingReq(PingReq {}), T0);

    assert_eq!(
        drain_events(&mut client),
        vec![Event::Disconnected(Some(ReasonCode::ProtocolError))]
    );
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn failed_open_socket_clears_the_connect_deadline() {
    let mut client = Client::<Duration>::default();
    connect(&mut client, connect_options());
    assert_eq!(drain_actions(&mut client), vec![DriverAction::OpenSocket]);

    assert_eq!(
        client.handle_event(at(DriverEvent::SocketError, T0)),
        Ok(())
    );

    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn socket_loss_while_awaiting_connack_clears_the_connect_deadline() {
    for loss in [DriverEvent::SocketClosed, DriverEvent::SocketError] {
        let mut client = Client::<Duration>::default();
        awaiting_connack(&mut client, connect_options());

        assert_eq!(client.handle_event(at(loss, T0)), Ok(()), "{loss:?}");

        assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
        assert_eq!(client.poll_timeout(), None, "{loss:?}");
    }
}

#[test]
fn disconnect_while_connecting_clears_the_connect_deadline() {
    let mut client = Client::<Duration>::default();
    awaiting_connack(&mut client, connect_options());

    assert_eq!(client.handle_write(at(Command::Disconnect, T0)), Ok(()));

    assert_eq!(drain_events(&mut client), vec![Event::Disconnected(None)]);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn close_while_connecting_clears_the_connect_deadline() {
    let mut client = Client::<Duration>::default();
    connect(&mut client, connect_options());

    assert_eq!(client.close(), Ok(()));

    assert_eq!(client.poll_timeout(), None);
}
