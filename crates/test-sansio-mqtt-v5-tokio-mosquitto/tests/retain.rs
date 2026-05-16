use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

#[tokio::test]
async fn retained_message_delivered_to_new_subscriber() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ret-basic-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg_retain(
            "test/retain/basic",
            b"retained-value",
            Qos::AtMostOnce,
        ))
        .await
        .expect("publish retained");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ret-basic-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub("test/retain/basic"))
        .await
        .expect("subscribe");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("retained within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected retained Message, got {ev:?}"
    );
}

#[tokio::test]
async fn clear_retained_message() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ret-clear-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg_retain("test/retain/clear", b"first", Qos::AtMostOnce))
        .await
        .expect("publish retained");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    pub_c
        .publish(msg_retain("test/retain/clear", b"", Qos::AtMostOnce))
        .await
        .expect("clear retained");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ret-clear-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub("test/retain/clear"))
        .await
        .expect("subscribe");

    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "cleared retained must not be delivered, got: {result:?}"
    );
}

#[tokio::test]
async fn retained_message_is_latest_value() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ret-latest-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg_retain("test/retain/latest", b"first", Qos::AtMostOnce))
        .await
        .expect("publish first");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    pub_c
        .publish(msg_retain("test/retain/latest", b"second", Qos::AtMostOnce))
        .await
        .expect("publish second");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ret-latest-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub("test/retain/latest"))
        .await
        .expect("subscribe");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("retained within 3s")
        .expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert_eq!(
        &broker_msg.payload.as_ref()[..],
        b"second",
        "must receive latest retained value"
    );

    pub_c
        .publish(msg_retain("test/retain/latest", b"", Qos::AtMostOnce))
        .await
        .expect("clear");
}

#[tokio::test]
async fn retain_handling_send_on_subscribe() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rh-sos-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg_retain("test/retain/sos", b"value", Qos::AtMostOnce))
        .await
        .expect("publish retained");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "rh-sos-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));

    sub_c
        .subscribe(sub_with_options(
            "test/retain/sos",
            Qos::AtMostOnce,
            false,
            false,
            RetainHandling::SendRetained,
            None,
        ))
        .await
        .expect("subscribe 1");
    let ev1 = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("retained on first sub")
        .expect("event");
    assert!(
        matches!(ev1, Event::Message(_)),
        "expected retained on 1st sub, got {ev1:?}"
    );

    sub_c
        .unsubscribe(UnsubscribeOptions {
            filter: Utf8String::try_from("test/retain/sos").expect("valid"),
            extra_filters: vec![],
            user_properties: vec![],
        })
        .await
        .expect("unsubscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    sub_c
        .subscribe(sub_with_options(
            "test/retain/sos",
            Qos::AtMostOnce,
            false,
            false,
            RetainHandling::SendRetained,
            None,
        ))
        .await
        .expect("subscribe 2");
    let ev2 = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("retained on re-sub")
        .expect("event");
    assert!(
        matches!(ev2, Event::Message(_)),
        "expected retained on re-sub, got {ev2:?}"
    );

    pub_c
        .publish(msg_retain("test/retain/sos", b"", Qos::AtMostOnce))
        .await
        .expect("clear");
}

#[tokio::test]
async fn retain_handling_send_only_on_new_subscribe() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rh-new-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg_retain("test/retain/new", b"value", Qos::AtMostOnce))
        .await
        .expect("publish retained");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "rh-new-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));

    sub_c
        .subscribe(sub_with_options(
            "test/retain/new",
            Qos::AtMostOnce,
            false,
            false,
            RetainHandling::SendRetainedIfSubscriptionDoesNotExist,
            None,
        ))
        .await
        .expect("subscribe 1");
    let ev1 = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("retained on first sub")
        .expect("event");
    assert!(
        matches!(ev1, Event::Message(_)),
        "expected retained on new sub, got {ev1:?}"
    );

    sub_c
        .subscribe(sub_with_options(
            "test/retain/new",
            Qos::AtMostOnce,
            false,
            false,
            RetainHandling::SendRetainedIfSubscriptionDoesNotExist,
            None,
        ))
        .await
        .expect("subscribe 2");
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "retained must not be sent on re-sub, got: {result:?}"
    );

    pub_c
        .publish(msg_retain("test/retain/new", b"", Qos::AtMostOnce))
        .await
        .expect("clear");
}

#[tokio::test]
async fn retain_handling_do_not_send() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rh-dns-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg_retain("test/retain/dns", b"value", Qos::AtMostOnce))
        .await
        .expect("publish retained");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "rh-dns-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub_with_options(
            "test/retain/dns",
            Qos::AtMostOnce,
            false,
            false,
            RetainHandling::DoNotSend,
            None,
        ))
        .await
        .expect("subscribe");

    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "DoNotSend must suppress retained on subscribe, got: {result:?}"
    );

    pub_c
        .publish(msg("test/retain/dns", b"live", Qos::AtMostOnce))
        .await
        .expect("publish live");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("live message within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected live message, got {ev:?}"
    );

    pub_c
        .publish(msg_retain("test/retain/dns", b"", Qos::AtMostOnce))
        .await
        .expect("clear");
}

#[tokio::test]
async fn retained_message_delivered_on_wildcard_subscribe() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ret-wild-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(msg_retain(
            "test/retain/wild/foo",
            b"wildcard-retained",
            Qos::AtMostOnce,
        ))
        .await
        .expect("publish retained");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ret-wild-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub("test/retain/wild/#"))
        .await
        .expect("subscribe");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("retained within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected retained Message on wildcard subscribe, got {ev:?}"
    );

    // Clean up
    pub_c
        .publish(msg_retain("test/retain/wild/foo", b"", Qos::AtMostOnce))
        .await
        .expect("clear");
}

#[tokio::test]
async fn retain_as_published_preserves_retain_flag() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rap-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    let (sub_c, mut el_sub) = connect(connect_options(port, "rap-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub_with_options(
            "test/retain/rap",
            Qos::AtMostOnce,
            false,
            true,
            RetainHandling::SendRetained,
            None,
        ))
        .await
        .expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    pub_c
        .publish(msg_retain("test/retain/rap", b"value", Qos::AtMostOnce))
        .await
        .expect("publish retained");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("message within 3s")
        .expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(
        broker_msg.retain,
        "retain_as_published=true must preserve retain=true in forwarded message"
    );

    pub_c
        .publish(msg_retain("test/retain/rap", b"", Qos::AtMostOnce))
        .await
        .expect("clear");
}
