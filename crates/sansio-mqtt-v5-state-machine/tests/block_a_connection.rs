use core::time::Duration;
use sansio_mqtt_v5_contract::{Action, ConnectOptions, TimerKey};
use sansio_mqtt_v5_state_machine::{Event, StateMachine};

#[test]
fn block_a_user_connect_emits_connect_send_and_timeout_schedule() {
    let mut machine = StateMachine::new_default();
    let connect_options = ConnectOptions {
        connect_timeout: Duration::from_millis(4_321),
        ..ConnectOptions::default()
    };

    let actions = machine.handle(Event::UserConnect(connect_options));

    assert_eq!(actions.len(), 2);
    assert!(matches!(&actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0x10, 0x00]));
    assert_eq!(
        actions[1],
        Action::ScheduleTimer {
            key: TimerKey::ConnectTimeout,
            delay_ms: 4_321,
        }
    );

    let second_actions = machine.handle(Event::UserConnect(ConnectOptions {
        connect_timeout: Duration::from_millis(9_999),
        ..ConnectOptions::default()
    }));

    assert!(
        second_actions.is_empty(),
        "expected no actions when connecting is already in progress"
    );
}
