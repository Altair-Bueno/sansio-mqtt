use std::time::Duration;

use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Publisher sends a PUBLISH with Topic Alias=1 and full topic name.
/// Mosquitto must accept the alias registration and forward the message.
#[tokio::test]
async fn publisher_send_with_topic_alias() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ta-pub-sub"))
        .await
        .expect("connect sub");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("ta/pub")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ta-pub-pub"))
        .await
        .expect("connect pub");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    let message = ClientMessage {
        topic: Topic::try_new(b"ta/pub".to_vec()).expect("valid topic"),
        payload: Payload::from(b"aliased".as_slice()),
        qos: Qos::AtMostOnce,
        topic_alias: core::num::NonZero::new(1),
        ..ClientMessage::default()
    };
    pub_c.publish(message).await.expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await
        .expect("message within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "expected Message after topic-alias publish, got {ev:?}"
    );
}

/// When the subscriber advertises topic_alias_maximum=5, Mosquitto may use
/// topic aliases for outbound PUBLISH packets. Our library must resolve
/// alias→topic so BrokerMessage.topic is always correct.
#[tokio::test]
async fn subscriber_resolves_broker_topic_aliases() {
    let (_c, port) = anonymous_broker().await;

    let sub_opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("ta-sub-sub").expect("id"),
            topic_alias_maximum: Some(5),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (sub_c, mut el_sub) = connect(sub_opts).await.expect("connect sub");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("ta/sub")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ta-sub-pub"))
        .await
        .expect("connect pub");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    for i in 0u8..5 {
        pub_c
            .publish(msg("ta/sub", &[i], Qos::AtMostOnce))
            .await
            .expect("publish");
        let _ = tokio::time::timeout(Duration::from_millis(100), el_pub.poll()).await;
    }

    for i in 0..5 {
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await
            .unwrap_or_else(|_| panic!("message {i} within 3s"))
            .expect("event");
        let broker_msg = match ev {
            Event::Message(m) => m,
            other => panic!("expected Message {i}, got {other:?}"),
        };
        // Verify topic was correctly resolved from any broker-side alias.
        // Topic derefs to Utf8String; as_bytes() returns the raw UTF-8 bytes.
        assert_eq!(
            broker_msg.topic.as_bytes(),
            b"ta/sub",
            "message {i} must have resolved topic 'ta/sub'"
        );
    }
}
