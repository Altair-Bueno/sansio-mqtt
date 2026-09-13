use crate::client::ClientSettings;
use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::ClientState;
use crate::state::StateHandler;
use crate::state::disconnected::Disconnected;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_v5_types::ControlPacket;

/// Initial state: no socket has ever been opened.
#[derive(Debug)]
pub(crate) struct Start;

/// Shared logic for handling a `Command::Connect` in the Start or
/// Disconnected state.
///
/// Stores the connection options, recomputes effective limits, optionally
/// clears session state for a clean start, marks the session persistence flag,
/// enqueues `OpenSocket` if not already present, and stays in the caller's
/// state (Start or Disconnected). The actual transition to Connecting happens
/// when `SocketConnected` fires.
///
/// [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
pub(crate) fn store_connect_options_and_enqueue_open_socket<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    options: ConnectOptions,
) where
    T: Time,
{
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

    if !scratchpad
        .action_queue
        .iter()
        .any(|event| matches!(event, DriverAction::OpenSocket))
    {
        scratchpad.action_queue.push_back(DriverAction::OpenSocket);
    }
}

impl<T> StateHandler<T> for Start
where
    T: Time,
{
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        _packet: ControlPacket,
        _received_at: T,
    ) -> (ClientState, Result<(), Error>) {
        crate::state::fail_with_protocol_error(settings, session, scratchpad)
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            Command::Connect(options) => {
                store_connect_options_and_enqueue_open_socket(
                    settings, session, scratchpad, options,
                );
                (ClientState::Start(self), Ok(()))
            }
            _ => (ClientState::Start(self), Err(Error::InvalidStateTransition)),
        }
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
    ) -> (ClientState, Result<(), Error>) {
        match evt {
            DriverEvent::SocketConnected => {
                // In Start state the user may not have called Connect first;
                // `pending_connect_options` may still be `None`.
                crate::state::connecting::on_socket_connected(settings, session, scratchpad)
            }
            DriverEvent::SocketClosed => {
                // Socket closed unexpectedly in Start state; emit Disconnected
                // and transition.
                scratchpad.read_queue.push_back(Event::Disconnected(None));
                (ClientState::Disconnected(Disconnected), Ok(()))
            }
            DriverEvent::SocketError => {
                // Socket error in Start state; enqueue CloseSocket and return
                // error.
                scratchpad.action_queue.push_back(DriverAction::CloseSocket);
                (
                    ClientState::Disconnected(Disconnected),
                    Err(Error::ProtocolError),
                )
            }
            _ => (ClientState::Start(self), Err(Error::InvalidStateTransition)),
        }
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        _now: T,
    ) -> (ClientState, Result<(), Error>) {
        // [MQTT-3.1.4-5] A timeout in the Start state means no connection was
        // established within the caller-imposed deadline. Close the socket and
        // signal the error.
        scratchpad.action_queue.push_back(DriverAction::CloseSocket);
        (
            ClientState::Disconnected(Disconnected),
            Err(Error::ConnectTimeout),
        )
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
