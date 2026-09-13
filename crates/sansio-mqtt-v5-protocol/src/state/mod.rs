pub(crate) mod connected;
pub(crate) mod connecting;
pub(crate) mod disconnected;
pub(crate) mod start;
pub(crate) use connected::Connected;
pub(crate) use connecting::Connecting;
pub(crate) use disconnected::Disconnected;
pub(crate) use start::Start;

use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::DisconnectReasonCode;

use crate::client::ClientSettings;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Time;

/// Tears the connection down with a protocol-error DISCONNECT and moves to
/// [`ClientState::Disconnected`].
///
/// [MQTT-4.13.1-1] A Protocol Error requires the client to send DISCONNECT with
/// the corresponding Reason Code and close the Network Connection.
pub(crate) fn fail_with_protocol_error<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
) -> (ClientState, Result<(), Error>) {
    queues::disconnect_and_reset(
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

/// The MQTT client lifecycle as a type-state FSM.
///
/// `Transitioning` is a zero-size default used as a `core::mem::take` sentinel.
/// It is never observable in stable code — the `unreachable!` in its trait impl
/// fires only if a bug leaves the FSM without a next state after `dispatch`.
#[derive(Default, Debug)]
pub(crate) enum ClientState {
    #[default]
    Transitioning,
    Start(Start),
    Disconnected(Disconnected),
    Connecting(Connecting),
    Connected(Connected),
}

pub(crate) trait StateHandler<T>: Sized {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        packet: ControlPacket,
        received_at: T,
    ) -> (ClientState, Result<(), Error>);

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
    ) -> (ClientState, Result<(), Error>);

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
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
/// Every method delegates identically, so spelling the five arms out once here
/// keeps the states in lockstep and makes adding a state a one-line change.
macro_rules! forward_to_state {
    ($state:expr, $method:ident($($arg:expr),* $(,)?)) => {
        match $state {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.$method($($arg),*),
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
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        packet: ControlPacket,
        received_at: T,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(
            self,
            handle_control_packet(settings, session, scratchpad, packet, received_at)
        )
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, handle_write(settings, session, scratchpad, msg))
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, handle_event(settings, session, scratchpad, evt))
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
