pub(crate) mod connected;
pub(crate) mod connecting;
pub(crate) mod disconnected;
pub(crate) mod start;
pub(crate) use connected::Connected;
pub(crate) use connecting::Connecting;
use core::ops::Add;
use core::time::Duration;
pub(crate) use disconnected::Disconnected;
pub(crate) use start::Start;

use sansio_mqtt_v5_types::ControlPacket;

use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::types::ClientSettings;
use crate::types::DriverEventIn;
use crate::types::Error;
use crate::types::UserWriteIn;

/// The MQTT client lifecycle as a type-state FSM.
///
/// `Transitioning` is a zero-size default used as a `core::mem::take` sentinel.
/// It is never observable in stable code — the `unreachable!` in its trait impl
/// fires only if a bug leaves the FSM without a next state after `dispatch`.
#[derive(Default, Debug)]
#[allow(dead_code, clippy::large_enum_variant)]
pub(crate) enum ClientState {
    #[default]
    Transitioning,
    Start(Start),
    Disconnected(Disconnected),
    Connecting(Connecting),
    Connected(Connected),
}

#[allow(dead_code)]
pub(crate) trait StateHandler<Time>: Sized {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
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

impl<Time> StateHandler<Time> for ClientState
where
    Time: Ord + Add<Duration, Output = Time> + Copy,
{
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.handle_control_packet(settings, session, scratchpad, packet),
            ClientState::Disconnected(x) => {
                x.handle_control_packet(settings, session, scratchpad, packet)
            }
            ClientState::Connecting(x) => {
                x.handle_control_packet(settings, session, scratchpad, packet)
            }
            ClientState::Connected(x) => {
                x.handle_control_packet(settings, session, scratchpad, packet)
            }
        }
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.handle_write(settings, session, scratchpad, msg),
            ClientState::Disconnected(x) => x.handle_write(settings, session, scratchpad, msg),
            ClientState::Connecting(x) => x.handle_write(settings, session, scratchpad, msg),
            ClientState::Connected(x) => x.handle_write(settings, session, scratchpad, msg),
        }
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.handle_event(settings, session, scratchpad, evt),
            ClientState::Disconnected(x) => x.handle_event(settings, session, scratchpad, evt),
            ClientState::Connecting(x) => x.handle_event(settings, session, scratchpad, evt),
            ClientState::Connected(x) => x.handle_event(settings, session, scratchpad, evt),
        }
    }

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        now: Time,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.handle_timeout(settings, session, scratchpad, now),
            ClientState::Disconnected(x) => x.handle_timeout(settings, session, scratchpad, now),
            ClientState::Connecting(x) => x.handle_timeout(settings, session, scratchpad, now),
            ClientState::Connected(x) => x.handle_timeout(settings, session, scratchpad, now),
        }
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        match self {
            ClientState::Transitioning => unreachable!("FSM observed mid-transition"),
            ClientState::Start(x) => x.close(settings, session, scratchpad),
            ClientState::Disconnected(x) => x.close(settings, session, scratchpad),
            ClientState::Connecting(x) => x.close(settings, session, scratchpad),
            ClientState::Connected(x) => x.close(settings, session, scratchpad),
        }
    }
}
