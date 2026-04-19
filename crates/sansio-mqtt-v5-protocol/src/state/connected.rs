use sansio_mqtt_v5_types::ControlPacket;

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::{ClientState, StateHandler};
use crate::types::{ClientSettings, DriverEventIn, Error, UserWriteIn};

#[allow(dead_code)]
pub(crate) struct Connected;

impl<Time: Copy + Ord + 'static> StateHandler<Time> for Connected {
    fn handle_control_packet(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        todo!("implemented in Task 11")
    }

    fn handle_write(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        todo!("implemented in Task 11")
    }

    fn handle_event(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        todo!("implemented in Task 11")
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _now: Time,
    ) -> (ClientState, Result<(), Error>) {
        todo!("implemented in Task 11")
    }

    fn close(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        todo!("implemented in Task 11")
    }
}
