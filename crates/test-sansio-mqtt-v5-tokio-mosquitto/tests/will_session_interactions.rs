use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Will fires at min(will_delay, session_expiry_interval). Here will_delay=10s
/// but session_expiry=2s, so the will must fire at ~2s. [MQTT-3.1.3-9]
#[tokio::test]
async fn will_fires_at_session_expiry_when_delay_exceeds_expiry() {
    let (_c, port) = anonymous_broker().await;

    let (sub_client, mut el_sub) = connect(connect_options(port, "wsi-sub"))
        .await
        .expect("connect sub");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub("will/si"))
        .await
        .expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let will = Will {
        topic: Topic::try_new("will/si").expect("valid"),
        payload: Payload::from(b"session-expiry-will".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        will_delay_interval: Some(10), // 10s delay...
        ..Will::default()
    };
    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("wsi-sender").expect("id"),
            will: Some(will),
            session_expiry_interval: Some(2), // ...but session expires at 2s
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sender, mut el) = connect(opts).await.expect("connect sender");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el); // abrupt disconnect — broker starts both timers

    // Will must arrive within ~4s (session expires at t≈2, firing the will early).
    // It must NOT arrive within 100ms (need at least the session expiry time).
    let too_early = tokio::time::timeout(Duration::from_millis(100), el_sub.poll()).await;
    assert!(
        too_early.is_err(),
        "will must not fire immediately, got: {too_early:?}"
    );

    let ev = tokio::time::timeout(Duration::from_secs(4), el_sub.poll())
        .await
        .expect("will within 4s (session_expiry=2s fires it early)")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected will Message, got {ev:?}"
    );
}

/// Reconnecting before will_delay_interval cancels the will. [MQTT-3.1.3-9]
#[tokio::test]
async fn will_cancelled_on_reconnect_before_delay() {
    let (_c, port) = anonymous_broker().await;

    let (sub_client, mut el_sub) = connect(connect_options(port, "wc-sub"))
        .await
        .expect("connect sub");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub("will/cancel"))
        .await
        .expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let will = Will {
        topic: Topic::try_new("will/cancel").expect("valid"),
        payload: Payload::from(b"should-not-appear".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        will_delay_interval: Some(5),
        ..Will::default()
    };
    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("wc-sender").expect("id"),
            will: Some(will),
            session_expiry_interval: Some(60),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sender, mut el) = connect(opts).await.expect("connect sender");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el); // abrupt disconnect — 5s will delay starts

    // Reconnect with the same client_id within 1s — cancels the pending will.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let reconnect_opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: false, // resume session to cancel will
            client_identifier: Utf8String::try_from("wc-sender").expect("id"),
            session_expiry_interval: Some(60),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sender2, mut el2) = connect(reconnect_opts).await.expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    // Wait past the original 5s delay — will must NOT fire.
    // Poll el_sub while waiting (so the subscriber can receive if something
    // arrives).
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let got = tokio::time::timeout(Duration::from_millis(100), el_sub.poll()).await;
        if let Ok(Ok(ev)) = got {
            panic!("will must not fire after reconnect, got {ev:?}");
        }
    }
}
