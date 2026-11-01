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

extern crate alloc;

use alloc::sync::Arc;
use core::num::NonZero;
use core::str::FromStr;
use std::path::PathBuf;

use rustls_platform_verifier::BuilderVerifierExt as _;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::Qos;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;
use sansio_mqtt_tokio::Connector;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_tokio::TcpConnector;
use sansio_mqtt_tokio::TlsConnector;
use sansio_mqtt_tokio::WebSocketConnector;
use sansio_mqtt_tokio::rustls::ClientConfig;
use sansio_mqtt_tokio::rustls::RootCertStore;
use sansio_mqtt_tokio::rustls::pki_types::CertificateDer;
use sansio_mqtt_tokio::rustls::pki_types::ServerName;
use sansio_mqtt_tokio::rustls::pki_types::pem::PemObject as _;
use sansio_mqtt_tokio::tokio_rustls;
use sansio_mqtt_tokio::tungstenite::http::Uri;
use sansio_mqtt_tokio::tungstenite::http::uri::InvalidUri;
use sansio_mqtt_v5_protocol::Client;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

type BoxError = Box<dyn core::error::Error + Send + Sync>;

/// Connects to an MQTT v5.0 broker, subscribes to a topic, optionally
/// publishes a message to it and logs every message it receives until Ctrl-C.
///
/// The broker URL's scheme selects the transport: `mqtt://` for plain TCP
/// (port 1883 by default), `mqtts://` for TLS (8883), `ws://` for a
/// WebSocket (80) and `wss://` for a WebSocket over TLS (443). A URL without
/// a scheme is plain TCP. Set `RUST_LOG` to change the log level.
#[derive(Debug, usage_rs::Cli)]
#[usage(bin = "client")]
struct Cli {
    /// The broker to connect to
    #[usage(env = "MQTT_BROKER", default = "mqtt://test.mosquitto.org:1883")]
    url: Broker,

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

/// How the bytes reach the broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    Tcp,
    Tls,
    WebSocket,
    SecureWebSocket,
}

impl Transport {
    const fn default_port(self) -> u16 {
        match self {
            Self::Tcp => 1883,
            Self::Tls => 8883,
            Self::WebSocket => 80,
            Self::SecureWebSocket => 443,
        }
    }

    const fn is_tls(self) -> bool {
        matches!(self, Self::Tls | Self::SecureWebSocket)
    }
}

/// A broker URL, already checked to name a supported transport and a host.
#[derive(Debug)]
struct Broker {
    transport: Transport,
    host: String,
    port: u16,
    uri: Uri,
}

#[derive(Debug, thiserror::Error)]
enum BrokerUrlError {
    #[error("the broker URL is invalid: {0}")]
    Invalid(#[from] InvalidUri),
    #[error("the broker URL scheme `{0}` is not one of mqtt, mqtts, ws or wss")]
    UnsupportedScheme(String),
    #[error("the broker URL has no host")]
    MissingHost,
}

impl FromStr for Broker {
    type Err = BrokerUrlError;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        let uri: Uri = url.parse()?;
        let transport = match uri.scheme_str() {
            None | Some("mqtt" | "tcp") => Transport::Tcp,
            Some("mqtts" | "ssl" | "tls") => Transport::Tls,
            Some("ws") => Transport::WebSocket,
            Some("wss") => Transport::SecureWebSocket,
            Some(scheme) => return Err(BrokerUrlError::UnsupportedScheme(scheme.to_owned())),
        };
        let host = uri.host().ok_or(BrokerUrlError::MissingHost)?.to_owned();
        let port = uri.port_u16().unwrap_or(transport.default_port());
        Ok(Self {
            transport,
            host,
            port,
            uri,
        })
    }
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

    let Broker {
        transport,
        ref host,
        port,
        ref uri,
    } = cli.url;
    if cli.ca.is_some() && !transport.is_tls() {
        tracing::warn!("ignoring --ca: the broker URL does not use TLS");
    }
    let tcp = TcpConnector::new((host.clone(), port));
    match transport {
        Transport::Tcp => run(&cli, tcp).await,
        Transport::Tls => {
            let tls =
                TlsConnector::new(tcp, tls_context(&cli)?, ServerName::try_from(host.clone())?);
            run(&cli, tls).await
        }
        Transport::WebSocket => run(&cli, WebSocketConnector::new(tcp, uri.clone())?).await,
        Transport::SecureWebSocket => {
            let tls =
                TlsConnector::new(tcp, tls_context(&cli)?, ServerName::try_from(host.clone())?);
            run(&cli, WebSocketConnector::new(tls, uri.clone())?).await
        }
    }
}

/// Verifies the broker with the operating system's certificate verifier, like
/// a browser would, or trusts only the certificates in `--ca`.
fn tls_context(cli: &Cli) -> Result<tokio_rustls::TlsConnector, BoxError> {
    let config = match &cli.ca {
        Some(path) => {
            let mut roots = RootCertStore::empty();
            for certificate in CertificateDer::pem_file_iter(path)? {
                roots.add(certificate?)?;
            }
            ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth()
        }
        None => ClientConfig::builder()
            .with_platform_verifier()?
            .with_no_client_auth(),
    };
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
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

async fn run<C>(cli: &Cli, connector: C) -> Result<(), BoxError>
where
    C: Connector,
{
    let mut driver = Driver::new(Client::default(), connector);
    tracing::info!(url = %cli.url.uri, "connecting to broker");
    driver.submit(cli.connect())?;

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
