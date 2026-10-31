//! Driver behaviour when the protocol rejects driver input, checked with a
//! scripted protocol instead of the real v5 client.

use std::collections::VecDeque;
use std::time::Duration;

use sansio::Protocol;
use sansio_mqtt_protocol::ByteString;
use sansio_mqtt_protocol::Bytes;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::MqttProtocol;
use sansio_mqtt_protocol::Timestamped;
use sansio_mqtt_tokio::Driver;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::io::duplex;
use tokio::time::Instant;
use tokio::time::timeout;

/// Asks for a socket on `Connect`, then rejects the `SocketConnected` that
/// answers it.
#[derive(Debug, Default)]
struct RejectsSocketConnected {
    actions: VecDeque<DriverAction>,
}

impl<'bytes>
    Protocol<
        IncomingData<'bytes, Instant>,
        Timestamped<Command, Instant>,
        Timestamped<DriverEvent, Instant>,
    > for RejectsSocketConnected
{
    type Rout = Event;
    type Wout = Bytes;
    type Eout = DriverAction;
    type Error = Error;
    type Time = Instant;

    fn handle_read(&mut self, _msg: IncomingData<'bytes, Instant>) -> Result<(), Error> {
        Err(Error::InvalidStateTransition)
    }

    fn handle_write(&mut self, msg: Timestamped<Command, Instant>) -> Result<(), Error> {
        match msg.value {
            Command::Connect(_) => {
                self.actions.push_back(DriverAction::OpenSocket);
                Ok(())
            }
            _ => Err(Error::InvalidStateTransition),
        }
    }

    fn handle_event(&mut self, _evt: Timestamped<DriverEvent, Instant>) -> Result<(), Error> {
        Err(Error::InvalidStateTransition)
    }

    fn handle_timeout(&mut self, _now: Instant) -> Result<(), Error> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn poll_read(&mut self) -> Option<Event> {
        None
    }

    fn poll_write(&mut self) -> Option<Bytes> {
        None
    }

    fn poll_event(&mut self) -> Option<DriverAction> {
        self.actions.pop_front()
    }

    fn poll_timeout(&mut self) -> Option<Instant> {
        None
    }
}

impl MqttProtocol<'_, Instant> for RejectsSocketConnected {
    fn version(&self) -> semver::Version {
        semver::Version::new(0, 0, 0)
    }
}

#[tokio::test]
async fn rejected_socket_connected_closes_the_socket() {
    let (client_side, mut broker) = duplex(1024);
    let mut socket = Some(BufReader::new(client_side));
    let connector =
        move || std::future::ready(Ok(socket.take().expect("driver connected more than once")));
    let mut driver = Driver::new(RejectsSocketConnected::default(), connector);
    driver
        .submit(Command::Connect(
            ConnectOptions::builder()
                .client_id(ByteString::from_static("rejected"))
                .build(),
        ))
        .unwrap();

    // The protocol never produces an event, so `next` only drives I/O.
    let mut rest = Vec::new();
    tokio::select! {
        event = driver.next() => panic!("unexpected event {event:?}"),
        read = timeout(Duration::from_secs(2), broker.read_to_end(&mut rest)) => {
            read.expect("driver kept the rejected socket open").unwrap();
        }
    }
    assert!(rest.is_empty(), "unexpected bytes: {rest:?}");
}
