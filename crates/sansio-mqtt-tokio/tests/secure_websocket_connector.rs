//! MQTT over a secure WebSocket (`wss://`, MQTT v5.0 §6): a
//! [`WebSocketConnector`] over a [`TlsConnector`] over a [`TcpConnector`],
//! against a loopback server with a throwaway certificate for `localhost`.

use std::sync::Arc;
use std::time::Duration;

use futures_util::SinkExt;
use futures_util::StreamExt;
use sansio_mqtt_protocol::ByteString;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_tokio::TcpConnector;
use sansio_mqtt_tokio::TlsConnector;
use sansio_mqtt_tokio::WebSocketConnector;
use sansio_mqtt_tokio::rustls::ClientConfig;
use sansio_mqtt_tokio::rustls::RootCertStore;
use sansio_mqtt_tokio::rustls::ServerConfig;
use sansio_mqtt_tokio::rustls::pki_types::ServerName;
use sansio_mqtt_v5_protocol::Client;
use tokio::net::TcpListener;
use tokio::time::Instant;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::ErrorResponse;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL;

const STEP: Duration = Duration::from_secs(2);

/// A successful MQTT v5 CONNACK: no session, reason 0x00, no properties.
const CONNACK: [u8; 5] = [0x20, 0x03, 0x00, 0x00, 0x00];

/// Selects the `mqtt` subprotocol the client must request [MQTT-6.0.0-3].
#[allow(
    clippy::result_large_err,
    reason = "tungstenite dictates the callback's error type"
)]
fn select_mqtt(request: &Request, mut response: Response) -> Result<Response, ErrorResponse> {
    assert_eq!(
        request.headers().get(SEC_WEBSOCKET_PROTOCOL),
        Some(&HeaderValue::from_static("mqtt"))
    );
    response
        .headers_mut()
        .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("mqtt"));
    Ok(response)
}

#[tokio::test]
async fn secure_websocket_carries_mqtt_through_tls() {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certified.cert.der().clone()],
                certified.signing_key.serialize_der().try_into().unwrap(),
            )
            .unwrap(),
    ));
    let mut roots = RootCertStore::empty();
    roots.add(certified.cert.der().clone()).unwrap();
    let tls = tokio_rustls::TlsConnector::from(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let connector = WebSocketConnector::new(
        TlsConnector::new(
            TcpConnector::new(listener.local_addr().unwrap()),
            tls,
            ServerName::try_from("localhost").unwrap(),
        ),
        format!("wss://localhost:{port}/mqtt").parse().unwrap(),
    )
    .unwrap();
    let mut driver = Driver::new(Client::<Instant>::default(), connector);
    driver
        .submit(Command::Connect(
            ConnectOptions::builder()
                .client_id(ByteString::from_static("secure-websocket-test"))
                .build(),
        ))
        .unwrap();

    let broker = async {
        let (tcp, _) = listener.accept().await.unwrap();
        let tls = acceptor.accept(tcp).await.expect("TLS handshake failed");
        let mut broker = accept_hdr_async(tls, select_mqtt)
            .await
            .expect("WebSocket handshake failed");
        match broker.next().await.unwrap().unwrap() {
            Message::Binary(data) => assert_eq!(data[0], 0x10, "expected a CONNECT"),
            other => panic!("expected a binary CONNECT, got {other:?}"),
        }
        broker
            .send(Message::binary(CONNACK.to_vec()))
            .await
            .unwrap();
        std::future::pending::<()>().await;
    };
    tokio::select! {
        () = broker => unreachable!(),
        event = timeout(STEP, driver.next()) => {
            assert_eq!(event.expect("no event within the timeout"), Event::Connected);
        }
    }
}
