//! Keep-alive scheduling while connected.
//!
//! "It is the responsibility of the Client to ensure that the interval between
//! MQTT Control Packets being sent does not exceed the Keep Alive value. If
//! Keep Alive is non-zero and in the absence of sending any other MQTT Control
//! Packets, the Client MUST send a PINGREQ packet" [MQTT-3.1.2-20]. A server
//! that hears nothing for 1.5 × Keep Alive closes the connection
//! [MQTT-3.1.2-22].
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
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::PublishOptions;
use sansio_mqtt_protocol::Qos;
use sansio_mqtt_protocol::Timestamped;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_wire::ConnAck;
use sansio_mqtt_v5_wire::ConnAckKind;
use sansio_mqtt_v5_wire::ConnackReasonCode;
use sansio_mqtt_v5_wire::ControlPacket;
use sansio_mqtt_v5_wire::PingResp;

/// When the tests issue `Command::Connect`; non-zero so a deadline computed
/// from the wrong instant shows up.
const T0: Duration = Duration::from_secs(100);

const KEEP_ALIVE: Duration = Duration::from_secs(10);

const PINGREQ: [u8; 2] = [0xC0, 0x00];

fn secs(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

fn at<Value>(value: Value, at: Duration) -> Timestamped<Value, Duration> {
    Timestamped { value, at }
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

fn drain_writes(client: &mut Client<Duration>) -> Vec<Bytes> {
    core::iter::from_fn(|| client.poll_write()).collect()
}

fn publish(client: &mut Client<Duration>, now: Duration) {
    let message = Message::builder()
        .topic(ByteString::from_static("t"))
        .payload(Bytes::from_static(b"x"))
        .qos(Qos::AtMostOnce)
        .build();
    let command = Command::Publish(PublishOptions::builder().token(0).message(message).build());
    assert_eq!(client.handle_write(at(command, now)), Ok(()));
    assert_eq!(drain_writes(client).len(), 1, "exactly one PUBLISH");
}

/// Connects with a 10 s keep-alive: `Command::Connect` at `T0`, the socket
/// (and with it CONNECT) at `connect_sent_at`, CONNACK at `connack_at`.
fn connected(connect_sent_at: Duration, connack_at: Duration) -> Client<Duration> {
    let mut client = Client::<Duration>::new(
        ClientSettings::builder()
            .keep_alive(NonZero::new(10))
            .build(),
    );
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("keep-alive"))
        .build();
    assert_eq!(
        client.handle_write(at(Command::Connect(options), T0)),
        Ok(())
    );
    assert_eq!(client.poll_event(), Some(DriverAction::OpenSocket));
    assert_eq!(
        client.handle_event(at(DriverEvent::SocketConnected, connect_sent_at)),
        Ok(())
    );
    assert_eq!(drain_writes(&mut client).len(), 1, "exactly one CONNECT");
    let connack = ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .build(),
    );
    read(&mut client, &connack, connack_at);
    assert!(matches!(client.poll_read(), Some(Event::Connected(_))));
    client
}

/// Fires every timeout the client asks for up to and including `until`, the
/// way a driver would, and returns the instants at which it wrote PINGREQ.
/// Each PINGREQ is answered with a PINGRESP `rtt` later when `rtt` is some.
fn run_until(
    client: &mut Client<Duration>,
    until: Duration,
    rtt: Option<Duration>,
) -> Vec<Duration> {
    let mut pings = Vec::new();
    let mut pending_pong = None;
    loop {
        let deadline = client.poll_timeout();
        if let Some(pong_at) = pending_pong
            && deadline.is_none_or(|deadline| pong_at <= deadline)
            && pong_at <= until
        {
            read(client, &ControlPacket::PingResp(PingResp {}), pong_at);
            pending_pong = None;
            continue;
        }
        let Some(deadline) = deadline.filter(|deadline| *deadline <= until) else {
            return pings;
        };
        assert_eq!(client.handle_timeout(deadline), Ok(()));
        for bytes in drain_writes(client) {
            if bytes == PINGREQ.as_slice() {
                pings.push(deadline);
                pending_pong = rtt.map(|rtt| deadline + rtt);
            }
        }
        assert!(
            !matches!(client.poll_read(), Some(Event::Disconnected(_))),
            "unexpected disconnect at {deadline:?}"
        );
    }
}

#[test]
fn idle_connection_pings_one_interval_after_connect() {
    let mut client = connected(T0, T0);

    let pings = run_until(&mut client, T0 + KEEP_ALIVE, None);

    assert_eq!(pings, vec![T0 + KEEP_ALIVE]);
}

#[test]
fn first_ping_counts_from_connect_not_from_connack() {
    // CONNECT is the last packet sent; a slow CONNACK does not reset the
    // interval the server measures.
    let mut client = connected(T0, T0 + secs(3));

    let pings = run_until(&mut client, T0 + KEEP_ALIVE, None);

    assert_eq!(pings, vec![T0 + KEEP_ALIVE]);
}

#[test]
fn traffic_after_connack_defers_the_ping_to_one_interval_after_that_traffic() {
    let mut client = connected(T0, T0);
    publish(&mut client, T0 + secs(1));

    let pings = run_until(&mut client, T0 + secs(1) + KEEP_ALIVE, None);

    assert_eq!(pings, vec![T0 + secs(1) + KEEP_ALIVE]);
}

#[test]
fn answered_ping_is_followed_by_the_next_one_a_full_interval_later() {
    let mut client = connected(T0, T0);

    let pings = run_until(&mut client, T0 + secs(35), Some(secs(1)));

    assert_eq!(pings, vec![T0 + secs(10), T0 + secs(20), T0 + secs(30)]);
}

#[test]
fn outbound_gaps_never_exceed_keep_alive_with_sporadic_traffic() {
    let mut client = connected(T0, T0);
    let mut last_sent = T0;
    let mut now = T0;

    for gap in [1, 7, 3, 9, 12, 2, 25, 4, 15, 8, 1, 31] {
        let publish_at = now + secs(gap);
        for ping in run_until(&mut client, publish_at, Some(secs(1))) {
            assert!(
                ping <= last_sent + KEEP_ALIVE,
                "{ping:?} after {last_sent:?}"
            );
            last_sent = ping;
        }
        publish(&mut client, publish_at);
        assert!(
            publish_at <= last_sent + KEEP_ALIVE,
            "publish at {publish_at:?} after {last_sent:?}"
        );
        last_sent = publish_at;
        now = publish_at;
    }
}
