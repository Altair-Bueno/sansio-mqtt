//! A simple MQTT client that sends stdin lines to the broker and prints received messages to stdout.
//!
//! The client is configured using environment variables:
//! - `BROKER`: The address of the MQTT broker to connect to (default: `test.mosquitto.org:1883`)
//! - `SUBSCRIPTION`: The subscription filter to subscribe to (default: `echo/#`)
//! - `TOPIC`: The topic to publish to (default: `echo`)
//! - `RUST_LOG`: The log level for the client (default: `info`). See the `tracing-subscriber` documentation for more details on log levels and configuration.

use std::net::ToSocketAddrs;

use sansio_mqtt_v5_protocol::{ClientMessage, SubscribeOptions};
use sansio_mqtt_v5_tokio::{connect, ConnectOptions, Event};
use sansio_mqtt_v5_types::{Payload, Qos, RetainHandling, Subscription, Topic, Utf8String};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main(flavor = "current_thread")]
#[tracing::instrument]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing for logs
    tracing_subscriber::fmt::init();

    // Configuration
    let broker_addr = std::env::var("BROKER")
        .unwrap_or(String::from("test.mosquitto.org:1883"))
        .to_socket_addrs()?
        .next()
        .expect("Failed to resolve broker address");
    let subscription_filter = std::env::var("SUBSCRIPTION").unwrap_or(String::from("echo/#"));
    let topic = std::env::var("TOPIC").unwrap_or(String::from("echo"));
    let subscription_filter = Utf8String::try_new(subscription_filter)?;
    let topic = Topic::try_new(topic)?;

    tracing::info!(%broker_addr, "Connecting to address");
    let (client, mut event_loop) = connect(ConnectOptions {
        addr: broker_addr,
        ..ConnectOptions::default()
    })
    .await?;

    // Loop state
    let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    let mut connected = false;
    loop {
        tokio::select! {
            event = event_loop.poll() => {
                match event? {
                    Event::Connected => {
                        tracing::info!("Connected to broker");
                        if !connected {
                            connected = true;
                            tracing::info!(%subscription_filter, "Subscribing to topic filter");
                            client
                                .subscribe(SubscribeOptions {
                                    subscription: Subscription {
                                        topic_filter: subscription_filter.clone(),
                                        qos: Qos::AtLeastOnce,
                                        no_local: true,
                                        retain_as_published: false,
                                        retain_handling: RetainHandling::SendRetained,
                                    },
                                    extra_subscriptions: Vec::new(),
                                    subscription_identifier: None,
                                    user_properties: Vec::new(),
                                })
                                .await?;
                        }
                    }
                    Event::Disconnected(reason_code) => {
                        tracing::info!(?reason_code, "Disconnected from broker");
                    }
                    Event::Message(message) => {
                        tracing::info!(topic = %message.topic, len = message.payload.len(), "Received message");
                        stdout.write_all(b"[").await?;
                        stdout.write_all(message.topic.as_bytes()).await?;
                        stdout.write_all(b"]\t").await?;
                        stdout.write_all(&message.payload).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    }
                    Event::MessageWithRequiredAcknowledgement(message_id, message) => {
                        tracing::info!(topic = %message.topic, len = message.payload.len(), ?message_id, "Received message requiring acknowledgement");
                        stdout.write_all(b"[").await?;
                        stdout.write_all(message.topic.as_bytes()).await?;
                        stdout.write_all(b"]\t").await?;
                        stdout.write_all(&message.payload).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    }
                    event => {
                        tracing::info!("Unhandled event received: {:?}", event);
                    }
                }
            }
            line = stdin_lines.next_line() => {
                let Some(line) = line? else {
                    tracing::info!("STDIN was closed, disconnecting and exiting");
                    let _ = client.disconnect().await;
                    break;
                };

                if line.trim().is_empty() {
                    tracing::info!("Empty line from stdin, ignoring");
                    continue;
                }

                tracing::info!(len = line.len(), "Publishing line from stdin");
                client.publish(ClientMessage {
                    topic: topic.clone(),
                    payload: Payload::try_new(line)?,
                    qos: Qos::AtLeastOnce,
                    ..Default::default()
                }).await?;
            }
            signal_result = tokio::signal::ctrl_c() => {
                signal_result?;
                tracing::info!("Received Ctrl+C, disconnecting and exiting");
                let _ = client.disconnect().await;
                break;
            }
        }
    }
    tracing::info!("Shutdown complete");
    Ok(())
}
