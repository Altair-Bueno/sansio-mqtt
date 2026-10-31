use crate::client::ClientSettings;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::ClientState;
use crate::state::StateHandler;
use crate::state::start::idle;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_v5_types::ControlPacket;

/// No live connection; a `Connect` write can start a new one.
///
/// Reached after every teardown. A `CloseSocket` queued on the way here may
/// still be pending in the action queue.
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
    ) -> (ClientState, Result<(), Error>) {
        idle::handle_write(
            ClientState::Disconnected(self),
            settings,
            session,
            scratchpad,
            msg,
        )
    }

    fn handle_event(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
    ) -> (ClientState, Result<(), Error>) {
        idle::handle_event(ClientState::Disconnected(self), evt)
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
