use std::time::Duration;

use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Verifies automatic reconnection with backoff using the "session steal" technique:
/// a second client connecting with the same client_id causes the broker to disconnect
/// the first client, which then reconnects automatically via the configured backoff.
#[tokio::test]
async fn reconnect_after_session_steal() {
    let (_container, port) = anonymous_broker().await;

    // Connect the main client with backoff configured so it reconnects after disconnect.
    let backoff_opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("reconnect-test").expect("valid client id"),
            ..ConnectionOptions::default()
        },
        backoff: Some(Backoff {
            algorithm: BackoffAlgorithm::Linear {
                slope: Duration::from_millis(100),
            },
            range: Duration::from_millis(100)..=Duration::from_secs(1),
            seed: 42,
        }),
        ..ConnectOptions::default()
    };

    let mut conn = Connection::connect(backoff_opts).await.expect("connect main");

    // Wait for initial connection.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match conn.poll().await.expect("poll main (initial connect)") {
                Event::Connected => break,
                _ => {}
            }
        }
    })
    .await
    .expect("main client connected within 5 seconds");

    // Steal the session: connect a second client with the same client_id.
    // The broker will close the first client's connection [MQTT-3.1.4-2].
    let stealer_opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("reconnect-test").expect("valid client id"),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let mut conn_stealer =
        Connection::connect(stealer_opts).await.expect("connect stealer");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match conn_stealer
                .poll()
                .await
                .expect("stealer poll (initial connect)")
            {
                Event::Connected => break,
                _ => {}
            }
        }
    })
    .await
    .expect("stealer connected within 5 seconds");

    // The main client should now see Disconnected (broker kicked it).
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match conn.poll().await {
                Ok(Event::Disconnected(_)) => break,
                Ok(_) => {}
                Err(e) => panic!("unexpected error waiting for disconnect on main client: {e}"),
            }
        }
    })
    .await
    .expect("main client disconnected within 5 seconds");

    // The backoff loop will reconnect automatically — poll until Connected.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match conn.poll().await {
                Ok(Event::Connected) => break,
                Ok(_) => {}
                Err(e) => panic!("unexpected error while waiting for reconnect: {e}"),
            }
        }
    })
    .await
    .expect("main client reconnected within 10 seconds");

    // Re-subscribe after reconnect: clean_start=true means no session state is
    // preserved on the broker, so subscriptions must be re-established manually.
    conn.subscribe(sub("test/reconnect")).expect("re-subscribe after reconnect");

    // Drive the event loop to flush the SUBSCRIBE packet and receive the SUBACK.
    // Without polling here, the SUBSCRIBE bytes stay in the protocol write buffer
    // and the broker never processes the subscription.
    let _ = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            conn.poll().await.expect("poll during subscribe flush");
        }
    })
    .await;

    // Publish from the stealer to the topic.
    conn_stealer
        .publish(msg("test/reconnect", b"hello-after-reconnect", Qos::AtMostOnce))
        .expect("publish from stealer");
    // Flush the stealer's PUBLISH packet; QoS 0 has no response so a timeout is expected.
    let _ = tokio::time::timeout(Duration::from_millis(200), conn_stealer.poll()).await;

    // The main client should receive the message.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match conn.poll().await {
                Ok(Event::Message(_)) => break,
                Ok(_) => {}
                Err(e) => panic!("unexpected error while waiting for message: {e}"),
            }
        }
    })
    .await
    .expect("message received within 5 seconds");
}
