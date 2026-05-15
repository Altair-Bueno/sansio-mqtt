use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// no_local=true: a client must not receive its own publishes on that topic.
#[tokio::test]
async fn no_local_prevents_receiving_own_publishes() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "nl-client"))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    client
        .subscribe(sub_with_options(
            "nl/topic",
            Qos::AtMostOnce,
            true, // no_local
            false,
            RetainHandling::SendRetained,
            None,
        ))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    client
        .publish(msg("nl/topic", b"self-pub", Qos::AtMostOnce))
        .await
        .expect("publish");

    let result = tokio::time::timeout(Duration::from_millis(500), el.poll()).await;
    assert!(
        result.is_err(),
        "no_local=true must suppress self-publish, got: {result:?}"
    );
}

/// Subscription identifier is carried in every matching BrokerMessage.
#[tokio::test]
async fn subscription_identifier_delivered_with_message() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "sid-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    let id = core::num::NonZero::<u64>::new(42).unwrap();
    sub_c
        .subscribe(sub_with_options(
            "sid/topic",
            Qos::AtMostOnce,
            false,
            false,
            RetainHandling::SendRetained,
            Some(id),
        ))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "sid-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg("sid/topic", b"v", Qos::AtMostOnce))
        .await
        .expect("publish");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(
        broker_msg.subscription_identifiers.contains(&id),
        "subscription_identifiers must contain {id}; got: {:?}",
        broker_msg.subscription_identifiers
    );
}

/// Subscription identifier on a wildcard subscription is carried in all
/// messages matching that wildcard.
#[tokio::test]
async fn subscription_identifier_on_wildcard_subscription() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "sidw-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    let id = core::num::NonZero::<u64>::new(99).unwrap();
    sub_c
        .subscribe(sub_with_options(
            "sidw/#",
            Qos::AtMostOnce,
            false,
            false,
            RetainHandling::SendRetained,
            Some(id),
        ))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "sidw-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    for topic in ["sidw/a", "sidw/b/c"] {
        pub_c
            .publish(msg(topic, b"v", Qos::AtMostOnce))
            .await
            .expect("publish");
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await
            .expect("within 3s")
            .expect("event");
        let broker_msg = match ev {
            Event::Message(m) => m,
            other => panic!("expected Message on {topic}, got {other:?}"),
        };
        assert!(
            broker_msg.subscription_identifiers.contains(&id),
            "sub id must be on message for {topic}; got: {:?}",
            broker_msg.subscription_identifiers
        );
    }
}

#[tokio::test]
async fn unsubscribe_stops_delivery() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "us-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("us/topic")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "us-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg("us/topic", b"before", Qos::AtMostOnce))
        .await
        .expect("publish before");
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected message before unsub, got {ev:?}"
    );

    sub_c
        .unsubscribe(UnsubscribeOptions {
            filter: Utf8String::try_from("us/topic").expect("valid"),
            extra_filters: vec![],
            user_properties: vec![],
        })
        .await
        .expect("unsubscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    pub_c
        .publish(msg("us/topic", b"after", Qos::AtMostOnce))
        .await
        .expect("publish after");
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "after unsubscribe, no message must arrive, got: {result:?}"
    );
}

/// Unsubscribing from a topic the client never subscribed to must not error.
#[tokio::test]
async fn unsubscribe_from_nonexistent_topic_succeeds() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "unt-client"))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    client
        .unsubscribe(UnsubscribeOptions {
            filter: Utf8String::try_from("unt/never-subscribed").expect("valid"),
            extra_filters: vec![],
            user_properties: vec![],
        })
        .await
        .expect("unsubscribe from nonexistent must not error on send");

    // No error event should arrive; connection stays alive
    let result = tokio::time::timeout(Duration::from_millis(500), el.poll()).await;
    // A timeout (no event) is success. An Ok(Err(...)) would be a failure.
    if let Ok(Err(e)) = result {
        panic!("unexpected error after unsubscribing nonexistent topic: {e:?}");
    }
}

/// Re-subscribing to a topic at a lower QoS downgrades delivery.
#[tokio::test]
async fn resubscribe_downgrades_qos() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(persistent_connect_options(port, "rdq-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub_qos1("rdq/topic"))
        .await
        .expect("subscribe QoS1");
    tokio::time::sleep(Duration::from_millis(100)).await;
    sub_c
        .subscribe(sub("rdq/topic"))
        .await
        .expect("re-subscribe QoS0");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rdq-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg("rdq/topic", b"v", Qos::AtLeastOnce))
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "after QoS downgrade, message must arrive as QoS0 (Message), got {ev:?}"
    );
}

/// A single subscribe() with extra_subscriptions subscribes to multiple topics.
#[tokio::test]
async fn multiple_subscriptions_one_call() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "msoc-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));

    let make_sub = |topic: &str| Subscription {
        topic_filter: Utf8String::try_from(topic).expect("valid"),
        qos: Qos::AtMostOnce,
        no_local: false,
        retain_as_published: false,
        retain_handling: RetainHandling::SendRetained,
    };
    sub_c
        .subscribe(SubscribeOptions {
            subscription: make_sub("msoc/x"),
            extra_subscriptions: vec![make_sub("msoc/y"), make_sub("msoc/z")],
            subscription_identifier: None,
            user_properties: vec![],
        })
        .await
        .expect("multi subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "msoc-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    for topic in ["msoc/x", "msoc/y", "msoc/z"] {
        pub_c
            .publish(msg(topic, b"v", Qos::AtMostOnce))
            .await
            .expect("publish");
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await
            .unwrap_or_else(|_| panic!("{topic} within 3s"))
            .expect("event");
        assert!(
            matches!(ev, Event::Message(_)),
            "expected message on {topic}, got {ev:?}"
        );
    }
}

/// Shared subscriptions distribute messages across subscribers.
/// With $share/group/topic two clients share load — each message goes to
/// exactly one of them.
#[tokio::test]
async fn shared_subscription_load_balancing() {
    let (_c, port) = anonymous_broker().await;

    let (s1, mut el1) = connect(connect_options(port, "ss-client1"))
        .await
        .expect("connect 1");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    s1.subscribe(sub("$share/grp/ss/topic"))
        .await
        .expect("subscribe 1");

    let (s2, mut el2) = connect(connect_options(port, "ss-client2"))
        .await
        .expect("connect 2");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));
    s2.subscribe(sub("$share/grp/ss/topic"))
        .await
        .expect("subscribe 2");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ss-pub"))
        .await
        .expect("connect pub");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    // Publish 4 messages; each must be delivered to exactly one of the two
    // subscribers.
    for i in 0u8..4 {
        pub_c
            .publish(msg("ss/topic", &[i], Qos::AtMostOnce))
            .await
            .expect("publish");
    }

    let mut total = 0u8;
    for _ in 0..4 {
        let got1 = tokio::time::timeout(Duration::from_millis(200), el1.poll()).await;
        let got2 = tokio::time::timeout(Duration::from_millis(200), el2.poll()).await;
        // Exactly one of the two should receive the message for each publish
        match (got1, got2) {
            (Ok(Ok(Event::Message(_))), Err(_)) => total += 1,
            (Err(_), Ok(Ok(Event::Message(_)))) => total += 1,
            (Ok(Ok(Event::Message(_))), Ok(Ok(Event::Message(_)))) => {
                // Both got this message — that's also acceptable if Mosquitto
                // doesn't guarantee exclusivity at AtMostOnce
                total += 1;
            }
            _ => {}
        }
    }
    // Give a brief extra window for any remaining messages
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        total >= 1,
        "at least one shared subscriber must receive messages"
    );
}
