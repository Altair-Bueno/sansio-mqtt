//! Connects over a WebSocket (MQTT v5.0 §6), publishes to a topic it is
//! subscribed to and prints what it receives. Uses
//! `ws://test.mosquitto.org:8080/mqtt` unless `MQTT_WS_URI` says otherwise; a
//! `wss://` URI runs the WebSocket over TLS, verifying the broker with the
//! operating system's verifier or the PEM file `MQTT_TLS_CA` names.

extern crate alloc;

use alloc::sync::Arc;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;

use rustls_platform_verifier::BuilderVerifierExt as _;
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
use sansio_mqtt_v5_protocol::Client;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn core::error::Error + Send + Sync>> {
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
    let tcp_port = |default| uri.port_u16().unwrap_or(default);

    // `ws://` runs over plain TCP, `wss://` over TLS: the connectors stack.
    if uri.scheme_str() == Some("wss") {
        let tcp = TcpConnector::new((host.clone(), tcp_port(443)));
        let tls = TlsConnector::new(tcp, tls_context()?, ServerName::try_from(host)?);
        run(WebSocketConnector::new(tls, uri)?).await
    } else {
        let tcp = TcpConnector::new((host, tcp_port(80)));
        run(WebSocketConnector::new(tcp, uri)?).await
    }
}

/// Verifies the broker with the operating system's certificate verifier, or
/// trusts only the PEM file `MQTT_TLS_CA` names.
fn tls_context() -> Result<tokio_rustls::TlsConnector, Box<dyn core::error::Error + Send + Sync>> {
    let config = match std::env::var("MQTT_TLS_CA") {
        Ok(path) => {
            let mut roots = RootCertStore::empty();
            for certificate in CertificateDer::pem_file_iter(path)? {
                roots.add(certificate?)?;
            }
            ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth()
        }
        Err(_) => ClientConfig::builder()
            .with_platform_verifier()?
            .with_no_client_auth(),
    };
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

async fn run<C>(connector: C) -> Result<(), Box<dyn core::error::Error + Send + Sync>>
where
    C: Connector,
{
    let mut driver = Driver::new(Client::default(), connector);

    tracing::info!("connecting to broker");
    driver.submit(Command::Connect(
        ConnectOptions::builder()
            .client_id("sansio-mqtt-ws-example")
            .build(),
    ))?;

    loop {
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "`Event` is `#[non_exhaustive]`; the example logs every event it does not act on"
        )]
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
