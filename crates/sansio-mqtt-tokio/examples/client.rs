//! A small MQTT v5.0 client that runs over every transport this crate offers.
//! The broker URL's scheme picks the transport: `mqtt://` (plain TCP),
//! `mqtts://` (TLS), `ws://` (WebSocket, MQTT v5.0 §6) or `wss://`
//! (WebSocket over TLS). It subscribes to a topic, optionally publishes a
//! message to it and logs everything it receives until Ctrl-C.
//!
//! ```text
//! cargo run -p sansio-mqtt-tokio --all-features --example client -- --help
//! cargo run -p sansio-mqtt-tokio --all-features --example client -- mqtts://test.mosquitto.org:8886 -m hello -c 1
//! cargo run -p sansio-mqtt-tokio --all-features --example client -- ws://test.mosquitto.org:8080/mqtt -m hello -c 1
//! ```

use core::num::NonZero;
use std::path::PathBuf;

use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::Qos;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;
use sansio_mqtt_tokio::DynDriver;
use sansio_mqtt_tokio::connect;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use url::Url;

type BoxError = Box<dyn core::error::Error + Send + Sync>;

/// Connects to an MQTT v5.0 broker, subscribes to a topic, optionally
/// publishes a message to it and logs every message it receives until Ctrl-C.
///
/// The broker URL's scheme selects the transport: `mqtt://` for plain TCP
/// (port 1883 by default), `mqtts://` for TLS (8883), `ws://` for a
/// WebSocket (80) and `wss://` for a WebSocket over TLS (443). Set `RUST_LOG`
/// to change the log level.
#[derive(Debug, usage_rs::Cli)]
#[usage(bin = "client")]
struct Cli {
    /// The broker to connect to; its query may carry connection settings,
    /// e.g. `?keep_alive=30`
    #[usage(env = "MQTT_BROKER", default = "mqtt://test.mosquitto.org:1883")]
    url: Url,

    /// The topic filter to subscribe to, and the topic to publish to
    #[usage(short, long, default = "sansio-mqtt/example")]
    topic: String,

    /// The payload to publish once connected; nothing is published without it
    #[usage(short, long)]
    message: Option<String>,

    /// The Quality of Service of the subscription and the published message
    #[usage(short, long, choices("0", "1", "2"), default = "0")]
    qos: u8,

    /// Disconnect after receiving this many messages
    #[usage(short, long)]
    count: Option<NonZero<usize>>,

    /// The Client Identifier; the broker assigns one when omitted
    #[usage(short = 'i', long)]
    client_id: Option<String>,

    /// The User Name to authenticate with
    #[usage(short, long, env = "MQTT_USERNAME")]
    username: Option<String>,

    /// The Password to authenticate with
    #[usage(short = 'P', long, env = "MQTT_PASSWORD")]
    password: Option<String>,

    /// A PEM file of the only certificates to trust for TLS; the operating
    /// system's verifier is used when omitted
    #[usage(
        long,
        env = "MQTT_TLS_CA",
        value_name = "FILE",
        value_hint = usage_rs::ValueHint::FilePath
    )]
    ca: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BoxError> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env()?,
        )
        .init();
    // TLS brokers need a process-wide rustls crypto provider.
    drop(tokio_rustls::rustls::crypto::ring::default_provider().install_default());

    let mut url = cli.url.clone();
    if let Some(ca) = &cli.ca {
        url.query_pairs_mut()
            .append_pair("ca", ca.to_str().ok_or("the --ca path is not UTF-8")?);
    }
    let mut driver = connect(&url)?;
    tracing::info!(%url, "connecting to broker");
    driver.submit(cli.connect())?;
    run(&cli, &mut driver).await
}

impl Cli {
    const fn qos(&self) -> Qos {
        match self.qos {
            0 => Qos::AtMostOnce,
            1 => Qos::AtLeastOnce,
            _ => Qos::ExactlyOnce,
        }
    }

    fn connect(&self) -> Command {
        Command::Connect(
            ConnectOptions::builder()
                .client_id(self.client_id.clone().unwrap_or_default())
                .maybe_user_name(self.username.clone())
                .maybe_password(self.password.clone())
                .build(),
        )
    }

    fn subscribe(&self) -> Command {
        Command::Subscribe(
            SubscribeOptions::builder()
                .subscriptions(vec![
                    Subscription::builder()
                        .filter(self.topic.as_str())
                        .qos(self.qos())
                        .build(),
                ])
                .build(),
        )
    }

    fn publish(&self) -> Option<Command> {
        self.message.as_ref().map(|payload| Command::Publish {
            token: 1,
            message: Message::builder()
                .topic(self.topic.as_str())
                .payload(payload.clone())
                .qos(self.qos())
                .build(),
        })
    }
}

async fn run(cli: &Cli, driver: &mut DynDriver) -> Result<(), BoxError> {
    let mut shutdown = core::pin::pin!(tokio::signal::ctrl_c());
    let mut shutting_down = false;
    let mut remaining = cli.count.map(NonZero::get);
    loop {
        let event = tokio::select! {
            event = driver.next() => event,
            result = &mut shutdown, if !shutting_down => {
                result?;
                shutting_down = true;
                tracing::info!("disconnecting");
                driver.submit(Command::Disconnect)?;
                continue;
            }
        };
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "`Event` is `#[non_exhaustive]`; the example logs every event it does not act on"
        )]
        let message = match event {
            Event::Connected => {
                tracing::info!(topic = %cli.topic, "connected, subscribing");
                driver.submit(cli.subscribe())?;
                if let Some(publish) = cli.publish() {
                    driver.submit(publish)?;
                }
                continue;
            }
            Event::Message(message) => message,
            Event::MessageRequiresAcknowledgement(id, message) => {
                driver.submit(Command::Acknowledge(id))?;
                message
            }
            Event::Disconnected(reason) => {
                tracing::info!(?reason, "disconnected");
                break;
            }
            event => {
                tracing::debug!(?event, "received an event but was not handled");
                continue;
            }
        };
        log_message(message);
        if let Some(left) = &mut remaining {
            *left = left.saturating_sub(1);
            if *left == 0 && !shutting_down {
                shutting_down = true;
                tracing::info!("received every requested message, disconnecting");
                driver.submit(Command::Disconnect)?;
            }
        }
    }
    Ok(())
}

fn log_message(message: Message) {
    let Message {
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
    } = message;
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
