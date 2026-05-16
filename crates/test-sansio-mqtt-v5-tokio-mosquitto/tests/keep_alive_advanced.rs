use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// keep_alive=0 disables the ping mechanism. The connection must survive at
/// least 3s of idling without PINGREQ/PINGRESP or a disconnect.
#[tokio::test]
async fn zero_keep_alive_connection_stays_alive() {
    let (_c, port) = anonymous_broker().await;

    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("ka-zero").expect("id"),
            keep_alive: None, // None means 0 (disabled)
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_client, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    // Idle for 3s — connection must remain alive (no unexpected disconnect or
    // error)
    let result = tokio::time::timeout(Duration::from_secs(3), el.poll()).await;
    match result {
        Err(_elapsed) => { /* success: still alive after 3s */ }
        Ok(Err(e)) => panic!("connection dropped with keep_alive=0: {e:?}"),
        Ok(Ok(ev)) => panic!("unexpected event with keep_alive=0: {ev:?}"),
    }
}

/// With keep_alive=2s, publishing a message at t≈1.5s resets the keep-alive
/// deadline so the connection survives past t=2s without a disconnect.
#[tokio::test]
async fn traffic_resets_keep_alive_deadline() {
    let (_c, port) = anonymous_broker().await;

    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("ka-reset").expect("id"),
            keep_alive: core::num::NonZero::new(2u16), // 2 second keep-alive
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (client, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    // Subscribe so we have somewhere to publish
    client.subscribe(sub("ka/reset")).await.expect("subscribe");

    // At t≈1.5s publish a message — this should reset the keep-alive deadline
    tokio::time::sleep(Duration::from_millis(1500)).await;
    client
        .publish(msg("ka/reset", b"ping", Qos::AtMostOnce))
        .await
        .expect("publish");

    // Poll for up to 2s past the original keep-alive deadline — connection must
    // still be alive
    let result = tokio::time::timeout(Duration::from_millis(2000), el.poll()).await;
    match result {
        Err(_elapsed) => { /* success: alive past 2s deadline */ }
        Ok(Err(e)) => panic!("connection dropped after traffic reset: {e:?}"),
        Ok(Ok(Event::Message(_))) => { /* received own QoS0 echo — acceptable */ }
        Ok(Ok(ev)) => panic!("unexpected event: {ev:?}"),
    }
}
