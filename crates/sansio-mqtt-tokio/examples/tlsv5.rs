//! Connects over TLS, publishes to a topic it is subscribed to and prints
//! what it receives. Uses `test.mosquitto.org:8886` (publicly trusted
//! certificate) unless `MQTT_TLS_HOST` / `MQTT_TLS_PORT` say otherwise;
//! `MQTT_TLS_CA` names a PEM file of certificates to trust instead of the
//! public roots.

use std::sync::Arc;

use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_tokio::TcpConnector;
use sansio_mqtt_tokio::TlsConnector;
use sansio_mqtt_tokio::rustls::ClientConfig;
use sansio_mqtt_tokio::rustls::RootCertStore;
use sansio_mqtt_tokio::rustls::pki_types::CertificateDer;
use sansio_mqtt_tokio::rustls::pki_types::ServerName;
use sansio_mqtt_tokio::rustls::pki_types::pem::PemObject;
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

    let host = std::env::var("MQTT_TLS_HOST").unwrap_or_else(|_| "test.mosquitto.org".to_owned());
    let port: u16 = std::env::var("MQTT_TLS_PORT").map_or(Ok(8886), |port| port.parse())?;

    // Trust the certificates in the PEM file `MQTT_TLS_CA` names, or the
    // usual public roots.
    let mut roots = RootCertStore::empty();
    match std::env::var("MQTT_TLS_CA") {
        Ok(path) => {
            for certificate in CertificateDer::pem_file_iter(path)? {
                roots.add(certificate?)?;
            }
        }
        Err(_) => roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned()),
    }
    let tls = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::new(
        TcpConnector::new((host.clone(), port)),
        Arc::new(tls),
        ServerName::try_from(host)?,
    );
    let mut driver = Driver::new(Client::default(), connector);

    tracing::info!("connecting to broker");
    driver.submit(Command::Connect(
        ConnectOptions::builder()
            .client_id("sansio-mqtt-tls-example")
            .build(),
    ))?;

    loop {
        match driver.next().await {
            Event::Connected => {
                tracing::info!("connected over TLS");
                driver.submit(Command::Subscribe(
                    SubscribeOptions::builder()
                        .subscriptions(vec![
                            Subscription::builder().filter("sansio-mqtt/tls").build(),
                        ])
                        .build(),
                ))?;
                driver.submit(Command::Publish {
                    token: 1,
                    message: Message::builder()
                        .topic("sansio-mqtt/tls")
                        .payload("Hello over TLS!")
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
