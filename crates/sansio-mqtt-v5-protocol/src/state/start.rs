use crate::client::ClientSettings;
use crate::convert;
use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::ClientState;
use crate::state::StateHandler;
use crate::state::connecting::Connecting;
use crate::state::disconnected::Disconnected;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_v5_types::ControlPacket;

/// Initial state: no socket has ever been requested.
#[derive(Debug)]
pub(crate) struct Start;

/// Shared logic for handling a `Command::Connect` in the Start or
/// Disconnected state.
///
/// Validates the options by building the CONNECT they describe; on failure the
/// command is rejected and nothing changes. Otherwise stores them, recomputes
/// effective limits, optionally clears session state for a clean start, marks
/// the session persistence flag, enqueues `OpenSocket` and moves to
/// [`Connecting`] awaiting the socket. CONNECT itself is only sent once the
/// driver reports `SocketConnected`.
///
/// [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
pub(crate) fn begin_connect<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    options: ConnectOptions,
) -> Result<ClientState, Error>
where
    T: Time,
{
    let connect = convert::connect_options_to_wire(settings, &options)?;
    let _ = queues::encode_control_packet(&ControlPacket::Connect(connect))?;

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

    Ok(ClientState::Connecting(Connecting::awaiting_socket()))
}

/// Shared handling of every input to a state without a socket (Start or
/// Disconnected).
///
/// `stay` rebuilds the current state for the paths that leave it unchanged.
pub(crate) mod idle {
    use super::*;

    pub(crate) fn handle_write<T>(
        stay: ClientState,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
    ) -> (ClientState, Result<(), Error>)
    where
        T: Time,
    {
        match msg {
            Command::Connect(options) => {
                match begin_connect(settings, session, scratchpad, options) {
                    Ok(next) => (next, Ok(())),
                    Err(err) => (stay, Err(err)),
                }
            }
            // Nothing is connected, so there is nothing to publish on,
            // acknowledge, (un)subscribe or disconnect.
            _ => (stay, Err(Error::InvalidStateTransition)),
        }
    }

    pub(crate) fn handle_event(
        stay: ClientState,
        evt: DriverEvent,
    ) -> (ClientState, Result<(), Error>) {
        match evt {
            // The protocol never asked for a socket: reject it so stored
            // connect options are not replayed on a socket the application did
            // not request.
            DriverEvent::SocketConnected => (stay, Err(Error::InvalidStateTransition)),
            // No socket is open (or it was already closed by the protocol):
            // nothing to tear down and no duplicate `Disconnected` to report.
            DriverEvent::SocketClosed | DriverEvent::SocketError => (stay, Ok(())),
            _ => (stay, Err(Error::InvalidStateTransition)),
        }
    }
}

impl<T> StateHandler<T> for Start
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
        (ClientState::Start(self), Err(Error::InvalidStateTransition))
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
    ) -> (ClientState, Result<(), Error>) {
        idle::handle_write(ClientState::Start(self), settings, session, scratchpad, msg)
    }

    fn handle_event(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
    ) -> (ClientState, Result<(), Error>) {
        idle::handle_event(ClientState::Start(self), evt)
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<T>,
        _now: T,
    ) -> (ClientState, Result<(), Error>) {
        // No connection attempt is in progress, so nothing can time out.
        (ClientState::Start(self), Ok(()))
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
    ) -> (ClientState, Result<(), Error>) {
        queues::reset_connection_state(settings, session, scratchpad);
        (ClientState::Disconnected(Disconnected), Ok(()))
    }
}
