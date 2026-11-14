//! [`connect`]: a driver built from a broker URL, against loopback brokers.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail the test on setup errors by design"
)]

extern crate alloc;

use alloc::sync::Arc;
use core::time::Duration;
use std::io::Write as _;

use futures_util::SinkExt as _;
use futures_util::StreamExt as _;
use sansio_mqtt_protocol::ByteString;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Connected;
use sansio_mqtt_protocol::Disconnected;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_tokio::BrokerUrlError;
use sansio_mqtt_tokio::DynDriver;
use sansio_mqtt_tokio::connect;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt as _;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt as _;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::crypto::CryptoProvider;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::ErrorResponse;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL;
use url::Url;

const STEP: Duration = Duration::from_secs(2);

/// A successful MQTT v5 CONNACK: no session, reason 0x00, no properties.
const CONNACK: [u8; 5] = [0x20, 0x03, 0x00, 0x00, 0x00];

fn connect_command() -> Command {
    Command::Connect(
        ConnectOptions::builder()
            .client_id(ByteString::from_static("connect-test"))
            .build(),
    )
}

fn parse(url: &str) -> Url {
    Url::parse(url).unwrap()
}

/// Installs the process-wide rustls crypto provider `connect` uses for TLS.
fn install_crypto_provider() {
    if CryptoProvider::get_default().is_none() {
        // Another test may win the race; either way one is installed.
        drop(tokio_rustls::rustls::crypto::ring::default_provider().install_default());
    }
}

/// A TLS server for `localhost` and a PEM file with the certificate a client
/// must trust.
fn localhost_server() -> (TlsAcceptor, tempfile::NamedTempFile) {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certified.cert.der().clone()],
            certified.signing_key.serialize_der().try_into().unwrap(),
        )
        .unwrap();
    let mut ca = tempfile::NamedTempFile::new().unwrap();
    ca.write_all(certified.cert.pem().as_bytes()).unwrap();
    (TlsAcceptor::from(Arc::new(config)), ca)
}

/// `base` with `?ca=<path>`, percent-encoded.
fn with_ca(base: &str, ca: &tempfile::NamedTempFile) -> Url {
    let mut url = parse(base);
    url.query_pairs_mut()
        .append_pair("ca", ca.path().to_str().unwrap());
    url
}

/// Reads one whole MQTT packet whose Remaining Length fits in one byte.
async fn read_small_packet(socket: &mut (impl AsyncRead + Unpin)) -> Vec<u8> {
    let header = socket.read_u8().await.unwrap();
    let remaining = socket.read_u8().await.unwrap();
    assert!(remaining < 0x80, "packet too large for this helper");
    let mut packet = vec![header, remaining];
    packet.resize(2 + usize::from(remaining), 0);
    socket.read_exact(&mut packet[2..]).await.unwrap();
    packet
}

/// Plays a broker on `socket`: reads CONNECT, answers CONNACK, then idles.
async fn accept_connect(mut socket: impl AsyncRead + AsyncWrite + Unpin) -> Vec<u8> {
    let connect = read_small_packet(&mut socket).await;
    socket.write_all(&CONNACK).await.unwrap();
    socket.flush().await.unwrap();
    // Keep the connection open until the test ends.
    core::future::pending::<()>().await;
    connect
}

/// Submits CONNECT and returns the first event while `broker` runs.
async fn first_event(driver: &mut DynDriver, broker: impl Future<Output = Vec<u8>>) -> Event {
    driver.submit(connect_command()).unwrap();
    tokio::select! {
        _ = broker => panic!("the broker finished before the client reported an event"),
        event = timeout(STEP, driver.next()) => event.expect("no event within the timeout"),
    }
}

#[tokio::test]
async fn connect_mqtt_url_connects_over_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut driver = connect(&parse(&format!("mqtt://127.0.0.1:{port}"))).unwrap();

    let broker = async { accept_connect(listener.accept().await.unwrap().0).await };
    assert_eq!(
        first_event(&mut driver, broker).await,
        Event::Connected(Connected::builder().session_present(false).build())
    );
}

#[tokio::test]
async fn connect_mqtt_url_accepts_a_domain_and_an_ipv6_literal() {
    for host in ["localhost", "[::1]"] {
        let Ok(listener) = TcpListener::bind((host.trim_matches(['[', ']']), 0)).await else {
            continue;
        };
        let port = listener.local_addr().unwrap().port();
        let mut driver = connect(&parse(&format!("mqtt://{host}:{port}/"))).unwrap();

        let broker = async { accept_connect(listener.accept().await.unwrap().0).await };
        assert_eq!(
            first_event(&mut driver, broker).await,
            Event::Connected(Connected::builder().session_present(false).build()),
            "{host}"
        );
    }
}

/// The CONNECT a driver built from `mqtt://<loopback>?<query>` sends.
async fn connect_packet(query: &str) -> Vec<u8> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut driver = connect(&parse(&format!("mqtt://127.0.0.1:{port}?{query}"))).unwrap();
    driver.submit(connect_command()).unwrap();

    let connect = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_small_packet(&mut socket).await
    };
    tokio::select! {
        event = driver.next() => panic!("unexpected event {event:?}"),
        connect = timeout(STEP, connect) => connect.expect("no CONNECT within the timeout"),
    }
}

#[tokio::test]
async fn connect_keep_alive_parameter_reaches_the_connect_packet() {
    let connect = connect_packet("keep_alive=30&receive_maximum=7").await;
    // Fixed header (2 bytes), Protocol Name (6), Protocol Version (1) and
    // Connect Flags (1) precede the Keep Alive [MQTT-3.1.2-20].
    assert_eq!(connect[8], 5, "protocol version");
    assert_eq!(connect[10..12], [0, 30], "keep alive");
    // Receive Maximum (0x21) is the first property [MQTT-3.1.2-23].
    assert_eq!(connect[13..16], [0x21, 0, 7], "receive maximum");
}

#[tokio::test]
async fn connect_keep_alive_zero_turns_keep_alive_off() {
    // A Keep Alive of 0 turns the mechanism off (MQTT v5.0 §3.1.2.10).
    let connect = connect_packet("keep_alive=0").await;
    assert_eq!(connect[10..12], [0, 0], "keep alive");
}

#[test]
fn connect_accepts_every_qos_as_its_number() {
    for qos in 0..=2 {
        connect(&parse(&format!(
            "mqtt://broker.example?max_outgoing_qos={qos}"
        )))
        .unwrap();
    }
}

#[tokio::test]
async fn connect_connect_timeout_parameter_gives_up_on_a_silent_broker() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut driver = connect(&parse(&format!(
        "mqtt://127.0.0.1:{port}?connect_timeout=1"
    )))
    .unwrap();
    driver.submit(connect_command()).unwrap();

    let broker = async {
        // Accept, but never answer the CONNECT.
        let _socket = listener.accept().await.unwrap();
        core::future::pending::<()>().await;
    };
    let event: Event = tokio::select! {
        () = broker => unreachable!(),
        event = timeout(STEP, driver.next()) => event.expect("the connect timeout did not fire"),
    };
    assert_eq!(event, Event::Disconnected(Disconnected::builder().build()));
}

#[tokio::test]
async fn connect_accepts_every_client_setting() {
    install_crypto_provider();
    let url = parse(
        "mqtt://127.0.0.1:1883?version=5&receive_maximum=10&maximum_packet_size=4096\
         &topic_alias_maximum=5&request_response_information=true\
         &request_problem_information=false&keep_alive=0&max_outgoing_qos=1\
         &allow_retain=false&allow_wildcard_subscriptions=false\
         &allow_shared_subscriptions=false&allow_subscription_identifiers=false\
         &max_user_properties=4&max_subscription_identifiers=4&max_subscriptions=4\
         &connect_timeout=5",
    );
    connect(&url).unwrap();
}

#[tokio::test]
async fn connect_mqtts_url_connects_over_tls() {
    install_crypto_provider();
    let (acceptor, ca) = localhost_server();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut driver = connect(&with_ca(&format!("mqtts://localhost:{port}"), &ca)).unwrap();

    let broker = async {
        let (tcp, _) = listener.accept().await.unwrap();
        accept_connect(acceptor.accept(tcp).await.unwrap()).await
    };
    assert_eq!(
        first_event(&mut driver, broker).await,
        Event::Connected(Connected::builder().session_present(false).build())
    );
}

#[tokio::test]
async fn connect_mqtts_url_without_ca_uses_the_platform_verifier() {
    install_crypto_provider();
    connect(&parse("mqtts://broker.example")).unwrap();
}

/// Checks the client asked for the `mqtt` subprotocol on `/mqtt` and selects
/// it.
#[expect(
    clippy::result_large_err,
    clippy::unnecessary_wraps,
    reason = "tungstenite's handshake `Callback` dictates this signature and its error type"
)]
fn select_mqtt(request: &Request, mut response: Response) -> Result<Response, ErrorResponse> {
    assert_eq!(request.uri().path(), "/mqtt");
    assert_eq!(
        request.uri().query(),
        None,
        "connect parameters must not reach the broker"
    );
    assert_eq!(
        request.headers().get(SEC_WEBSOCKET_PROTOCOL),
        Some(&HeaderValue::from_static("mqtt"))
    );
    response
        .headers_mut()
        .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("mqtt"));
    Ok(response)
}

/// Plays a WebSocket broker on `socket`: checks the CONNECT, answers CONNACK.
async fn accept_websocket_connect(socket: impl AsyncRead + AsyncWrite + Unpin) -> Vec<u8> {
    let mut broker = accept_hdr_async(socket, select_mqtt).await.unwrap();
    let connect = broker.next().await.unwrap().unwrap().into_data().to_vec();
    broker
        .send(Message::binary(CONNACK.to_vec()))
        .await
        .unwrap();
    core::future::pending::<()>().await;
    connect
}

#[tokio::test]
async fn connect_ws_url_connects_over_websocket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut driver = connect(&parse(&format!("ws://127.0.0.1:{port}/mqtt?keep_alive=30"))).unwrap();

    let broker = async { accept_websocket_connect(listener.accept().await.unwrap().0).await };
    assert_eq!(
        first_event(&mut driver, broker).await,
        Event::Connected(Connected::builder().session_present(false).build())
    );
}

#[tokio::test]
async fn connect_wss_url_connects_over_websocket_over_tls() {
    install_crypto_provider();
    let (acceptor, ca) = localhost_server();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut driver = connect(&with_ca(&format!("wss://localhost:{port}/mqtt"), &ca)).unwrap();

    let broker = async {
        let (tcp, _) = listener.accept().await.unwrap();
        accept_websocket_connect(acceptor.accept(tcp).await.unwrap()).await
    };
    assert_eq!(
        first_event(&mut driver, Box::pin(broker)).await,
        Event::Connected(Connected::builder().session_present(false).build())
    );
}

#[tokio::test]
async fn connect_driver_can_be_spawned() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut driver = connect(&parse(&format!("mqtt://127.0.0.1:{port}"))).unwrap();
    driver.submit(connect_command()).unwrap();

    let client = tokio::spawn(async move { driver.next().await });
    let broker = async { accept_connect(listener.accept().await.unwrap().0).await };
    let event: Event = tokio::select! {
        _ = broker => unreachable!(),
        event = timeout(STEP, client) => event.expect("no event within the timeout").unwrap(),
    };
    assert_eq!(
        event,
        Event::Connected(Connected::builder().session_present(false).build())
    );
}

#[track_caller]
fn assert_rejected(url_text: &str, expected: impl Fn(&BrokerUrlError) -> bool) {
    install_crypto_provider();
    match connect(&parse(url_text)) {
        Ok(driver) => panic!("{url_text} was accepted: {driver:?}"),
        Err(error) => assert!(expected(&error), "{url_text} was rejected with {error:?}"),
    }
}

#[test]
fn connect_rejects_an_unsupported_scheme() {
    assert_rejected(
        "http://broker.example",
        |error| matches!(error, BrokerUrlError::UnsupportedScheme(scheme) if scheme == "http"),
    );
}

#[test]
fn connect_rejects_a_url_without_a_host() {
    assert_rejected("mqtt:broker", |error| {
        matches!(error, BrokerUrlError::MissingHost)
    });
    // `Url::parse` refuses an empty host, but `Url::set_host` allows one.
    let mut empty_host = parse("mqtt://broker.example");
    empty_host.set_host(Some("")).unwrap();
    assert!(
        matches!(connect(&empty_host), Err(BrokerUrlError::MissingHost)),
        "{empty_host}"
    );
}

#[test]
fn connect_rejects_credentials() {
    assert_rejected("mqtt://user@broker.example", |error| {
        matches!(error, BrokerUrlError::Credentials)
    });
    assert_rejected("mqtt://:secret@broker.example", |error| {
        matches!(error, BrokerUrlError::Credentials)
    });
}

#[test]
fn connect_rejects_a_fragment() {
    assert_rejected("mqtt://broker.example#here", |error| {
        matches!(error, BrokerUrlError::Fragment)
    });
}

#[test]
fn connect_rejects_a_path_on_a_tcp_or_tls_url() {
    assert_rejected(
        "mqtt://broker.example/mqtt",
        |error| matches!(error, BrokerUrlError::Path(path) if path == "/mqtt"),
    );
    assert_rejected(
        "mqtts://broker.example/mqtt",
        |error| matches!(error, BrokerUrlError::Path(path) if path == "/mqtt"),
    );
}

#[test]
fn connect_rejects_an_unknown_parameter() {
    assert_rejected("mqtt://broker.example?keep_alive_secs=30", |error| {
        matches!(
            error,
            BrokerUrlError::InvalidParameter { source, .. }
                if source.to_string().contains("`keep_alive_secs`")
        )
    });
}

#[test]
fn connect_rejects_a_repeated_parameter() {
    assert_rejected(
        "mqtt://broker.example?keep_alive=30&keep_alive=60",
        |error| matches!(error, BrokerUrlError::RepeatedParameter(name) if name == "keep_alive"),
    );
}

#[test]
fn connect_rejects_an_invalid_parameter_value() {
    for (query, key) in [
        ("keep_alive=soon", "keep_alive"),
        ("receive_maximum=0", "receive_maximum"),
        ("maximum_packet_size=0", "maximum_packet_size"),
        ("allow_retain=yes", "allow_retain"),
        ("max_outgoing_qos=3", "max_outgoing_qos"),
        ("connect_timeout=-1", "connect_timeout"),
    ] {
        assert_rejected(
            &format!("mqtt://broker.example?{query}"),
            |error| matches!(error, BrokerUrlError::InvalidParameter { name, .. } if name == key),
        );
    }
}

#[test]
fn connect_rejects_an_unsupported_protocol_version() {
    assert_rejected(
        "mqtt://broker.example?version=4",
        |error| matches!(error, BrokerUrlError::UnsupportedVersion(version) if version == "4"),
    );
}

#[test]
fn connect_rejects_a_ca_on_a_url_without_tls() {
    assert_rejected("mqtt://broker.example?ca=/etc/ca.pem", |error| {
        matches!(error, BrokerUrlError::CaWithoutTls)
    });
}

#[test]
fn connect_rejects_an_unreadable_ca() {
    assert_rejected(
        "mqtts://broker.example?ca=/nonexistent/ca.pem",
        |error| matches!(error, BrokerUrlError::Ca { path, .. } if path.as_os_str() == "/nonexistent/ca.pem"),
    );
}

#[test]
fn connect_rejects_a_ca_file_without_certificates() {
    let empty = tempfile::NamedTempFile::new().unwrap();
    let url = with_ca("mqtts://broker.example", &empty);
    install_crypto_provider();
    assert!(matches!(
        connect(&url),
        Err(BrokerUrlError::NoCertificates(_))
    ));
}

/// A `ca` file holding `pem`.
fn ca_file(pem: &str) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(pem.as_bytes()).unwrap();
    file
}

#[test]
fn connect_rejects_a_ca_file_that_is_not_pem() {
    let ca = ca_file("-----BEGIN CERTIFICATE-----\nAAAA\n");
    install_crypto_provider();
    assert!(matches!(
        connect(&with_ca("mqtts://broker.example", &ca)),
        Err(BrokerUrlError::Ca { .. })
    ));
}

#[test]
fn connect_rejects_a_ca_that_is_not_a_certificate() {
    let ca = ca_file("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n");
    install_crypto_provider();
    assert!(matches!(
        connect(&with_ca("mqtts://broker.example", &ca)),
        Err(BrokerUrlError::Tls(_))
    ));
}

#[test]
fn connect_rejects_a_host_that_is_not_a_tls_server_name() {
    assert_rejected(
        "mqtts://a..b",
        |error| matches!(error, BrokerUrlError::ServerName { host, .. } if host == "a..b"),
    );
}

#[test]
fn connect_ws_url_accepts_any_path_the_url_holds() {
    for path in ["/", "/a^b|c", "/%7Bx%7D", "/ünï"] {
        connect(&parse(&format!("ws://broker.example{path}"))).unwrap();
    }
    connect(&parse("ws://[::1]:8080/mqtt")).unwrap();
}
