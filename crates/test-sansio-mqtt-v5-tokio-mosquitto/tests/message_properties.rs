use std::time::Duration;

use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// User properties are passed through the broker unchanged.
#[tokio::test]
async fn user_properties_preserved_end_to_end() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "up-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub("mp/user-props"))
        .await
        .expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "up-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    let k1 = Utf8String::try_from("env").expect("valid");
    let v1 = Utf8String::try_from("test").expect("valid");
    let k2 = Utf8String::try_from("version").expect("valid");
    let v2 = Utf8String::try_from("1.0").expect("valid");
    let k3 = Utf8String::try_from("region").expect("valid");
    let v3 = Utf8String::try_from("eu-west").expect("valid");

    pub_c
        .publish(ClientMessage {
            topic: Topic::try_new("mp/user-props".as_bytes().to_vec()).expect("valid"),
            payload: Payload::from(b"payload".as_slice()),
            qos: Qos::AtMostOnce,
            user_properties: vec![
                (k1.clone(), v1.clone()),
                (k2.clone(), v2.clone()),
                (k3.clone(), v3.clone()),
            ],
            ..ClientMessage::default()
        })
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    let m = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(m.user_properties.contains(&(k1, v1)), "prop 'env' missing");
    assert!(
        m.user_properties.contains(&(k2, v2)),
        "prop 'version' missing"
    );
    assert!(
        m.user_properties.contains(&(k3, v3)),
        "prop 'region' missing"
    );
}

/// A message with message_expiry_interval=1s is NOT delivered to a subscriber
/// that reconnects after the expiry elapses.
#[tokio::test]
async fn message_expiry_interval_drops_stale_message() {
    let (_c, port) = anonymous_broker().await;

    // Persistent subscriber goes offline
    let (sub1, mut el1) = connect(persistent_connect_options(port, "mei-sub"))
        .await
        .expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_qos1("mp/expiry"))
        .await
        .expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el1.poll()).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "mei-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    pub_c
        .publish(ClientMessage {
            topic: Topic::try_new("mp/expiry".as_bytes().to_vec()).expect("valid"),
            payload: Payload::from(b"stale".as_slice()),
            qos: Qos::AtLeastOnce,
            message_expiry_interval: Some(Duration::from_secs(1)),
            ..ClientMessage::default()
        })
        .await
        .expect("publish expiring message");
    let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await; // drain puback

    // Wait for message to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    let (_sub2, mut el2) = connect(resume_connect_options(port, "mei-sub"))
        .await
        .expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    let result = tokio::time::timeout(Duration::from_millis(500), el2.poll()).await;
    assert!(
        result.is_err(),
        "expired message must not be delivered, got: {result:?}"
    );
}

/// response_topic and correlation_data survive the broker round-trip.
#[tokio::test]
async fn response_topic_and_correlation_data_round_trip() {
    let (_c, port) = anonymous_broker().await;

    // Responder subscribes to the request topic
    let (resp_c, mut el_resp) = connect(connect_options(port, "rr-resp"))
        .await
        .expect("connect");
    assert!(matches!(
        el_resp.poll().await.expect("poll"),
        Event::Connected
    ));
    resp_c
        .subscribe(sub("mp/request"))
        .await
        .expect("subscribe request");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_resp.poll()).await;

    // Requester publishes with response_topic and correlation_data
    let (req_c, mut el_req) = connect(connect_options(port, "rr-req"))
        .await
        .expect("connect");
    assert!(matches!(
        el_req.poll().await.expect("poll"),
        Event::Connected
    ));
    req_c
        .subscribe(sub("mp/response/rr-req"))
        .await
        .expect("subscribe response");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_req.poll()).await;

    req_c
        .publish(ClientMessage {
            topic: Topic::try_new("mp/request".as_bytes().to_vec()).expect("valid"),
            payload: Payload::from(b"compute-42".as_slice()),
            qos: Qos::AtMostOnce,
            response_topic: Some(
                Topic::try_new("mp/response/rr-req".as_bytes().to_vec()).expect("valid"),
            ),
            correlation_data: Some(BinaryData::new(b"corr-abc".as_slice())),
            ..ClientMessage::default()
        })
        .await
        .expect("publish request");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_req.poll()).await;

    // Responder receives request, checks response_topic and correlation_data
    let req_ev = tokio::time::timeout(Duration::from_secs(3), el_resp.poll())
        .await
        .expect("request within 3s")
        .expect("event");
    let req_msg = match req_ev {
        Event::Message(m) => m,
        other => panic!("expected request Message, got {other:?}"),
    };
    let rt = req_msg
        .response_topic
        .expect("response_topic must be present");
    let cd = req_msg
        .correlation_data
        .expect("correlation_data must be present");
    assert_eq!(&cd[..], b"corr-abc", "correlation_data mismatch");

    // Responder replies on the response_topic
    resp_c
        .publish(ClientMessage {
            topic: rt,
            payload: Payload::from(b"answer-42".as_slice()),
            qos: Qos::AtMostOnce,
            correlation_data: Some(cd),
            ..ClientMessage::default()
        })
        .await
        .expect("publish response");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_resp.poll()).await;

    let resp_ev = tokio::time::timeout(Duration::from_secs(3), el_req.poll())
        .await
        .expect("response within 3s")
        .expect("event");
    let resp_msg = match resp_ev {
        Event::Message(m) => m,
        other => panic!("expected response Message, got {other:?}"),
    };
    let resp_cd = resp_msg
        .correlation_data
        .expect("response must carry correlation_data");
    assert_eq!(
        &resp_cd[..],
        b"corr-abc",
        "response correlation_data mismatch"
    );
}

/// content_type is preserved end-to-end.
#[tokio::test]
async fn content_type_preserved() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ct-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("mp/ct")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ct-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(ClientMessage {
            topic: Topic::try_new("mp/ct".as_bytes().to_vec()).expect("valid"),
            payload: Payload::from(b"{}".as_slice()),
            qos: Qos::AtMostOnce,
            content_type: Some(Utf8String::try_from("application/json").expect("valid")),
            ..ClientMessage::default()
        })
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    let m = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    let ct = m.content_type.expect("content_type must be present");
    assert_eq!(
        <Utf8String as AsRef<str>>::as_ref(&ct),
        "application/json",
        "content_type mismatch"
    );
}

/// payload_format_indicator is preserved end-to-end.
#[tokio::test]
async fn payload_format_indicator_preserved() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "pfi-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("mp/pfi")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "pfi-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));
    pub_c
        .publish(ClientMessage {
            topic: Topic::try_new("mp/pfi".as_bytes().to_vec()).expect("valid"),
            payload: Payload::from(b"hello world".as_slice()),
            qos: Qos::AtMostOnce,
            payload_format_indicator: Some(FormatIndicator::Utf8),
            ..ClientMessage::default()
        })
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    let m = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert_eq!(
        m.payload_format_indicator,
        Some(FormatIndicator::Utf8),
        "payload_format_indicator must be preserved"
    );
}

/// Duplicate user property keys are all preserved in order.
#[tokio::test]
async fn multiple_user_properties_duplicate_keys() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "dupk-sub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("mp/dupkeys")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "dupk-pub"))
        .await
        .expect("connect");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    let key = || Utf8String::try_from("tag").expect("valid");
    let v1 = Utf8String::try_from("alpha").expect("valid");
    let v2 = Utf8String::try_from("beta").expect("valid");
    pub_c
        .publish(ClientMessage {
            topic: Topic::try_new("mp/dupkeys".as_bytes().to_vec()).expect("valid"),
            payload: Payload::from(b"v".as_slice()),
            qos: Qos::AtMostOnce,
            user_properties: vec![(key(), v1.clone()), (key(), v2.clone())],
            ..ClientMessage::default()
        })
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("within 3s")
        .expect("event");
    let m = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(
        m.user_properties.contains(&(key(), v1)),
        "duplicate key 'tag'='alpha' missing"
    );
    assert!(
        m.user_properties.contains(&(key(), v2)),
        "duplicate key 'tag'='beta' missing"
    );
}
