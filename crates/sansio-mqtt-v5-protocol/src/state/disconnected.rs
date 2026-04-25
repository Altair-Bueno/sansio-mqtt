use sansio_mqtt_v5_types::ControlPacket;

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::connecting::Connecting;
use crate::state::{ClientState, StateHandler};
use crate::types::{ClientSettings, DriverEventIn, DriverEventOut, Error, InstantAdd, UserWriteIn};

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct Disconnected;

impl<Time: InstantAdd> StateHandler<Time> for Disconnected {
    fn handle_control_packet(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Disconnected(self), Err(Error::ProtocolError))
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            UserWriteIn::Connect(options) => {
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
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        match evt {
            DriverEventIn::SocketConnected => {
                // Use stored pending_connect_options (from before disconnection) to reconnect.
                let connecting = Connecting {
                    pending_connect_options: scratchpad.pending_connect_options.clone(),
                    connect_sent: false,
                };
                crate::state::connecting::on_socket_connected(
                    connecting, settings, session, scratchpad,
                )
            }
            DriverEventIn::SocketClosed => {
                // Socket closed while already disconnected; no duplicate Disconnected event.
                (ClientState::Disconnected(self), Ok(()))
            }
            DriverEventIn::SocketError => {
                // Socket error while already disconnected; enqueue CloseSocket only.
                scratchpad
                    .action_queue
                    .push_back(DriverEventOut::CloseSocket);
                (ClientState::Disconnected(self), Err(Error::ProtocolError))
            }
        }
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _now: Time,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Disconnected(self), Ok(()))
    }

    fn close(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Disconnected(self), Ok(()))
    }
}
