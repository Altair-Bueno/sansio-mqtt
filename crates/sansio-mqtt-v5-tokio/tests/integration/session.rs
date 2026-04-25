use std::time::Duration;

use sansio_mqtt_v5_protocol::{ConnectionOptions, Will};
use sansio_mqtt_v5_tokio::{connect, ConnectOptions, Event};
use sansio_mqtt_v5_types::{Payload, Qos, Topic, Utf8String};

use crate::common;

/// Test 1: clean_start=true wipes any existing session — queued QoS 1 messages are NOT delivered.
#[tokio::test]
async fn clean_start_clears_session() {
    let (_container, port) = common::anonymous_broker().await;

    let (client1, mut el1) = connect(common::persistent_connect_options(
        port,
        "clean-start-client",
    ))
    .await
    .expect("connect phase 1");
    assert!(
        matches!(
            el1.poll().await.expect("connected phase 1"),
            Event::Connected
        ),
        "expected Connected"
    );
    client1
        .subscribe(common::sub_qos1("test/clean-start"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    client1.disconnect().await.expect("disconnect phase 1");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    let (client_pub, mut el_pub) = connect(common::connect_options(port, "clean-start-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        el_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    client_pub
        .publish(common::msg("test/clean-start", b"queued", Qos::AtLeastOnce))
        .await
        .expect("publish");
    let pub_event = el_pub.poll().await.expect("puback");
    assert!(
        matches!(pub_event, Event::PublishAcknowledged(_, _)),
        "expected PublishAcknowledged, got {pub_event:?}"
    );

    let (_client, mut el3) = connect(common::connect_options(port, "clean-start-client"))
        .await
        .expect("connect phase 3");
    assert!(
        matches!(
            el3.poll().await.expect("connected phase 3"),
            Event::Connected
        ),
        "expected Connected after clean reconnect"
    );

    let result = tokio::time::timeout(Duration::from_millis(300), el3.poll()).await;
    assert!(
        result.is_err(),
        "expected no message after clean_start, but got an event: {result:?}"
    );
}

/// Test 2: session resumption — messages queued while offline are delivered on reconnect.
#[tokio::test]
async fn session_resumption() {
    let (_container, port) = common::anonymous_broker().await;

    let (client1, mut el1) = connect(common::persistent_connect_options(port, "resume-client"))
        .await
        .expect("connect phase 1");
    assert!(
        matches!(
            el1.poll().await.expect("connected phase 1"),
            Event::Connected
        ),
        "expected Connected"
    );
    client1
        .subscribe(common::sub_qos1("test/resume"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    client1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    let (client_pub, mut el_pub) = connect(common::connect_options(port, "resume-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        el_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    client_pub
        .publish(common::msg(
            "test/resume",
            b"queued-for-resume",
            Qos::AtLeastOnce,
        ))
        .await
        .expect("publish");
    let pub_event = el_pub.poll().await.expect("puback");
    assert!(
        matches!(pub_event, Event::PublishAcknowledged(_, _)),
        "expected PublishAcknowledged, got {pub_event:?}"
    );

    let (_client, mut el3) = connect(common::resume_connect_options(port, "resume-client"))
        .await
        .expect("connect phase 3");
    assert!(
        matches!(
            el3.poll().await.expect("connected phase 3"),
            Event::Connected
        ),
        "expected Connected on resume"
    );

    let result = tokio::time::timeout(Duration::from_secs(3), el3.poll())
        .await
        .expect("message must arrive within 3 s")
        .expect("event loop ok");
    assert!(
        matches!(result, Event::MessageWithRequiredAcknowledgement(_, _)),
        "expected MessageWithRequiredAcknowledgement, got {result:?}"
    );
}

/// Test 3: will message is delivered to a subscriber when the sender disconnects abruptly.
#[tokio::test]
async fn will_message_delivered() {
    let (_container, port) = common::anonymous_broker().await;

    let (client_sub, mut el_sub) = connect(common::connect_options(port, "will-subscriber"))
        .await
        .expect("connect subscriber");
    assert!(matches!(
        el_sub.poll().await.expect("subscriber connected"),
        Event::Connected
    ));
    client_sub
        .subscribe(common::sub("will/gone"))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let sub_task = tokio::spawn(async move { el_sub.poll().await });

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

    let (client_will, mut el_will) = connect(will_opts).await.expect("connect will sender");
    assert!(
        matches!(
            el_will.poll().await.expect("will sender connected"),
            Event::Connected
        ),
        "expected will sender Connected"
    );

    // Dropping the event loop without sending DISCONNECT triggers the broker's will publication.
    drop(el_will);

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
