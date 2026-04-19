use sansio_mqtt_v5_types::ControlPacket;

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::{ClientState, StateHandler};
use crate::types::{ClientSettings, DriverEventIn, Error, UserWriteIn};

#[allow(dead_code)]
pub(crate) struct Disconnected;

impl<Time: Copy + Ord + 'static> StateHandler<Time> for Disconnected {
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
                crate::state::start::handle_connect(settings, session, scratchpad, options)
            }
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
        _scratchpad: &mut ClientScratchpad<Time>,
        _evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        (
            ClientState::Disconnected(self),
            Err(Error::InvalidStateTransition),
        )
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
