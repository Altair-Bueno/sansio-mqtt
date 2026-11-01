//! [`TlsConnector`] against a rustls server on the loopback interface, using
//! a throwaway self-signed certificate for `localhost`.
#![cfg(feature = "rustls")]
#![expect(
    clippy::unwrap_used,
    reason = "test helpers fail the test on setup errors by design"
)]

extern crate alloc;

use alloc::sync::Arc;
use core::time::Duration;

use sansio_mqtt_protocol::ByteString;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_tokio::TcpConnector;
use sansio_mqtt_tokio::TlsConnector;
use sansio_mqtt_tokio::rustls::ClientConfig;
use sansio_mqtt_tokio::rustls::RootCertStore;
use sansio_mqtt_tokio::rustls::ServerConfig;
use sansio_mqtt_tokio::rustls::pki_types::ServerName;
use sansio_mqtt_v5_protocol::Client;
use tokio::io::AsyncReadExt as _;
use tokio::net::TcpListener;
use tokio::time::Instant;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

const STEP: Duration = Duration::from_secs(2);

fn connect_command() -> Command {
    Command::Connect(
        ConnectOptions::builder()
            .client_id(ByteString::from_static("tls-connector-test"))
            .build(),
    )
}

/// A TLS server for `localhost` and the certificate a client must trust.
fn localhost_server() -> (TlsAcceptor, rcgen::CertifiedKey<rcgen::KeyPair>) {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certified.cert.der().clone()],
            certified.signing_key.serialize_der().try_into().unwrap(),
        )
        .unwrap();
    (TlsAcceptor::from(Arc::new(config)), certified)
}

/// A TLS context trusting only `roots`; cloning it shares the configuration.
fn tls_context(roots: RootCertStore) -> tokio_rustls::TlsConnector {
    tokio_rustls::TlsConnector::from(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

#[tokio::test]
async fn tls_connector_delivers_connect_through_the_tls_session() {
    let (acceptor, certified) = localhost_server();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut roots = RootCertStore::empty();
    roots.add(certified.cert.der().clone()).unwrap();
    let connector = TlsConnector::new(
        TcpConnector::new(listener.local_addr().unwrap()),
        tls_context(roots),
        ServerName::try_from("localhost").unwrap(),
    );
    let mut driver = Driver::new(Client::<Instant>::default(), connector);
    driver.submit(connect_command()).unwrap();

    let first_byte = async {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut broker = acceptor.accept(tcp).await.unwrap();
        broker.read_u8().await.unwrap()
    };
    tokio::select! {
        event = driver.next() => panic!("unexpected event {event:?}"),
        first_byte = timeout(STEP, first_byte) => {
            // Fixed header of a CONNECT: packet type 1, flags 0.
            assert_eq!(first_byte.expect("no CONNECT within the timeout"), 0x10);
        }
    }
}

#[tokio::test]
async fn tls_connector_untrusted_certificate_reports_disconnected() {
    let (acceptor, _certified) = localhost_server();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let connector = TlsConnector::new(
        TcpConnector::new(listener.local_addr().unwrap()),
        tls_context(RootCertStore::empty()),
        ServerName::try_from("localhost").unwrap(),
    );
    let mut driver = Driver::new(Client::<Instant>::default(), connector);
    driver.submit(connect_command()).unwrap();

    let server = async {
        let (tcp, _) = listener.accept().await.unwrap();
        // The client aborts the handshake; the server side just fails.
        drop(acceptor.accept(tcp).await);
        core::future::pending::<()>().await;
    };
    tokio::select! {
        () = server => unreachable!(),
        event = timeout(STEP, driver.next()) => {
            assert_eq!(event.expect("no event within the timeout"), Event::Disconnected(None));
        }
    }
}
