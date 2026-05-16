use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

const PAYLOAD_64KB: usize = 64 * 1024;

#[tokio::test]
async fn large_payload_roundtrip_qos0() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "lp0-sub"))
        .await
        .expect("connect sub");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c.subscribe(sub("lp/qos0")).await.expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "lp0-pub"))
        .await
        .expect("connect pub");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    let payload: Vec<u8> = (0..PAYLOAD_64KB).map(|i| (i % 251) as u8).collect();
    pub_c
        .publish(ClientMessage {
            topic: Topic::try_new(b"lp/qos0".to_vec()).expect("valid topic"),
            payload: Payload::from(payload.clone()),
            qos: Qos::AtMostOnce,
            ..ClientMessage::default()
        })
        .await
        .expect("publish");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(5), el_sub.poll())
        .await
        .expect("message within 5s")
        .expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert_eq!(
        broker_msg.payload.as_ref().len(),
        PAYLOAD_64KB,
        "payload length must be {PAYLOAD_64KB}"
    );
    assert_eq!(
        &broker_msg.payload.as_ref()[..],
        &payload[..],
        "payload content must match"
    );
}

#[tokio::test]
async fn large_payload_roundtrip_qos1() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "lp1-sub"))
        .await
        .expect("connect sub");
    assert!(matches!(
        el_sub.poll().await.expect("poll"),
        Event::Connected
    ));
    sub_c
        .subscribe(sub_qos1("lp/qos1"))
        .await
        .expect("subscribe");
    let _ = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "lp1-pub"))
        .await
        .expect("connect pub");
    assert!(matches!(
        el_pub.poll().await.expect("poll"),
        Event::Connected
    ));

    let payload: Vec<u8> = (0..PAYLOAD_64KB).map(|i| (i % 251) as u8).collect();
    pub_c
        .publish(ClientMessage {
            topic: Topic::try_new(b"lp/qos1".to_vec()).expect("valid topic"),
            payload: Payload::from(payload.clone()),
            qos: Qos::AtLeastOnce,
            ..ClientMessage::default()
        })
        .await
        .expect("publish");
    let ev_pub = tokio::time::timeout(Duration::from_secs(5), el_pub.poll())
        .await
        .expect("puback within 5s")
        .expect("event");
    assert!(
        matches!(ev_pub, Event::PublishAcknowledged(_, _)),
        "expected PublishAcknowledged, got {ev_pub:?}"
    );

    let ev = tokio::time::timeout(Duration::from_secs(5), el_sub.poll())
        .await
        .expect("message within 5s")
        .expect("event");
    let broker_msg = match ev {
        Event::MessageWithRequiredAcknowledgement(_, m) => m,
        other => panic!("expected MessageWithRequiredAcknowledgement, got {other:?}"),
    };
    assert_eq!(
        broker_msg.payload.as_ref().len(),
        PAYLOAD_64KB,
        "payload length must be {PAYLOAD_64KB}"
    );
    assert_eq!(
        &broker_msg.payload.as_ref()[..],
        &payload[..],
        "payload content must match"
    );
}
