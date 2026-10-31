use crate::client::ClientSettings;
use crate::convert;
use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::ClientState;
use crate::state::StateHandler;
use crate::state::connecting::Connecting;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_v5_types::ControlPacket;

/// No live connection and no connection attempt; a `Connect` write can start
/// a new one.
///
/// This is both the initial state and the state reached after every teardown.
/// A `CloseSocket` queued on the way here may still be pending in the action
/// queue.
#[derive(Debug)]
pub(crate) struct Disconnected;

/// Handles a `Command::Connect`.
///
/// Validates the options by building and encoding the CONNECT they describe;
/// on failure the command is rejected and nothing changes. Otherwise stores
/// them, recomputes effective limits, optionally clears session state for a
/// clean start, marks the session persistence flag, enqueues `OpenSocket` and
/// moves to [`Connecting`] awaiting the socket, which keeps the encoded CONNECT
/// until the driver reports `SocketConnected`.
///
/// Arms the connect deadline [`ClientSettings::connect_timeout`] after `at`. It
/// bounds the whole attempt, socket and CONNACK alike, and is replaced or
/// cleared when the attempt ends.
///
/// [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
fn begin_connect<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    options: ConnectOptions,
    at: T,
) -> Result<ClientState, Error>
where
    T: Time,
{
    let connect = convert::connect_options_to_wire(settings, &options)?;
    let connect = queues::encode_control_packet(&ControlPacket::Connect(connect))?;

    let clean_start = options.clean_start;
    let session_should_persist = options
        .session_expiry
        .is_some_and(|interval| !interval.is_zero());

    scratchpad.pending_connect_options = Some(options);
    limits::recompute_effective_limits(settings, scratchpad);
    if clean_start {
        // [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
        *session = ClientSession::default();
    }
    scratchpad.session_should_persist = session_should_persist;
    scratchpad.action_queue.push_back(DriverAction::OpenSocket);
    scratchpad.arm_connect_deadline(at, settings.connect_timeout);

    Ok(ClientState::Connecting(Connecting::awaiting_socket(
        connect,
    )))
}

impl<T> StateHandler<T> for Disconnected
where
    T: Time,
{
    fn handle_control_packet(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<T>,
        _packet: ControlPacket,
        _received_at: T,
    ) -> (ClientState, Result<(), Error>) {
        // No socket is open, so no packet can legitimately have arrived.
        (
            ClientState::Disconnected(self),
            Err(Error::InvalidStateTransition),
        )
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
        at: T,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            Command::Connect(options) => {
                match begin_connect(settings, session, scratchpad, options, at) {
                    Ok(next) => (next, Ok(())),
                    Err(err) => (ClientState::Disconnected(self), Err(err)),
                }
            }
            // Nothing is connected, so there is nothing to publish on,
            // acknowledge, (un)subscribe or disconnect.
            _ => (
                ClientState::Disconnected(self),
                Err(Error::InvalidStateTransition),
            ),
        }
    }

    fn handle_event(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
        _at: T,
    ) -> (ClientState, Result<(), Error>) {
        match evt {
            // The protocol never asked for a socket: reject it so stored
            // connect options are not replayed on a socket the application did
            // not request.
            DriverEvent::SocketConnected => (
                ClientState::Disconnected(self),
                Err(Error::InvalidStateTransition),
            ),
            // No socket is open (or it was already closed by the protocol):
            // nothing to tear down and no duplicate `Disconnected` to report.
            DriverEvent::SocketClosed | DriverEvent::SocketError => {
                (ClientState::Disconnected(self), Ok(()))
            }
            _ => (
                ClientState::Disconnected(self),
                Err(Error::InvalidStateTransition),
            ),
        }
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<T>,
        _now: T,
    ) -> (ClientState, Result<(), Error>) {
        // No connection attempt is in progress, so nothing can time out.
        (ClientState::Disconnected(self), Ok(()))
    }

    fn close(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<T>,
    ) -> (ClientState, Result<(), Error>) {
        // Every teardown path already reset the connection state on the way
        // here, and the initial state has none to reset.
        (ClientState::Disconnected(self), Ok(()))
    }
}
