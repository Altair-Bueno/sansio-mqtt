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
use crate::types::{
    ClientSettings, ConnectionOptions, DriverEventIn, DriverEventOut, Error, UserWriteIn,
    UserWriteOut,
};

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct Start;

/// Shared logic for handling a `UserWriteIn::Connect` in the Start or Disconnected state.
///
/// Stores the connection options, recomputes effective limits, optionally clears session
/// state for a clean start, marks the session persistence flag, enqueues `OpenSocket` if
/// not already present, and stays in the caller's state (Start or Disconnected).
/// The actual transition to Connecting happens when `SocketConnected` fires.
///
/// [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
pub(crate) fn store_connect_options_and_enqueue_open_socket<Time: Copy + Ord + 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    options: ConnectionOptions,
) {
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
            UserWriteIn::Connect(options) => {
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
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        match evt {
            DriverEventIn::SocketConnected => {
                // In Start state the user may not have called Connect first; use stored
                // pending_connect_options (defaults when never set).
                let connecting = Connecting {
                    pending_connect_options: core::mem::take(
                        &mut scratchpad.pending_connect_options,
                    ),
                    connect_sent: false,
                };
                crate::state::connecting::on_socket_connected(
                    connecting, settings, session, scratchpad,
                )
            }
            DriverEventIn::SocketClosed => {
                // Socket closed unexpectedly in Start state; emit Disconnected and transition.
                scratchpad.read_queue.push_back(UserWriteOut::Disconnected);
                (ClientState::Disconnected(Disconnected), Ok(()))
            }
            DriverEventIn::SocketError => {
                // Socket error in Start state; enqueue CloseSocket and return error.
                scratchpad
                    .action_queue
                    .push_back(DriverEventOut::CloseSocket);
                (
                    ClientState::Disconnected(Disconnected),
                    Err(Error::ProtocolError),
                )
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
