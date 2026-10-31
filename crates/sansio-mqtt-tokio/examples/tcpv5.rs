use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_v5_protocol::Client;
use tokio::io::BufStream;
use tokio::net::TcpStream;
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
    tracing::info!("initializing client");
    let mut driver = Driver::new(Client::default(), || async {
        let stream = TcpStream::connect("test.mosquitto.org:1883").await?;
        stream.set_nodelay(true)?;
        Ok(BufStream::new(stream))
    });

    tracing::info!("connecting to broker");
    driver.submit(Command::Connect(ConnectOptions::builder().build()))?;

    let mut shutdown = std::pin::pin!(tokio::signal::ctrl_c());
    let mut shutting_down = false;
    loop {
        let event = tokio::select! {
            event = driver.next() => event,
            result = &mut shutdown, if !shutting_down => {
                result?;
                shutting_down = true;
                tracing::info!("shutting down system");
                driver.submit(Command::Disconnect)?;
                continue;
            }
        };
        match event {
            Event::Disconnected(_) => break,
            Event::Connected => {
                tracing::info!("subscribing to all topics");
                driver.submit(Command::Subscribe(
                    SubscribeOptions::builder()
                        .subscriptions(vec![Subscription::builder().filter("test/topic").build()])
                        .build(),
                ))?;
                driver.submit(Command::Publish {
                    token: 1,
                    message: Message::builder()
                        .topic("test/topic")
                        .payload("Hello, MQTT!")
                        .build(),
                })?;
            }
            Event::Message(Message {
                topic,
                payload,
                qos,
                retain,
                payload_format,
                message_expiry,
                response_topic,
                correlation_data,
                content_type,
                user_properties,
                subscription_identifiers,
                ..
            }) => {
                tracing::info!(
                    %topic,
                    ?payload,
                    ?qos,
                    %retain,
                    ?payload_format,
                    ?message_expiry,
                    ?response_topic,
                    ?correlation_data,
                    ?content_type,
                    ?user_properties,
                    ?subscription_identifiers,
                    "received a message"
                );
            }
            event => tracing::debug!(?event, "received an event but was not handled"),
        }
    }

    tracing::info!("Driver finished");
    Ok(())
}
