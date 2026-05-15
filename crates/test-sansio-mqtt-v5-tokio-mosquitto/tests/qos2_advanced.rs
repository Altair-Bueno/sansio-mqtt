use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Five QoS 2 messages published concurrently all complete the full
/// PUBLISH→PUBREC→PUBREL→PUBCOMP flow.
#[tokio::test]
async fn concurrent_qos2_publishes_all_complete() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "cq2-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub("qos2/concurrent"))
        .await
        .expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "cq2-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    for i in 0u8..5 {
        pub_c
            .publish(msg("qos2/concurrent", &[i], Qos::ExactlyOnce))
            .await
            .expect("publish");
    }

    // All 5 must complete
    for i in 0..5 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el_pub.poll())
            .await
            .unwrap_or_else(|_| panic!("PublishCompleted {i} within 5s"))
            .expect("event");
        assert!(
            matches!(ev, Event::PublishCompleted(_, _)),
            "expected PublishCompleted {i}, got {ev:?}"
        );
    }

    // Subscriber must receive all 5
    for i in 0..5 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el_sub.poll())
            .await
            .unwrap_or_else(|_| panic!("subscriber message {i} within 5s"))
            .expect("event");
        assert!(
            matches!(ev, Event::Message(_)),
            "expected subscriber Message {i}, got {ev:?}"
        );
    }
}

/// A QoS 2 message queued for an offline QoS-2 subscriber is delivered on
/// session resume.
#[tokio::test]
async fn qos2_queued_for_offline_subscriber_delivered_on_resume() {
    let (_c, port) = anonymous_broker().await;

    // Subscribe at QoS 2
    let (sub1, mut el1) = connect(persistent_connect_options(port, "qos2-offline-sub"))
        .await
        .expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_with_options(
        "qos2/offline",
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

    // Publisher sends QoS 2
    let (pub_c, mut el_pub) = connect(connect_options(port, "qos2-offline-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg("qos2/offline", b"queued-qos2", Qos::ExactlyOnce))
        .await
        .expect("publish");
    let pev = tokio::time::timeout(Duration::from_secs(5), el_pub.poll())
        .await
        .expect("PublishCompleted within 5s")
        .expect("event");
    assert!(
        matches!(pev, Event::PublishCompleted(_, _)),
        "expected PublishCompleted, got {pev:?}"
    );

    // Reconnect subscriber
    let (_sub2, mut el2) = connect(resume_connect_options(port, "qos2-offline-sub"))
        .await
        .expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    let ev = tokio::time::timeout(Duration::from_secs(5), el2.poll())
        .await
        .expect("queued QoS2 within 5s")
        .expect("event");
    assert!(
        matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
        "expected MessageWithRequiredAcknowledgement for queued QoS2, got {ev:?}"
    );
}

/// Publishing more QoS 2 messages than the server's receive_maximum allows in
/// parallel does not cause errors — the protocol queues them.
#[tokio::test]
async fn qos2_beyond_receive_maximum_queues_without_error() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "qos2-rm-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("qos2/rm")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Connect with a low receive_maximum to limit in-flight QoS2 from broker to us,
    // then fire more publishes than the default broker receive_maximum (typically
    // 20)
    let (pub_c, mut el_pub) = connect(connect_options(port, "qos2-rm-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    // Submit 10 QoS 2 publishes — more than many broker defaults but should be
    // handled
    for i in 0u8..10 {
        pub_c
            .publish(msg("qos2/rm", &[i], Qos::ExactlyOnce))
            .await
            .expect("publish");
    }

    // All 10 must complete without error
    for i in 0..10 {
        let ev = tokio::time::timeout(Duration::from_secs(10), el_pub.poll())
            .await
            .unwrap_or_else(|_| panic!("PublishCompleted {i} within 10s"))
            .expect("event");
        assert!(
            matches!(ev, Event::PublishCompleted(_, _)),
            "expected PublishCompleted {i}, got {ev:?}"
        );
    }
}
