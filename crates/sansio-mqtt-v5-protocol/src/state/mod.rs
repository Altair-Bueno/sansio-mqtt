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

use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::types::ClientSettings;
use crate::types::DriverEventIn;
use crate::types::Error;
use crate::types::ProtocolTime;
use crate::types::UserWriteIn;

/// Tears the connection down with a protocol-error DISCONNECT and moves to
/// [`ClientState::Disconnected`].
///
/// [MQTT-4.13.1-1] A Protocol Error requires the client to send DISCONNECT with
/// the corresponding Reason Code and close the Network Connection.
pub(crate) fn fail_with_protocol_error<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) -> (ClientState, Result<(), Error>) {
    queues::fail_protocol_and_disconnect(
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

pub(crate) trait StateHandler<Time>: Sized {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
        received_at: Time,
    ) -> (ClientState, Result<(), Error>);

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>);

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>);

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        now: Time,
    ) -> (ClientState, Result<(), Error>);

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
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

impl<Time> StateHandler<Time> for ClientState
where
    Time: ProtocolTime,
{
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
        received_at: Time,
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
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, handle_write(settings, session, scratchpad, msg))
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, handle_event(settings, session, scratchpad, evt))
    }

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        now: Time,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, handle_timeout(settings, session, scratchpad, now))
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        forward_to_state!(self, close(settings, session, scratchpad))
    }
}
