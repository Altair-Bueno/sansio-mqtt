use std::time::Duration;

use test_sansio_mqtt_v5_tokio_mosquitto::*;

#[tokio::test]
async fn connect_and_disconnect() {
    let (_container, port) = anonymous_broker().await;

    let (client, mut event_loop) = connect(connect_options(port, "conn-disc"))
        .await
        .expect("connect");

    let event = event_loop.poll().await.expect("poll");
    assert!(
        matches!(event, Event::Connected),
        "expected Connected, got {event:?}"
    );

    client.disconnect().await.expect("disconnect");

    let event = event_loop.poll().await.expect("poll after disconnect");
    assert!(
        matches!(event, Event::Disconnected(None)),
        "expected Disconnected(None), got {event:?}"
    );
}

#[tokio::test]
async fn publish_qos0() {
    let (_container, port) = anonymous_broker().await;

    // Subscriber: connect, subscribe at QoS 0, wait in background for the message.
    let (client_sub, mut el_sub) = connect(connect_options(port, "qos0-sub"))
        .await
        .expect("connect subscriber");
    assert!(matches!(
        el_sub.poll().await.expect("subscriber connected"),
        Event::Connected
    ));
    client_sub
        .subscribe(sub("test/qos0"))
        .await
        .expect("subscribe");
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    // Allow time for SUBSCRIBE/SUBACK round-trip before publishing.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Publisher: connect, publish QoS 0, give event loop time to flush the packet.
    let (client_pub, mut el_pub) = connect(connect_options(port, "qos0-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        el_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    client_pub
        .publish(msg("test/qos0", b"hello-qos0", Qos::AtMostOnce))
        .await
        .expect("publish");
    // Poll briefly so the PUBLISH is flushed; timeout is expected because QoS 0 has
    // no response.
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    let event = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("subscriber receives message within 3s")
        .expect("sub task joins")
        .expect("subscriber event");
    assert!(
        matches!(event, Event::Message(_)),
        "expected Message, got {event:?}"
    );
}

#[tokio::test]
async fn publish_qos1() {
    let (_container, port) = anonymous_broker().await;

    // Subscriber at QoS 0 — broker delivers at most-once regardless of publish QoS.
    let (client_sub, mut el_sub) = connect(connect_options(port, "qos1-sub"))
        .await
        .expect("connect subscriber");
    assert!(matches!(
        el_sub.poll().await.expect("subscriber connected"),
        Event::Connected
    ));
    client_sub
        .subscribe(sub("test/qos1"))
        .await
        .expect("subscribe");
    let sub_task = tokio::spawn(async move { el_sub.poll().await });
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Publisher at QoS 1 — expects PublishAcknowledged from broker.
    let (client_pub, mut el_pub) = connect(connect_options(port, "qos1-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        el_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    client_pub
        .publish(msg("test/qos1", b"hello-qos1", Qos::AtLeastOnce))
        .await
        .expect("publish");
    let pub_event = el_pub.poll().await.expect("puback");
    assert!(
        matches!(pub_event, Event::PublishAcknowledged(_, _)),
        "expected PublishAcknowledged, got {pub_event:?}"
    );

    let sub_event = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("subscriber receives message within 3s")
        .expect("sub task joins")
        .expect("subscriber event");
    assert!(
        matches!(sub_event, Event::Message(_)),
        "expected Message, got {sub_event:?}"
    );
}

#[tokio::test]
async fn publish_qos2() {
    let (_container, port) = anonymous_broker().await;

    // Subscriber at QoS 0.
    let (client_sub, mut el_sub) = connect(connect_options(port, "qos2-sub"))
        .await
        .expect("connect subscriber");
    assert!(matches!(
        el_sub.poll().await.expect("subscriber connected"),
        Event::Connected
    ));
    client_sub
        .subscribe(sub("test/qos2"))
        .await
        .expect("subscribe");
    let sub_task = tokio::spawn(async move { el_sub.poll().await });
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Publisher at QoS 2 — full PUBLISH→PUBREC→PUBREL→PUBCOMP flow.
    let (client_pub, mut el_pub) = connect(connect_options(port, "qos2-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        el_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    client_pub
        .publish(msg("test/qos2", b"hello-qos2", Qos::ExactlyOnce))
        .await
        .expect("publish");
    let pub_event = el_pub.poll().await.expect("pubcomp");
    assert!(
        matches!(pub_event, Event::PublishCompleted(_, _)),
        "expected PublishCompleted, got {pub_event:?}"
    );

    let sub_event = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("subscriber receives message within 3s")
        .expect("sub task joins")
        .expect("subscriber event");
    assert!(
        matches!(sub_event, Event::Message(_)),
        "expected Message, got {sub_event:?}"
    );
}

#[tokio::test]
async fn keep_alive_maintains_connection() {
    let (_container, port) = anonymous_broker().await;

    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("keepalive-test").expect("valid"),
            keep_alive: core::num::NonZero::new(5),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };

    let (_client, mut event_loop) = connect(opts).await.expect("connect");
    assert!(matches!(
        event_loop.poll().await.expect("connected"),
        Event::Connected
    ));

    // Poll for 7 seconds. PINGREQ/PINGRESP are transparent — no Event emitted.
    // A timeout means the connection stayed alive. An Ok result means an unexpected
    // event arrived. An Err result means the connection dropped.
    let result = tokio::time::timeout(Duration::from_secs(7), event_loop.poll()).await;
    match result {
        Err(_elapsed) => { /* success: connection alive after 7s idle */ }
        Ok(Err(err)) => panic!("connection dropped during keep-alive window: {err:?}"),
        Ok(Ok(event)) => panic!("unexpected event during keep-alive window: {event:?}"),
    }
}
