use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;
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
    let (driver_handle, commands, mut events) =
        sansio_mqtt_tokio::spawn(Client::default(), || async {
            let stream = TcpStream::connect("test.mosquitto.org:1883").await?;
            stream.set_nodelay(true)?;
            let stream = BufStream::new(stream);
            Ok(stream)
        })
        .await?;

    let commands_signal = commands.clone();
    let commands_mqtt = commands.clone();

    tracing::info!("spawning event consumer");
    let mqtt_task = async move {
        while let Some(event) = events.recv().await {
            match event {
                Event::Disconnected(_) => break,
                Event::Connected => {
                    tracing::info!("subscribing to all topics");
                    commands_mqtt
                        .send(Command::Subscribe(
                            SubscribeOptions::builder()
                                .subscriptions(vec![
                                    Subscription::builder().filter("test/topic").build(),
                                ])
                                .build(),
                        ))
                        .await?;

                    commands_mqtt
                        .send(Command::Publish {
                            token: 1,
                            message: Message::builder()
                                .topic("test/topic")
                                .payload("Hello, MQTT!")
                                .build(),
                        })
                        .await?;
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
            };
        }
        Ok::<_, Box<dyn core::error::Error + Send + Sync>>(())
    };

    let signal_task = async move {
        tokio::signal::ctrl_c().await?;
        tracing::info!("shutting down system");
        commands_signal.send(Command::Disconnect).await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };

    tracing::info!("connecting to broker");
    commands
        .send(Command::Connect(ConnectOptions::builder().build()))
        .await?;

    drop(commands);

    let (r1, r2, r3) = tokio::join!(driver_handle, mqtt_task, signal_task);
    r1??;
    r2?;
    r3?;

    tracing::info!("Driver finished");
    Ok(())
}
