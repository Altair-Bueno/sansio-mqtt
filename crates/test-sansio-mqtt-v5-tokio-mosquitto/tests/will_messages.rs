use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

fn make_will(topic: &'static str, payload: &'static [u8], qos: Qos) -> Will {
    Will {
        topic: Topic::try_new(topic).expect("valid topic"),
        payload: Payload::from(payload),
        qos,
        retain: false,
        ..Will::default()
    }
}

/// Will MUST NOT be published when the client sends DISCONNECT normally.
/// [MQTT-3.1.2-10]
#[tokio::test]
async fn will_not_sent_on_graceful_disconnect() {
    let (_c, port) = anonymous_broker().await;

    let (sub_client, mut el_sub) = connect(connect_options(port, "wng-sub"))
        .await
        .expect("connect sub");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub("will/ng"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let will = make_will("will/ng", b"gone", Qos::AtMostOnce);
    let (sender, mut el_sender) = connect(will_connect_options(port, "wng-sender", will))
        .await
        .expect("connect sender");
    assert!(matches!(
        el_sender.poll().await.expect("poll"),
        Event::Connected
    ));

    sender.disconnect().await.expect("graceful disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el_sender.poll()).await;

    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "will must not be published after graceful disconnect, got: {result:?}"
    );
}

#[tokio::test]
async fn will_sent_on_abrupt_disconnect_qos0() {
    let (_c, port) = anonymous_broker().await;

    let (sub_client, mut el_sub) = connect(connect_options(port, "waq0-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub("will/aq0"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    let will = make_will("will/aq0", b"gone-qos0", Qos::AtMostOnce);
    let (_sender, mut el) = connect(will_connect_options(port, "waq0-sender", will))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el); // abrupt — triggers will

    let ev = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("will within 3s")
        .expect("join")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected Message, got {ev:?}"
    );
}

#[tokio::test]
async fn will_sent_on_abrupt_disconnect_qos1() {
    let (_c, port) = anonymous_broker().await;

    let (sub_client, mut el_sub) = connect(persistent_connect_options(port, "waq1-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub_qos1("will/aq1"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    let will = make_will("will/aq1", b"gone-qos1", Qos::AtLeastOnce);
    let (_sender, mut el) = connect(will_connect_options(port, "waq1-sender", will))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el);

    let ev = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("will within 3s")
        .expect("join")
        .expect("event");
    assert!(
        matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
        "expected MessageWithRequiredAcknowledgement for QoS1 will, got {ev:?}"
    );
}

#[tokio::test]
async fn will_with_retain_flag() {
    let (_c, port) = anonymous_broker().await;

    let will = Will {
        topic: Topic::try_new("will/retained").expect("valid"),
        payload: Payload::from(b"retained-will".as_slice()),
        qos: Qos::AtMostOnce,
        retain: true,
        ..Will::default()
    };
    let (_sender, mut el) = connect(will_connect_options(port, "wr-sender", will))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el); // abrupt — publishes retained will

    tokio::time::sleep(Duration::from_secs(1)).await; // let broker store it

    // Late subscriber must receive the retained will
    let (sub_client, mut el_sub) = connect(connect_options(port, "wr-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub("will/retained"))
        .await
        .expect("subscribe");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("retained will within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected retained will, got {ev:?}"
    );

    // Clean up retained message
    let (cleanup_client, mut el_c) = connect(connect_options(port, "wr-cleanup"))
        .await
        .expect("connect");
    assert!(matches!(el_c.poll().await.expect("poll"), Event::Connected));
    cleanup_client
        .publish(msg_retain("will/retained", b"", Qos::AtMostOnce))
        .await
        .expect("clear");
    tokio::time::sleep(Duration::from_millis(150)).await;
}

/// will_delay_interval=2s: will must NOT arrive before the delay expires.
#[tokio::test]
async fn will_with_delay_interval() {
    let (_c, port) = anonymous_broker().await;

    let (sub_client, mut el_sub) = connect(connect_options(port, "wdi-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub("will/delayed"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let will = Will {
        topic: Topic::try_new("will/delayed").expect("valid"),
        payload: Payload::from(b"delayed".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        will_delay_interval: Some(2),
        ..Will::default()
    };
    // session_expiry must be > will_delay_interval so the will can fire
    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("wdi-sender").expect("id"),
            will: Some(will),
            session_expiry_interval: Some(10),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sender, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el);

    // Must NOT arrive within 500ms (delay not elapsed)
    let early = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        early.is_err(),
        "will must not arrive before delay: {early:?}"
    );

    // Must arrive within 3s total (2s delay + margin)
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("delayed will within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected delayed will, got {ev:?}"
    );
}

/// will_delay=2s + message_expiry=1s: will fires at t=2 but message expired at
/// t=3 (1s after publication). Offline subscriber reconnecting at t=4 gets
/// nothing.
#[tokio::test]
async fn will_with_expiry_interval() {
    let (_c, port) = anonymous_broker().await;

    // Persistent subscriber goes offline
    let (s, mut el_s) = connect(persistent_connect_options(port, "wei-sub"))
        .await
        .expect("connect");
    assert!(matches!(el_s.poll().await.expect("poll"), Event::Connected));
    s.subscribe(sub_qos1("will/expiry"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    s.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el_s.poll()).await;

    let will = Will {
        topic: Topic::try_new("will/expiry").expect("valid"),
        payload: Payload::from(b"should-expire".as_slice()),
        qos: Qos::AtLeastOnce,
        retain: false,
        will_delay_interval: Some(2),
        message_expiry_interval: Some(Duration::from_secs(1)),
        ..Will::default()
    };
    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("wei-sender").expect("id"),
            will: Some(will),
            session_expiry_interval: Some(10),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sender, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el); // t=0: trigger will

    // Wait: t=2 will fires, t=3 message expires, we check at t=4
    tokio::time::sleep(Duration::from_secs(4)).await;

    let (_s2, mut el_s2) = connect(resume_connect_options(port, "wei-sub"))
        .await
        .expect("reconnect");
    assert!(matches!(
        el_s2.poll().await.expect("poll"),
        Event::Connected
    ));

    let result = tokio::time::timeout(Duration::from_millis(500), el_s2.poll()).await;
    assert!(
        result.is_err(),
        "expired will must not be delivered, got: {result:?}"
    );
}

#[tokio::test]
async fn will_with_empty_payload() {
    let (_c, port) = anonymous_broker().await;

    let (sub_client, mut el_sub) = connect(connect_options(port, "wep-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub("will/empty"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    let will = Will {
        topic: Topic::try_new("will/empty").expect("valid"),
        payload: Payload::from(b"".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        ..Will::default()
    };
    let (_sender, mut el) = connect(will_connect_options(port, "wep-sender", will))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el);

    let ev = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("within 3s")
        .expect("join")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected Message for empty-payload will, got {ev:?}"
    );
}

#[tokio::test]
async fn will_with_user_properties() {
    let (_c, port) = anonymous_broker().await;

    let (sub_client, mut el_sub) = connect(connect_options(port, "wup-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_client
        .subscribe(sub("will/props"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    let key = Utf8String::try_from("reason").expect("valid");
    let val = Utf8String::try_from("crash").expect("valid");
    let will = Will {
        topic: Topic::try_new("will/props").expect("valid"),
        payload: Payload::from(b"props-will".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        user_properties: vec![(key, val)],
        ..Will::default()
    };
    let (_sender, mut el) = connect(will_connect_options(port, "wup-sender", will))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el);

    let ev = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("within 3s")
        .expect("join")
        .expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    let expected_key = Utf8String::try_from("reason").expect("valid");
    let expected_val = Utf8String::try_from("crash").expect("valid");
    assert!(
        broker_msg
            .user_properties
            .contains(&(expected_key, expected_val)),
        "user property must be preserved in will; got: {:?}",
        broker_msg.user_properties
    );
}
