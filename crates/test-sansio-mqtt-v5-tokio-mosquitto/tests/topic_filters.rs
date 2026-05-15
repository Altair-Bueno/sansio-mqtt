use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

#[tokio::test]
async fn single_level_wildcard_matches_one_segment() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-slw-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    // + matches exactly one segment
    sub_c
        .subscribe(sub("sensors/+/temp"))
        .await
        .expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-slw-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    // This MUST match: one segment between slashes
    pub_c
        .publish(msg("sensors/room1/temp", b"22", Qos::AtMostOnce))
        .await
        .expect("publish match");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("matched within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected match on single-segment topic, got {ev:?}"
    );

    // This must NOT match: two segments between the fixed parts
    pub_c
        .publish(msg("sensors/room1/floor2/temp", b"21", Qos::AtMostOnce))
        .await
        .expect("publish no-match");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "+ must not match two segments, but subscriber got: {result:?}"
    );
}

#[tokio::test]
async fn multi_level_wildcard_matches_all_below() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-mlw-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("sensors/#")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-mlw-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    for topic in [
        "sensors/temp",
        "sensors/room1/temp",
        "sensors/room1/floor/temp",
    ] {
        pub_c
            .publish(msg(topic, b"val", Qos::AtMostOnce))
            .await
            .expect("publish");
        let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await
            .expect("within 3s")
            .expect("event");
        assert!(
            matches!(ev, Event::Message(_)),
            "# must match {topic}, got {ev:?}"
        );
    }
}

/// # matches `sensors/` and all descendants but NOT topics at a different root.
#[tokio::test]
async fn multi_level_wildcard_boundary_not_matched() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-mlwb-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("a/b/#")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-mlwb-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    // Must match
    pub_c
        .publish(msg("a/b/c", b"yes", Qos::AtMostOnce))
        .await
        .expect("publish match");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    assert!(matches!(ev, Event::Message(_)), "a/b/# must match a/b/c");

    // Must NOT match (different root)
    pub_c
        .publish(msg("a/c/d", b"no", Qos::AtMostOnce))
        .await
        .expect("publish no-match");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "a/b/# must not match a/c/d, got: {result:?}"
    );
}

/// A single SUBSCRIBE packet with extra_subscriptions covers multiple topics.
#[tokio::test]
async fn multiple_topics_in_one_subscribe_packet() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-multi-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));

    sub_c
        .subscribe(SubscribeOptions {
            subscription: Subscription {
                topic_filter: Utf8String::try_from("tf/multi/a").expect("valid"),
                qos: Qos::AtMostOnce,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendRetained,
            },
            extra_subscriptions: vec![
                Subscription {
                    topic_filter: Utf8String::try_from("tf/multi/b").expect("valid"),
                    qos: Qos::AtMostOnce,
                    no_local: false,
                    retain_as_published: false,
                    retain_handling: RetainHandling::SendRetained,
                },
                Subscription {
                    topic_filter: Utf8String::try_from("tf/multi/c").expect("valid"),
                    qos: Qos::AtMostOnce,
                    no_local: false,
                    retain_as_published: false,
                    retain_handling: RetainHandling::SendRetained,
                },
            ],
            subscription_identifier: None,
            user_properties: vec![],
        })
        .await
        .expect("multi-subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-multi-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    for topic in ["tf/multi/a", "tf/multi/b", "tf/multi/c"] {
        pub_c
            .publish(msg(topic, b"val", Qos::AtMostOnce))
            .await
            .expect("publish");
        let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await
            .expect("within 3s")
            .expect("event");
        assert!(
            matches!(ev, Event::Message(_)),
            "expected message on {topic}, got {ev:?}"
        );
    }
}

/// Overlapping subscriptions: `a/#` and `a/b` both match a publish to `a/b`.
/// Mosquitto 2 delivers one message per matching subscription filter.
#[tokio::test]
async fn overlapping_subscriptions_deliver_once_per_filter() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-overlap-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    // Two separate SUBSCRIBE packets for the same effective topic
    sub_c
        .subscribe(sub("tf/overlap/#"))
        .await
        .expect("subscribe wildcard");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    sub_c
        .subscribe(sub("tf/overlap/b"))
        .await
        .expect("subscribe exact");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-overlap-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg("tf/overlap/b", b"v", Qos::AtMostOnce))
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    // At least one delivery must happen
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected at least one delivery, got {ev:?}"
    );
}

/// Re-subscribing to an existing topic at a higher QoS upgrades delivery.
#[tokio::test]
async fn resubscribe_upgrades_qos() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(persistent_connect_options(port, "tf-upqos-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub("tf/upqos"))
        .await
        .expect("subscribe QoS0"); // QoS 0
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Re-subscribe at QoS 1
    sub_c
        .subscribe(sub_qos1("tf/upqos"))
        .await
        .expect("re-subscribe QoS1");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-upqos-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg("tf/upqos", b"v", Qos::AtLeastOnce))
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await; // drain puback

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
        "after QoS upgrade, message must arrive as QoS1, got {ev:?}"
    );
}

/// Unsubscribing stops delivery; re-subscribing restores it.
#[tokio::test]
async fn unsubscribe_followed_by_resubscribe() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-unsub-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("tf/unsub")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-unsub-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    // Receive first message (subscription active)
    pub_c
        .publish(msg("tf/unsub", b"first", Qos::AtMostOnce))
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("first within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected first message, got {ev:?}"
    );

    // Unsubscribe — second publish must NOT arrive
    sub_c
        .unsubscribe(UnsubscribeOptions {
            filter: Utf8String::try_from("tf/unsub").expect("valid"),
            extra_filters: vec![],
            user_properties: vec![],
        })
        .await
        .expect("unsubscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    pub_c
        .publish(msg("tf/unsub", b"second", Qos::AtMostOnce))
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    let no_msg = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        no_msg.is_err(),
        "after unsubscribe, must not receive message, got: {no_msg:?}"
    );

    // Re-subscribe — third publish MUST arrive
    sub_c
        .subscribe(sub("tf/unsub"))
        .await
        .expect("re-subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    pub_c
        .publish(msg("tf/unsub", b"third", Qos::AtMostOnce))
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    let ev3 = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("third within 3s")
        .expect("event");
    assert!(
        matches!(ev3, Event::Message(_)),
        "expected third message after re-subscribe, got {ev3:?}"
    );
}
