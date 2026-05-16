use sansio_mqtt_v5_tokio::{
    ClientMessage, ConnectOptions, Connection, ConnectionError, Event, Payload, Qos, Topic,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Start a mock broker that sends a valid CONNACK and then gracefully closes
/// the connection (FIN, not RST).
///
/// Returns the address string that the listener is bound to.
async fn mock_broker_connack_then_close(addr: &str) -> String {
    let listener = TcpListener::bind(addr).await.unwrap();
    let local = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        // Drain the CONNECT packet sent by the client.
        let mut buf = [0u8; 256];
        stream.read(&mut buf).await.ok();
        // Send CONNACK: success, session not present, no properties.
        // Bytes: 0x20 (type), 0x03 (remaining length), 0x00 (ack flags),
        //        0x00 (reason code = success), 0x00 (properties length)
        stream
            .write_all(&[0x20, 0x03, 0x00, 0x00, 0x00])
            .await
            .ok();
        // Give the client time to receive CONNACK, then initiate a graceful
        // TCP shutdown (FIN) so the client side sees EOF (Ok(0)) rather than a
        // connection reset.
        tokio::time::sleep(Duration::from_millis(50)).await;
        stream.shutdown().await.ok();
        // Keep `stream` alive briefly so the FIN is sent and the OS can finish
        // the TCP teardown before we drop the socket entirely.
        tokio::time::sleep(Duration::from_millis(50)).await;
    });
    local
}

/// Start a mock broker that sends a valid CONNACK and then keeps the connection
/// open, draining any data the client sends without responding further.
///
/// This simulates a broker that never sends PUBACK, leaving QoS 1 messages
/// permanently in-flight.
///
/// Returns the address string that the listener is bound to.
async fn mock_broker_connack_keep_open(addr: &str) -> String {
    let listener = TcpListener::bind(addr).await.unwrap();
    let local = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        // Drain the CONNECT packet.
        let mut buf = [0u8; 256];
        stream.read(&mut buf).await.ok();
        // Send CONNACK.
        stream
            .write_all(&[0x20, 0x03, 0x00, 0x00, 0x00])
            .await
            .ok();
        // Keep draining without responding, so the in-flight queue fills up.
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                break;
            }
        }
    });
    local
}

/// After the connection drops and `Event::Disconnected` is returned, every
/// subsequent `poll()` call on a connection with `backoff: None` must return
/// `Err(ConnectionError::Disconnected)`.
#[tokio::test(flavor = "current_thread")]
async fn no_backoff_returns_disconnected_error_after_disconnect() {
    let addr = mock_broker_connack_then_close("127.0.0.1:0").await;

    let options = ConnectOptions {
        addr: addr.parse().unwrap(),
        backoff: None,
        ..ConnectOptions::default()
    };

    let mut conn = Connection::connect(options)
        .await
        .expect("connect should succeed");

    // Drive the event loop until we see Event::Connected.
    loop {
        let event = conn
            .poll()
            .await
            .expect("no error expected before disconnection");
        if matches!(event, Event::Connected) {
            break;
        }
    }

    // The mock gracefully shuts down the TCP connection after ~50 ms. Poll
    // until we observe the disconnect: the protocol should emit
    // `Event::Disconnected` when it sees EOF (FIN) on the socket.
    loop {
        match conn.poll().await {
            Ok(Event::Disconnected(_)) => break,
            Ok(_other) => continue,
            Err(e) => panic!("expected Ok(Event::Disconnected(_)), got Err({e})"),
        }
    }

    // With `backoff: None` and the state now Terminal, every subsequent
    // `poll()` must return `Err(ConnectionError::Disconnected)` immediately
    // without touching the network.
    for _ in 0..3 {
        match conn.poll().await {
            Err(ConnectionError::Disconnected) => {}
            other => panic!(
                "expected Err(ConnectionError::Disconnected), got {other:?}"
            ),
        }
    }
}

/// Once the number of unacknowledged outbound QoS 1/2 messages reaches
/// `max_out_queued_messages`, `publish()` must return
/// `Err(ConnectionError::QueueFull)`.
#[tokio::test(flavor = "current_thread")]
async fn queue_full_when_max_out_exceeded() {
    let addr = mock_broker_connack_keep_open("127.0.0.1:0").await;

    let options = ConnectOptions {
        addr: addr.parse().unwrap(),
        max_out_queued_messages: 1,
        backoff: None,
        ..ConnectOptions::default()
    };

    let mut conn = Connection::connect(options)
        .await
        .expect("connect should succeed");

    // Drive until connected.
    loop {
        let event = conn
            .poll()
            .await
            .expect("no error expected during connect handshake");
        if matches!(event, Event::Connected) {
            break;
        }
    }

    let topic = Topic::try_new("test/topic").unwrap();
    let payload = Payload::from(b"hello".as_ref());

    let first_msg = ClientMessage {
        topic: topic.clone(),
        payload: payload.clone(),
        qos: Qos::AtLeastOnce,
        ..ClientMessage::default()
    };

    // First publish occupies the single slot.
    conn.publish(first_msg)
        .expect("first publish should succeed");

    let second_msg = ClientMessage {
        topic,
        payload,
        qos: Qos::AtLeastOnce,
        ..ClientMessage::default()
    };

    // Second publish must be rejected because the queue is full.
    match conn.publish(second_msg) {
        Err(ConnectionError::QueueFull) => {}
        other => panic!("expected Err(ConnectionError::QueueFull), got {other:?}"),
    }
}
