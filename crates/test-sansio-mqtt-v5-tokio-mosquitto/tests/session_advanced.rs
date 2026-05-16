use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Three QoS 1 messages queued while subscriber is offline are all delivered
/// on session resume.
#[tokio::test]
async fn multiple_inflight_qos1_all_delivered_after_reconnect() {
    let (_c, port) = anonymous_broker().await;

    let (sub1, mut el1) = connect(persistent_connect_options(port, "sa-mq1-sub"))
        .await
        .expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_qos1("sa/mq1")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    // Publisher sends 3 messages while subscriber is offline
    let (pub_c, mut el_pub) = connect(connect_options(port, "sa-mq1-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    for i in 0u8..3 {
        pub_c
            .publish(msg("sa/mq1", &[i], Qos::AtLeastOnce))
            .await
            .expect("publish");
        let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await;
        // drain puback
    }

    // Reconnect with session resume
    let (_sub2, mut el2) = connect(resume_connect_options(port, "sa-mq1-sub"))
        .await
        .expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    // All 3 must arrive
    for i in 0..3 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el2.poll())
            .await
            .unwrap_or_else(|_| panic!("message {i} within 5s"))
            .expect("event");
        assert!(
            matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
            "expected queued QoS1 message {i}, got {ev:?}"
        );
    }
}

/// Two QoS 2 messages queued for an offline subscriber are both delivered on
/// session resume.
#[tokio::test]
async fn multiple_inflight_qos2_all_delivered_after_reconnect() {
    let (_c, port) = anonymous_broker().await;

    // Subscribe at QoS 2
    let (sub1, mut el1) = connect(persistent_connect_options(port, "sa-mq2-sub"))
        .await
        .expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_with_options(
        "sa/mq2",
        Qos::ExactlyOnce,
        false,
        false,
        RetainHandling::SendRetained,
        None,
    ))
    .await
    .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    // Publisher sends 2 QoS 2 messages
    let (pub_c, mut el_pub) = connect(connect_options(port, "sa-mq2-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    for i in 0u8..2 {
        pub_c
            .publish(msg("sa/mq2", &[i], Qos::ExactlyOnce))
            .await
            .expect("publish");
        let ev = tokio::time::timeout(Duration::from_secs(5), el_pub.poll())
            .await
            .expect("pubcomp within 5s")
            .expect("event");
        assert!(
            matches!(ev, Event::PublishCompleted(_, _)),
            "expected PublishCompleted, got {ev:?}"
        );
    }

    // Reconnect subscriber
    let (_sub2, mut el2) = connect(resume_connect_options(port, "sa-mq2-sub"))
        .await
        .expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    for i in 0..2 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el2.poll())
            .await
            .unwrap_or_else(|_| panic!("qos2 message {i} within 5s"))
            .expect("event");
        assert!(
            matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
            "expected queued QoS2 message {i}, got {ev:?}"
        );
    }
}

/// Queued inbound messages arrive in the same poll loop after the Connected
/// event.
#[tokio::test]
async fn queued_inbound_messages_arrive_after_connack() {
    let (_c, port) = anonymous_broker().await;

    let (sub1, mut el1) = connect(persistent_connect_options(port, "sa-qi-sub"))
        .await
        .expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_qos1("sa/qi")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "sa-qi-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    for i in 0u8..3 {
        pub_c
            .publish(msg("sa/qi", &[i], Qos::AtLeastOnce))
            .await
            .expect("publish");
        let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await;
    }

    let (_sub2, mut el2) = connect(resume_connect_options(port, "sa-qi-sub"))
        .await
        .expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    // All 3 queued messages must be deliverable by polling without further action
    for i in 0..3 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el2.poll())
            .await
            .unwrap_or_else(|_| panic!("queued msg {i} within 5s"))
            .expect("event");
        assert!(
            matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
            "expected queued message {i}, got {ev:?}"
        );
    }
}

/// When a second client connects with the same client_id, Mosquitto sends
/// DISCONNECT with reason code SessionTakenOver to the first connection.
#[tokio::test]
async fn session_takeover_disconnects_old_connection() {
    let (_c, port) = anonymous_broker().await;

    let (_client1, mut el1) = connect(connect_options(port, "sa-takeover"))
        .await
        .expect("connect first");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));

    // Second connection with the same client_id
    let (_client2, mut el2) = connect(connect_options(port, "sa-takeover"))
        .await
        .expect("connect second");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    // First event loop must receive a disconnect with SessionTakenOver
    let result = tokio::time::timeout(Duration::from_secs(3), el1.poll())
        .await
        .expect("disconnect within 3s");
    match result {
        Ok(Event::Disconnected(_)) | Err(_) => {}
        Ok(ev) => panic!("expected Disconnected or error on session takeover, got {ev:?}"),
    }
}

/// After session_expiry elapses, reconnecting with clean_start=false results
/// in no queued messages (session was dropped).
#[tokio::test]
async fn session_expiry_drops_queued_messages() {
    let (_c, port) = anonymous_broker().await;

    // session_expiry_interval=0 means the broker deletes the session synchronously
    // when it processes the DISCONNECT — no periodic-timer race.
    let opts_short = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("sa-expiry").expect("id"),
            session_expiry_interval: Some(0),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (sub1, mut el1) = connect(opts_short).await.expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_qos1("sa/expiry"))
        .await
        .expect("subscribe");
    // Flush SUBSCRIBE to broker before disconnecting.
    let _ = tokio::time::timeout(Duration::from_millis(500), el1.poll()).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;
    // Give the broker a moment to finish processing the DISCONNECT and delete
    // the session before the publisher connects.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Publisher sends a message — session is gone, so the broker has no
    // subscription to queue it against.
    let (pub_c, mut el_pub) = connect(connect_options(port, "sa-expiry-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg("sa/expiry", b"queued", Qos::AtLeastOnce))
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await;

    // Reconnect without clean_start — session was dropped on disconnect, no queued
    // messages
    let opts_resume = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: false,
            client_identifier: Utf8String::try_from("sa-expiry").expect("id"),
            session_expiry_interval: Some(300),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sub2, mut el2) = connect(opts_resume).await.expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    let result = tokio::time::timeout(Duration::from_millis(500), el2.poll()).await;
    assert!(
        result.is_err(),
        "expired session must not deliver queued messages, got: {result:?}"
    );
}
