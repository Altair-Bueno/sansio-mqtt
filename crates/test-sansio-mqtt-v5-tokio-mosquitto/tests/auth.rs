use test_sansio_mqtt_v5_tokio_mosquitto::*;

#[tokio::test]
async fn valid_credentials_accepted() {
    let (_container, port) = authenticated_broker().await;

    let (_client, mut event_loop) = connect(authenticated_connect_options(
        port,
        "valid-auth-client",
        "testuser",
        "testpassword",
    ))
    .await
    .expect("connect");

    let event = event_loop.poll().await.expect("poll");
    assert!(
        matches!(event, Event::Connected),
        "expected Connected, got {event:?}"
    );
}

#[tokio::test]
async fn invalid_credentials_rejected() {
    let (_container, port) = authenticated_broker().await;

    let (_client, mut event_loop) = connect(authenticated_connect_options(
        port,
        "invalid-auth-client",
        "testuser",
        "wrongpassword",
    ))
    .await
    .expect("connect");

    let result = event_loop.poll().await;
    assert!(
        result.is_err(),
        "expected Err for wrong password, got {result:?}"
    );
}

#[tokio::test]
async fn anonymous_rejected() {
    let (_container, port) = authenticated_broker().await;

    let (_client, mut event_loop) = connect(connect_options(port, "anon-client"))
        .await
        .expect("connect");

    let result = event_loop.poll().await;
    assert!(
        result.is_err(),
        "expected Err for anonymous connection, got {result:?}"
    );
}
