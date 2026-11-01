//! Connects over a WebSocket (MQTT v5.0 §6), publishes to a topic it is
//! subscribed to and prints what it receives. Uses
//! `ws://test.mosquitto.org:8080/mqtt` unless `MQTT_WS_URI` says otherwise.

use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_tokio::TcpConnector;
use sansio_mqtt_tokio::WebSocketConnector;
use sansio_mqtt_tokio::tungstenite::http::Uri;
use sansio_mqtt_v5_protocol::Client;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env()?,
        )
        .init();

    let uri: Uri = std::env::var("MQTT_WS_URI")
        .unwrap_or_else(|_| "ws://test.mosquitto.org:8080/mqtt".to_owned())
        .parse()?;
    let host = uri.host().ok_or("MQTT_WS_URI has no host")?.to_owned();
    let port = uri.port_u16().unwrap_or(80);
    let connector = WebSocketConnector::new(TcpConnector::new((host, port)), uri)?;
    let mut driver = Driver::new(Client::default(), connector);

    tracing::info!("connecting to broker");
    driver.submit(Command::Connect(
        ConnectOptions::builder()
            .client_id("sansio-mqtt-ws-example")
            .build(),
    ))?;

    loop {
        match driver.next().await {
            Event::Connected => {
                tracing::info!("connected over WebSocket");
                driver.submit(Command::Subscribe(
                    SubscribeOptions::builder()
                        .subscriptions(vec![
                            Subscription::builder().filter("sansio-mqtt/ws").build(),
                        ])
                        .build(),
                ))?;
                driver.submit(Command::Publish {
                    token: 1,
                    message: Message::builder()
                        .topic("sansio-mqtt/ws")
                        .payload("Hello over WebSocket!")
                        .build(),
                })?;
            }
            Event::Message(message) => {
                tracing::info!(topic = %message.topic, payload = ?message.payload, "received a message");
                driver.submit(Command::Disconnect)?;
            }
            Event::Disconnected(reason) => {
                tracing::info!(?reason, "disconnected");
                break;
            }
            event => tracing::debug!(?event, "received an event but was not handled"),
        }
    }
    Ok(())
}
