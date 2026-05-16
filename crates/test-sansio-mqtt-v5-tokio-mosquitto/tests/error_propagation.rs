use std::time::Duration;

use test_sansio_mqtt_v5_tokio_mosquitto::*;

#[tokio::test]
async fn publish_after_disconnect_returns_closed() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "ep-pub"))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    client.disconnect().await.expect("disconnect");
    let ev = tokio::time::timeout(Duration::from_secs(3), el.poll())
        .await
        .expect("Disconnected within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Disconnected(None)),
        "expected Disconnected(None), got {ev:?}"
    );
    // Drop the event loop to close the MPSC receiver; only then does the
    // sender side return ClientError::Closed.
    drop(el);

    let result = client
        .publish(msg("ep/topic", b"after-disconnect", Qos::AtMostOnce))
        .await;
    assert_eq!(
        result,
        Err(ClientError::Closed),
        "publish after disconnect must return ClientError::Closed"
    );
}

#[tokio::test]
async fn subscribe_after_disconnect_returns_closed() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "ep-sub"))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    client.disconnect().await.expect("disconnect");
    let ev = tokio::time::timeout(Duration::from_secs(3), el.poll())
        .await
        .expect("Disconnected within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Disconnected(None)),
        "expected Disconnected(None), got {ev:?}"
    );
    drop(el);

    let result = client.subscribe(sub("ep/topic")).await;
    assert_eq!(
        result,
        Err(ClientError::Closed),
        "subscribe after disconnect must return ClientError::Closed"
    );
}

#[tokio::test]
async fn unsubscribe_after_disconnect_returns_closed() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "ep-unsub"))
        .await
        .expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    client.disconnect().await.expect("disconnect");
    let ev = tokio::time::timeout(Duration::from_secs(3), el.poll())
        .await
        .expect("Disconnected within 3s")
        .expect("event");
    assert!(
        matches!(ev, Event::Disconnected(None)),
        "expected Disconnected(None), got {ev:?}"
    );
    drop(el);

    let result = client
        .unsubscribe(UnsubscribeOptions {
            filter: Utf8String::try_from("ep/topic").expect("valid"),
            extra_filters: vec![],
            user_properties: vec![],
        })
        .await;
    assert_eq!(
        result,
        Err(ClientError::Closed),
        "unsubscribe after disconnect must return ClientError::Closed"
    );
}
