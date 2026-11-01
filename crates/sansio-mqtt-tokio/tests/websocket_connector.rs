//! [`WebSocketConnector`] against a WebSocket server on the loopback
//! interface (MQTT v5.0 §6).

use std::time::Duration;

use futures_util::SinkExt;
use futures_util::StreamExt;
use sansio_mqtt_protocol::ByteString;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_tokio::TcpConnector;
use sansio_mqtt_tokio::WebSocketConnector;
use sansio_mqtt_v5_protocol::Client;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::error::UrlError;
use tokio_tungstenite::tungstenite::handshake::server::ErrorResponse;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL;

const STEP: Duration = Duration::from_secs(2);

/// A successful MQTT v5 CONNACK: no session, reason 0x00, no properties.
const CONNACK: [u8; 5] = [0x20, 0x03, 0x00, 0x00, 0x00];

fn connect_command() -> Command {
    Command::Connect(
        ConnectOptions::builder()
            .client_id(ByteString::from_static("websocket-connector-test"))
            .build(),
    )
}

/// Checks the client asked for the `mqtt` subprotocol [MQTT-6.0.0-3] and
/// selects it.
#[allow(
    clippy::result_large_err,
    reason = "tungstenite dictates the callback's error type"
)]
fn select_mqtt(request: &Request, mut response: Response) -> Result<Response, ErrorResponse> {
    let protocols = request
        .headers()
        .get(SEC_WEBSOCKET_PROTOCOL)
        .expect("client must request a WebSocket subprotocol");
    assert_eq!(protocols, "mqtt");
    response
        .headers_mut()
        .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("mqtt"));
    Ok(response)
}

/// Accepts the next connection, completes the WebSocket handshake and
/// consumes the client's CONNECT.
async fn accept_mqtt(listener: &TcpListener) -> WebSocketStream<TcpStream> {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut broker = accept_hdr_async(tcp, select_mqtt)
        .await
        .expect("WebSocket handshake failed");
    match broker.next().await.unwrap().unwrap() {
        // MQTT Control Packets travel in binary messages [MQTT-6.0.0-1];
        // this one is a CONNECT.
        Message::Binary(data) => assert_eq!(data[0], 0x10),
        other => panic!("expected a binary CONNECT, got {other:?}"),
    }
    broker
}

fn websocket_connector(
    listener: &TcpListener,
) -> WebSocketConnector<TcpConnector<std::net::SocketAddr>> {
    let address = listener.local_addr().unwrap();
    let uri = format!("ws://{address}/mqtt").parse().unwrap();
    WebSocketConnector::new(TcpConnector::new(address), uri).unwrap()
}

/// Connects a driver while `broker` plays the server side, and returns the
/// first `count` events the application observes.
async fn events<Broker>(count: usize, broker: impl FnOnce(TcpListener) -> Broker) -> Vec<Event>
where
    Broker: Future<Output = ()>,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut driver = Driver::new(Client::<Instant>::default(), websocket_connector(&listener));
    driver.submit(connect_command()).unwrap();
    let broker = broker(listener);
    let observed = async {
        let mut observed = Vec::new();
        for _ in 0..count {
            observed.push(driver.next().await);
        }
        observed
    };
    tokio::select! {
        () = broker => panic!("the broker finished before the client reported its events"),
        observed = timeout(STEP, observed) => observed.expect("no events within the timeout"),
    }
}

#[tokio::test]
async fn websocket_connector_carries_mqtt_in_binary_messages() {
    let observed = events(1, |listener| async move {
        let mut broker = accept_mqtt(&listener).await;
        broker
            .send(Message::binary(CONNACK.to_vec()))
            .await
            .unwrap();
        std::future::pending().await
    })
    .await;

    assert_eq!(observed, [Event::Connected]);
}

#[tokio::test]
async fn websocket_connector_reassembles_packets_split_across_messages() {
    // MQTT packets need not be aligned on WebSocket message boundaries
    // [MQTT-6.0.0-2], and an empty message is not the end of the stream.
    let observed = events(1, |listener| async move {
        let mut broker = accept_mqtt(&listener).await;
        broker
            .send(Message::binary(CONNACK[..2].to_vec()))
            .await
            .unwrap();
        broker.send(Message::binary(Vec::new())).await.unwrap();
        broker
            .send(Message::binary(CONNACK[2..].to_vec()))
            .await
            .unwrap();
        std::future::pending().await
    })
    .await;

    assert_eq!(observed, [Event::Connected]);
}

#[tokio::test]
async fn websocket_connector_ignores_pings_between_mqtt_packets() {
    // A control frame with an empty payload must neither reach the MQTT byte
    // stream nor look like the end of the connection.
    let observed = events(1, |listener| async move {
        let mut broker = accept_mqtt(&listener).await;
        broker.send(Message::Ping(Vec::new().into())).await.unwrap();
        broker
            .send(Message::binary(CONNACK.to_vec()))
            .await
            .unwrap();
        std::future::pending().await
    })
    .await;

    assert_eq!(observed, [Event::Connected]);
}

#[tokio::test]
async fn websocket_connector_text_message_closes_the_connection() {
    // Any data frame other than binary closes the Network Connection
    // [MQTT-6.0.0-1].
    let observed = events(2, |listener| async move {
        let mut broker = accept_mqtt(&listener).await;
        broker
            .send(Message::binary(CONNACK.to_vec()))
            .await
            .unwrap();
        broker.send(Message::text("not mqtt")).await.unwrap();
        std::future::pending().await
    })
    .await;

    assert_eq!(observed, [Event::Connected, Event::Disconnected(None)]);
}

#[tokio::test]
async fn websocket_connector_close_message_reports_disconnected() {
    let observed = events(2, |listener| async move {
        let mut broker = accept_mqtt(&listener).await;
        broker
            .send(Message::binary(CONNACK.to_vec()))
            .await
            .unwrap();
        broker.close(None).await.unwrap();
        std::future::pending().await
    })
    .await;

    assert_eq!(observed, [Event::Connected, Event::Disconnected(None)]);
}

#[tokio::test]
async fn websocket_connector_failed_handshake_reports_disconnected() {
    let observed = events(1, |listener| async move {
        let (mut tcp, _) = listener.accept().await.unwrap();
        tcp.write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n")
            .await
            .unwrap();
        std::future::pending().await
    })
    .await;

    assert_eq!(observed, [Event::Disconnected(None)]);
}

#[test]
fn websocket_connector_rejects_a_uri_it_cannot_connect_to() {
    let uri = "http://broker.example/mqtt".parse().unwrap();
    assert!(matches!(
        WebSocketConnector::new(TcpConnector::new("broker.example:80"), uri),
        Err(UrlError::UnsupportedUrlScheme)
    ));
}

#[test]
fn websocket_connector_rejects_a_uri_without_a_host() {
    let uri = "ws://:8080/mqtt".parse().unwrap();
    assert!(matches!(
        WebSocketConnector::new(TcpConnector::new("127.0.0.1:8080"), uri),
        Err(UrlError::EmptyHostName)
    ));
}
