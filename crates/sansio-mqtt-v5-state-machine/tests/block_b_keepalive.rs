use core::time::Duration;
use sansio_mqtt_v5_contract::{Action, ConnectOptions, Input, TimerKey};
use sansio_mqtt_v5_state_machine::StateMachine;

fn connected_machine() -> StateMachine {
    connect_with_keepalive(Some(Duration::from_secs(60)))
}

fn connect_with_keepalive(keep_alive: Option<Duration>) -> StateMachine {
    let mut machine = StateMachine::new_default();
    let _ = machine.handle(Input::UserConnect(ConnectOptions {
        keep_alive,
        ..ConnectOptions::default()
    }));
    let _ = machine.handle(Input::PacketConnAck);
    machine
}

#[test]
fn idle_keepalive_timer_sends_pingreq_and_waits_for_pingresp() {
    let mut machine = connected_machine();

    let actions = machine.handle(Input::TimerFired(TimerKey::Keepalive));

    assert_eq!(actions.len(), 2);
    assert!(matches!(&actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0xC0, 0x00]));
    assert!(matches!(
        actions[1],
        Action::ScheduleTimer {
            key: TimerKey::PingRespTimeout,
            ..
        }
    ));
}

#[test]
fn waiting_for_pingresp_receives_pingresp_and_returns_idle() {
    let mut machine = connected_machine();
    let _ = machine.handle(Input::TimerFired(TimerKey::Keepalive));

    let actions = machine.handle(Input::PacketPingResp);

    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], Action::CancelTimer(TimerKey::PingRespTimeout));
    assert!(matches!(
        actions[1],
        Action::ScheduleTimer {
            key: TimerKey::Keepalive,
            ..
        }
    ));

    let idle_actions = machine.handle(Input::TimerFired(TimerKey::Keepalive));
    assert!(
        !idle_actions.is_empty(),
        "expected idle state to react to keepalive timer"
    );
}

#[test]
fn pingresp_timeout_disconnects_session() {
    let mut machine = connected_machine();
    let _ = machine.handle(Input::TimerFired(TimerKey::Keepalive));

    let actions = machine.handle(Input::TimerFired(TimerKey::PingRespTimeout));

    assert_eq!(actions.len(), 2);
    assert!(matches!(&actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0xE0, 0x00]));
    assert_eq!(actions[1], Action::DisconnectedByTimeout);
}

#[test]
fn reconnect_without_keepalive_does_not_schedule_stale_keepalive_timer() {
    let mut machine = connect_with_keepalive(Some(Duration::from_secs(3)));
    let _ = machine.handle(Input::TimerFired(TimerKey::Keepalive));
    let _ = machine.handle(Input::TimerFired(TimerKey::PingRespTimeout));

    let _ = machine.handle(Input::UserConnect(ConnectOptions::default()));
    let actions = machine.handle(Input::PacketConnAck);

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], Action::CancelTimer(TimerKey::ConnectTimeout));
}
