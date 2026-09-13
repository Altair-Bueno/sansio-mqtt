use crate::client::ClientSettings;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::ClientState;
use crate::state::StateHandler;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_v5_types::ControlPacket;

/// No live connection; a `Connect` write can start a new one.
#[derive(Debug)]
pub(crate) struct Disconnected;

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
        (ClientState::Disconnected(self), Err(Error::ProtocolError))
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
                crate::state::start::store_connect_options_and_enqueue_open_socket(
                    settings, session, scratchpad, options,
                );
                (ClientState::Disconnected(self), Ok(()))
            }
            _ => (
                ClientState::Disconnected(self),
                Err(Error::InvalidStateTransition),
            ),
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
                // Reconnect with the options stored before the disconnection.
                crate::state::connecting::on_socket_connected(settings, session, scratchpad)
            }
            DriverEvent::SocketClosed => {
                // Socket closed while already disconnected; no duplicate
                // Disconnected event.
                (ClientState::Disconnected(self), Ok(()))
            }
            DriverEvent::SocketError => {
                // Socket error while already disconnected; enqueue CloseSocket
                // only.
                scratchpad.action_queue.push_back(DriverAction::CloseSocket);
                (ClientState::Disconnected(self), Err(Error::ProtocolError))
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
        (ClientState::Disconnected(self), Ok(()))
    }

    fn close(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<T>,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Disconnected(self), Ok(()))
    }
}
