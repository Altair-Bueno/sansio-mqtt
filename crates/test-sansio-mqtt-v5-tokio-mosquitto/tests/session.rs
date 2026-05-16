use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Test 1: clean_start=true wipes any existing session — queued QoS 1 messages
/// are NOT delivered.
#[tokio::test]
async fn clean_start_clears_session() {
    let (_container, port) = anonymous_broker().await;

    let mut conn1 = Connection::connect(persistent_connect_options(port, "clean-start-client"))
        .await
        .expect("connect phase 1");
    assert!(
        matches!(
            conn1.poll().await.expect("connected phase 1"),
            Event::Connected
        ),
        "expected Connected"
    );
    conn1
        .subscribe(sub_qos1("test/clean-start"))
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    conn1.disconnect().expect("disconnect phase 1");
    let _ = tokio::time::timeout(Duration::from_secs(1), conn1.poll()).await;

    let mut conn_pub = Connection::connect(connect_options(port, "clean-start-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        conn_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    conn_pub
        .publish(msg("test/clean-start", b"queued", Qos::AtLeastOnce))
        .expect("publish");
    let pub_event = conn_pub.poll().await.expect("puback");
    assert!(
        matches!(pub_event, Event::PublishAcknowledged(_, _)),
        "expected PublishAcknowledged, got {pub_event:?}"
    );

    let mut conn3 = Connection::connect(connect_options(port, "clean-start-client"))
        .await
        .expect("connect phase 3");
    assert!(
        matches!(
            conn3.poll().await.expect("connected phase 3"),
            Event::Connected
        ),
        "expected Connected after clean reconnect"
    );

    let result = tokio::time::timeout(Duration::from_millis(300), conn3.poll()).await;
    assert!(
        result.is_err(),
        "expected no message after clean_start, but got an event: {result:?}"
    );
}

/// Test 2: session resumption — messages queued while offline are delivered on
/// reconnect.
#[tokio::test]
async fn session_resumption() {
    let (_container, port) = anonymous_broker().await;

    let mut conn1 = Connection::connect(persistent_connect_options(port, "resume-client"))
        .await
        .expect("connect phase 1");
    assert!(
        matches!(
            conn1.poll().await.expect("connected phase 1"),
            Event::Connected
        ),
        "expected Connected"
    );
    conn1.subscribe(sub_qos1("test/resume")).expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    conn1.disconnect().expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), conn1.poll()).await;

    let mut conn_pub = Connection::connect(connect_options(port, "resume-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        conn_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    conn_pub
        .publish(msg("test/resume", b"queued-for-resume", Qos::AtLeastOnce))
        .expect("publish");
    let pub_event = conn_pub.poll().await.expect("puback");
    assert!(
        matches!(pub_event, Event::PublishAcknowledged(_, _)),
        "expected PublishAcknowledged, got {pub_event:?}"
    );

    let mut conn3 = Connection::connect(resume_connect_options(port, "resume-client"))
        .await
        .expect("connect phase 3");
    assert!(
        matches!(
            conn3.poll().await.expect("connected phase 3"),
            Event::Connected
        ),
        "expected Connected on resume"
    );

    let result = tokio::time::timeout(Duration::from_secs(3), conn3.poll())
        .await
        .expect("message must arrive within 3 s")
        .expect("event loop ok");
    assert!(
        matches!(result, Event::MessageWithRequiredAcknowledgement(_, _)),
        "expected MessageWithRequiredAcknowledgement, got {result:?}"
    );
}

/// Test 3: will message is delivered to a subscriber when the sender
/// disconnects abruptly.
#[tokio::test]
async fn will_message_delivered() {
    let (_container, port) = anonymous_broker().await;

    let mut conn_sub = Connection::connect(connect_options(port, "will-subscriber"))
        .await
        .expect("connect subscriber");
    assert!(matches!(
        conn_sub.poll().await.expect("subscriber connected"),
        Event::Connected
    ));
    conn_sub.subscribe(sub("will/gone")).expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let sub_task = tokio::spawn(async move { conn_sub.poll().await });

    let will = Will {
        topic: Topic::try_new("will/gone").expect("valid will topic"),
        payload: Payload::from(b"sender-gone".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        will_delay_interval: None,
        ..Will::default()
    };

    let will_opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("will-sender").expect("valid"),
            will: Some(will),
            session_expiry_interval: Some(0),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };

    let mut conn_will = Connection::connect(will_opts)
        .await
        .expect("connect will sender");
    assert!(
        matches!(
            conn_will.poll().await.expect("will sender connected"),
            Event::Connected
        ),
        "expected will sender Connected"
    );

    // Dropping the connection without sending DISCONNECT triggers the broker's will
    // publication.
    drop(conn_will);

    let event = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("will message must arrive within 3 s")
        .expect("sub task joined")
        .expect("subscriber event");
    assert!(
        matches!(event, Event::Message(_)),
        "expected Message (will), got {event:?}"
    );
}
