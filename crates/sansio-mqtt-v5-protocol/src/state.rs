mod connected;
mod connecting;
mod disconnected;
use connected::Connected;
use connecting::Connecting;
pub(crate) use disconnected::Disconnected;

use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::DisconnectReasonCode;

use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::settings::ClientSettings;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Time;

/// Tears the connection down with a protocol-error DISCONNECT, reports it to
/// the application, and moves to [`ClientState::Disconnected`].
///
/// [MQTT-4.13.1-1] A Protocol Error requires the client to send DISCONNECT with
/// the corresponding Reason Code and close the Network Connection. The failure
/// is handled, so the input that triggered it is not rejected: `Ok(())`.
pub(crate) fn fail_with_protocol_error<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
) -> (ClientState, Result<(), Error>) {
    fail_with(
        settings,
        session,
        scratchpad,
        DisconnectReasonCode::ProtocolError,
    )
}

/// Like [`fail_with_protocol_error`], closing with a specific `reason`.
pub(crate) fn fail_with<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    reason: DisconnectReasonCode,
) -> (ClientState, Result<(), Error>) {
    let _ = queues::fail_connection(settings, session, scratchpad, reason);
    (ClientState::Disconnected(Disconnected), Ok(()))
}

/// Handles the driver reporting an unsolicited socket loss while a socket was
/// requested or open.
pub(crate) fn on_socket_lost<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
) -> (ClientState, Result<(), Error>) {
    queues::on_socket_lost(settings, session, scratchpad);
    (ClientState::Disconnected(Disconnected), Ok(()))
}

/// The MQTT client lifecycle as a type-state FSM.
///
/// `Transitioning` is a zero-size default used as a `core::mem::take` sentinel.
/// It is never observable in stable code — the `unreachable!` in its trait impl
/// fires only if a bug leaves the FSM without a next state after `dispatch`.
#[derive(Default, Debug)]
pub(crate) enum ClientState {
    #[default]
    Transitioning,
    Disconnected(Disconnected),
    Connecting(Connecting),
    Connected(Connected),
}

impl ClientState {
    /// Whether the driver has reported a connected socket that the protocol
    /// has neither closed nor seen lost, i.e. whether inbound bytes can be
    /// legitimate.
    pub(crate) fn has_open_socket(&self) -> bool {
        match self {
            Self::Connecting(connecting) => connecting.has_open_socket(),
            Self::Connected(_) => true,
            Self::Transitioning | Self::Disconnected(_) => false,
        }
    }

    /// Handles a control packet read from the open socket.
    ///
    /// `Client::handle_read` is the single place that enforces
    /// [`has_open_socket`](Self::has_open_socket) before parsing, so only
    /// [`Connecting`] (awaiting CONNACK) and [`Connected`] ever see a packet;
    /// every other state rejects it unchanged.
    pub(crate) fn handle_control_packet<T>(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        packet: ControlPacket,
        received_at: T,
    ) -> (Self, Result<(), Error>)
    where
        T: Time,
    {
        match self {
            Self::Connecting(x) => {
                x.handle_control_packet(settings, session, scratchpad, packet, received_at)
            }
            Self::Connected(x) => {
                x.handle_control_packet(settings, session, scratchpad, packet, received_at)
            }
            state @ (Self::Transitioning | Self::Disconnected(_)) => {
                (state, Err(Error::InvalidStateTransition))
            }
        }
    }
}

pub(crate) trait StateHandler<T>: Sized {
    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
        at: T,
    ) -> (ClientState, Result<(), Error>);

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
        at: T,
    ) -> (ClientState, Result<(), Error>);

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        now: T,
    ) -> (ClientState, Result<(), Error>);

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
    ) -> (ClientState, Result<(), Error>);
}

/// Forwards a [`StateHandler`] method to whichever concrete state is live.
///
/// Every method delegates identically, so spelling the arms out once here
/// keeps the states in lockstep and makes adding a state a one-line change.
macro_rules! forward_to_state {
    ($state:expr, $method:ident($($arg:expr),* $(,)?)) => {
        match $state {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Disconnected(x) => x.$method($($arg),*),
            ClientState::Connecting(x) => x.$method($($arg),*),
            ClientState::Connected(x) => x.$method($($arg),*),
        }
    };
}

impl<T> StateHandler<T> for ClientState
where
    T: Time,
{
    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
        at: T,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, handle_write(settings, session, scratchpad, msg, at))
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
        at: T,
    ) -> (ClientState, Result<(), Error>) {
        if matches!(evt, DriverEvent::SocketClosed | DriverEvent::SocketError) {
            // Unsent bytes belonged to the lost socket and must never reach
            // the next one.
            scratchpad.write_queue.clear();
            if scratchpad.action_queue.contains(&DriverAction::CloseSocket) {
                // The protocol already closed the socket the driver lost: the
                // loss was reported before the driver got to `CloseSocket`
                // (e.g. writing the DISCONNECT that precedes it failed).
                // Teardown is done and any new attempt queued behind
                // `CloseSocket` has not started, so nothing changes.
                return (self, Ok(()));
            }
        }
        forward_to_state!(self, handle_event(settings, session, scratchpad, evt, at))
    }

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        now: T,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, handle_timeout(settings, session, scratchpad, now))
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, close(settings, session, scratchpad))
    }
}
