use std::time::Duration;

use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Connects, asserts `Connected`, sends a graceful DISCONNECT, drains until
/// `Disconnected(None)`, then drops the EventLoop (closing the MPSC receiver).
/// Returns the `Client` — the next send on it will return
/// `ClientError::Closed`.
async fn connect_and_close(port: u16, client_id: &str) -> Client {
    let (client, mut el) = connect(connect_options(port, client_id))
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
    client
}

#[tokio::test]
async fn publish_after_disconnect_returns_closed() {
    let (_c, port) = anonymous_broker().await;
    let client = connect_and_close(port, "ep-pub").await;
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
    let client = connect_and_close(port, "ep-sub").await;
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
    let client = connect_and_close(port, "ep-unsub").await;
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
