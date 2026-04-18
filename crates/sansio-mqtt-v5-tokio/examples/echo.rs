use std::net::ToSocketAddrs;

use sansio_mqtt_v5_protocol::{ClientMessage, SubscribeOptions};
use sansio_mqtt_v5_tokio::{connect, ConnectOptions, Event};
use sansio_mqtt_v5_types::{Payload, Qos, Topic, Utf8String};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    let broker_addr = std::env::var("BROKER")
        .unwrap_or(String::from("test.mosquitto.org:1883"))
        .to_socket_addrs()?
        .next()
        .expect("Failed to resolve broker address");
    tracing::info!(%broker_addr, "Connecting to broker");

    let connect_options = ConnectOptions {
        addr: broker_addr,
        ..ConnectOptions::default()
    };

    let (client, mut event_loop) = connect(connect_options).await?;

    client
        .subscribe(SubscribeOptions {
            subscription: Utf8String::new("echo/+/in"),
            qos: Qos::AtLeastOnce,
            no_local: true,
            ..Default::default()
        })
        .await?;

    loop {
        tokio::select! {
            event = event_loop.poll() => {
                match event? {
                    Event::Connected => {
                        tracing::info!("Connected to broker");
                    }
                    Event::Disconnected => {
                        tracing::info!("Disconnected from broker");
                    }
                    Event::Message(message) => {
                        tracing::info!(topic = %message.topic, "Received message");
                        stdout.write_all(message.payload.as_ref().as_ref()).await?;
                        stdout.write_all(b"\n").await?;
                        stdout.flush().await?;
                    }
                    event => {
                        tracing::info!("Received event: {event:?}");
                    }
                }
            }
            line = stdin_lines.next_line() => {
                let Some(line) = line? else {
                    let _ = client.disconnect().await;
                    return Ok(());
                };
                client.publish(ClientMessage {
                    topic: Topic::try_new(format!("echo/{}/in", line))?,
                    payload: Payload::new(b"hello world".as_slice()),
                    qos: Qos::AtLeastOnce,
                    ..Default::default()
                }).await?;
            }
            signal_result = tokio::signal::ctrl_c() => {
                signal_result?;
                let _ = client.disconnect().await;
                return Ok(());
            }
        }
    }
}
