use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::DisconnectReasonCode;

use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::session_ops;
use crate::state::connecting::Connecting;
use crate::state::disconnected::Disconnected;
use crate::state::{ClientState, StateHandler};
use crate::types::{ClientSettings, ConnectionOptions, DriverEventIn, Error, UserWriteIn};

#[allow(dead_code)]
pub(crate) struct Start;

/// Shared logic for handling a `UserWriteIn::Connect` in the Start or Disconnected state.
///
/// Stores the connection options, recomputes effective limits, optionally clears session
/// state for a clean start, marks the session persistence flag, enqueues `OpenSocket` if
/// not already present, and transitions to the `Connecting` state.
///
/// [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
pub(crate) fn handle_connect<Time: Copy + Ord + 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    options: ConnectionOptions,
) -> (ClientState, Result<(), Error>) {
    scratchpad.pending_connect_options = options;
    limits::recompute_effective_limits(settings, scratchpad);
    if scratchpad.pending_connect_options.clean_start {
        // [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
        session.clear();
    }
    scratchpad.session_should_persist = scratchpad
        .pending_connect_options
        .session_expiry_interval
        .unwrap_or(0)
        > 0;

    if !scratchpad
        .action_queue
        .iter()
        .any(|event| matches!(event, crate::types::DriverEventOut::OpenSocket))
    {
        scratchpad
            .action_queue
            .push_back(crate::types::DriverEventOut::OpenSocket);
    }

    let connecting = Connecting {
        pending_connect_options: core::mem::take(&mut scratchpad.pending_connect_options),
    };
    (ClientState::Connecting(connecting), Ok(()))
}

impl<Time: Copy + Ord + 'static> StateHandler<Time> for Start {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        _packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        let _ = queues::fail_protocol_and_disconnect(
            settings,
            session,
            scratchpad,
            DisconnectReasonCode::ProtocolError,
        );
        (
            ClientState::Disconnected(Disconnected),
            Err(Error::ProtocolError),
        )
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            UserWriteIn::Connect(options) => handle_connect(settings, session, scratchpad, options),
            _ => (ClientState::Start(self), Err(Error::InvalidStateTransition)),
        }
    }

    fn handle_event(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Start(self), Err(Error::InvalidStateTransition))
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        _scratchpad: &mut ClientScratchpad<Time>,
        _now: Time,
    ) -> (ClientState, Result<(), Error>) {
        (ClientState::Start(self), Ok(()))
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        session_ops::reset_keepalive(scratchpad);
        limits::reset_negotiated_limits(settings, session, scratchpad);
        session_ops::maybe_reset_session_state(session, scratchpad);
        (ClientState::Disconnected(Disconnected), Ok(()))
    }
}
