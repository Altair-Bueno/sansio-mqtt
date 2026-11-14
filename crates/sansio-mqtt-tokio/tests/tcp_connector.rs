//! [`TcpConnector`] against a real TCP listener on the loopback interface.

use core::net::SocketAddr;
use core::time::Duration;

use sansio_mqtt_protocol::ByteString;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_tokio::Driver;
use sansio_mqtt_tokio::DynConnector;
use sansio_mqtt_tokio::DynDriver;
use sansio_mqtt_tokio::DynProtocol;
use sansio_mqtt_tokio::connector::TcpConnector;
use sansio_mqtt_v5_protocol::Client;
use tokio::io::AsyncReadExt as _;
use tokio::net::TcpListener;
use tokio::time::Instant;
use tokio::time::timeout;

const STEP: Duration = Duration::from_secs(2);

fn connect_command() -> Command {
    Command::Connect(
        ConnectOptions::builder()
            .client_id(ByteString::from_static("tcp-connector-test"))
            .build(),
    )
}

#[tokio::test]
async fn tcp_connector_delivers_connect_to_the_broker() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let mut driver = Driver::new(Client::<Instant>::default(), TcpConnector::new(address));
    driver.submit(connect_command()).unwrap();

    let first_byte = async {
        let (mut broker, _) = listener.accept().await.unwrap();
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
async fn tcp_connector_refused_connection_reports_disconnected() {
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let mut driver = Driver::new(Client::<Instant>::default(), TcpConnector::new(address));
    driver.submit(connect_command()).unwrap();

    let event = timeout(STEP, driver.next())
        .await
        .expect("no event within the timeout");
    assert_eq!(event, Event::Disconnected(None));
}

/// A driver stored in a field, whose type has to be written out.
struct Application {
    driver: Driver<Client<Instant>, TcpConnector<SocketAddr>>,
}

#[tokio::test]
async fn driver_type_is_nameable_without_the_attempt_type() {
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let mut application = Application {
        driver: Driver::new(Client::<Instant>::default(), TcpConnector::new(address)),
    };
    application.driver.submit(connect_command()).unwrap();

    let event = timeout(STEP, application.driver.next())
        .await
        .expect("no event within the timeout");
    assert_eq!(event, Event::Disconnected(None));
}

#[tokio::test]
async fn dyn_connector_erases_a_connector() {
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let mut driver = DynDriver::new(
        DynProtocol::new(Client::<Instant>::default()),
        DynConnector::new(TcpConnector::new(address)),
    );
    driver.submit(connect_command()).unwrap();

    let event = timeout(STEP, driver.next())
        .await
        .expect("no event within the timeout");
    assert_eq!(event, Event::Disconnected(None));
}
