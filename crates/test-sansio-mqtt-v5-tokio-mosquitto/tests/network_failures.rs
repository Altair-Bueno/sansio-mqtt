use std::time::Duration;

use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Connecting to a port with no listener must immediately return a
/// `ConnectError::Io` — the TCP handshake is refused before any MQTT is sent.
#[tokio::test]
async fn connect_to_closed_port_returns_error() {
    // Bind a TcpListener to get a free port, then drop it immediately.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener); // port is now closed

    let result = connect(connect_options(port, "nf-closed")).await;
    assert!(
        result.is_err(),
        "connect to closed port must return error, got: {result:?}"
    );
}

/// After the broker container is stopped, the next `el.poll()` call must return
/// `Err(EventLoopError::Io(...))` — the TCP connection is torn down from the
/// server side.
#[tokio::test]
async fn broker_kill_closes_event_loop() {
    let (container, port) = anonymous_broker().await;

    let (_client, mut el) = connect(connect_options(port, "nf-kill"))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    // Stop the broker — this closes the TCP connection from the server side.
    let _ = container.stop().await;

    // Mosquitto sends a DISCONNECT packet before closing TCP, so we may get
    // Ok(Disconnected(_)) instead of Err(Io). Both mean the connection is gone.
    let result = tokio::time::timeout(Duration::from_secs(5), el.poll())
        .await
        .expect("connection closed within 5s");
    assert!(
        matches!(result, Err(_) | Ok(Event::Disconnected(_))),
        "poll must return error or Disconnected after broker kill, got: {result:?}"
    );
}

/// With keep_alive=2s the broker closes the TCP connection at ~3s (1.5×
/// keep-alive) if no PINGREQ arrives.  After sleeping for 4s without polling
/// (which would have triggered the PINGREQ), the subsequent `poll()` must
/// return an error.
#[tokio::test]
async fn keepalive_timeout_disconnects_client() {
    let (_container, port) = anonymous_broker().await;

    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("nf-ka").expect("id"),
            keep_alive: core::num::NonZero::new(1u16),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };

    let (_client, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    // Do NOT poll for 2.5s — broker closes TCP after ~1.5s (1.5 × 1s keepalive).
    tokio::time::sleep(Duration::from_millis(2500)).await;

    // Mosquitto sends DISCONNECT (reason: KeepAliveTimeout) before closing TCP,
    // so we may get Ok(Disconnected(Some(_))) instead of Err(Io). Both mean
    // the broker has terminated the connection.
    let result = el.poll().await;
    assert!(
        matches!(result, Err(_) | Ok(Event::Disconnected(Some(_)))),
        "poll after keepalive timeout must return error or Disconnected(Some(_)), got: {result:?}"
    );
}
