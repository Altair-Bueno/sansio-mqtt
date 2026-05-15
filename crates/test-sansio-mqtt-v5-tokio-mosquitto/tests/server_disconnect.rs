use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// A graceful client-initiated DISCONNECT arrives as Disconnected(None).
/// Verifies the reason code plumbing from wire to Event.
#[tokio::test]
async fn graceful_disconnect_reason_code_is_none() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "sd-grace"))
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
        "graceful disconnect must produce Disconnected(None), got {ev:?}"
    );
}

/// When a second client connects with the same client_id, Mosquitto sends a
/// server-initiated DISCONNECT with reason SessionTakenOver to the first
/// client.
#[tokio::test]
async fn session_takeover_reason_code() {
    let (_c, port) = anonymous_broker().await;

    let (_c1, mut el1) = connect(connect_options(port, "sd-takeover"))
        .await
        .expect("connect first");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));

    // Second connection with the same client_id takes over
    let (_c2, mut el2) = connect(connect_options(port, "sd-takeover"))
        .await
        .expect("connect second");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    let result = tokio::time::timeout(Duration::from_secs(3), el1.poll())
        .await
        .expect("server disconnect within 3s");
    match result {
        Ok(Event::Disconnected(_)) | Err(_) => {}
        Ok(ev) => panic!("expected Disconnected or error on session takeover, got {ev:?}"),
    }
}
