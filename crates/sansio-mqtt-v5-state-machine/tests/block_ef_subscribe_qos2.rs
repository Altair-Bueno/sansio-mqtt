use sansio_mqtt_v5_contract::{
    Action, ConnectOptions, Input, PublishRequest, Qos, SessionAction, SubscribeRequest, TimerKey,
    TOPIC_CAPACITY,
};
use sansio_mqtt_v5_state_machine::StateMachine;

const TOPIC: &str = "sensor/temp";
const PAYLOAD: u8 = 0x2A;
const FILTER: &str = "sensor/#";

fn connected_machine() -> StateMachine {
    let mut machine = StateMachine::new_default();
    let _ = machine.handle(Input::UserConnect(ConnectOptions::default()));
    let _ = machine.handle(Input::PacketConnAck);
    machine
}

fn publish_qos2() -> PublishRequest {
    let mut publish = PublishRequest {
        qos: Qos::Exactly,
        ..PublishRequest::default()
    };
    let mut topic = heapless::String::<TOPIC_CAPACITY>::new();
    assert!(topic.push_str(TOPIC).is_ok());
    publish.topic = topic;
    assert!(publish.payload.push(PAYLOAD).is_ok());
    publish
}

fn subscribe_request() -> SubscribeRequest {
    SubscribeRequest::single(FILTER).expect("valid topic filter")
}

fn suback_reason_codes(code: u8) -> heapless::Vec<u8, 8> {
    heapless::Vec::from_slice(&[code]).expect("single reason code fits")
}

fn assert_publish_packet(bytes: &[u8], expected_header: u8, expected_packet_id: u16) {
    let topic_len = TOPIC.len();
    let remaining = 2 + topic_len + 2 + 1 + 1;

    assert_eq!(bytes[0], expected_header);
    assert_eq!(bytes[1], remaining as u8);
    assert_eq!(bytes[2], 0x00);
    assert_eq!(bytes[3], topic_len as u8);
    assert_eq!(&bytes[4..(4 + topic_len)], TOPIC.as_bytes());
    assert_eq!(bytes[4 + topic_len], (expected_packet_id >> 8) as u8);
    assert_eq!(bytes[5 + topic_len], (expected_packet_id & 0xFF) as u8);
    assert_eq!(bytes[6 + topic_len], 0x00);
    assert_eq!(bytes[7 + topic_len], PAYLOAD);
}

#[test]
fn qos2_happy_path_publishes_pubrel_and_completes() {
    let mut machine = connected_machine();

    let publish_actions = machine.handle(Input::UserPublish(publish_qos2()));
    assert_eq!(publish_actions.len(), 2);
    match &publish_actions[0] {
        Action::SendBytes(bytes) => assert_publish_packet(bytes.as_slice(), 0x34, 1),
        action => panic!("expected send bytes action, got {action:?}"),
    }
    assert_eq!(
        publish_actions[1],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );

    let pubrec_actions = machine.handle(Input::PacketPubRec { packet_id: 1 });
    assert_eq!(pubrec_actions.len(), 3);
    assert_eq!(
        pubrec_actions[0],
        Action::CancelTimer(TimerKey::AckTimeout(1))
    );
    assert!(
        matches!(&pubrec_actions[1], Action::SendBytes(bytes) if bytes.as_slice() == [0x62, 0x02, 0x00, 0x01])
    );
    assert_eq!(
        pubrec_actions[2],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );

    let pubcomp_actions = machine.handle(Input::PacketPubComp { packet_id: 1 });
    assert_eq!(pubcomp_actions.len(), 1);
    assert_eq!(
        pubcomp_actions[0],
        Action::CancelTimer(TimerKey::AckTimeout(1))
    );

    let idle_actions = machine.handle(Input::UserPublish(PublishRequest::default()));
    assert_eq!(idle_actions.len(), 1, "expected machine to return to idle");
}

#[test]
fn qos2_ack_timeout_retries_in_both_waiting_stages() {
    let mut machine = connected_machine();
    let _ = machine.handle(Input::UserPublish(publish_qos2()));

    let waiting_pubrec = machine.handle(Input::TimerFired(TimerKey::AckTimeout(1)));
    assert_eq!(waiting_pubrec.len(), 2);
    match &waiting_pubrec[0] {
        Action::SendBytes(bytes) => assert_publish_packet(bytes.as_slice(), 0x3C, 1),
        action => panic!("expected send bytes action, got {action:?}"),
    }
    assert_eq!(
        waiting_pubrec[1],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );

    let _ = machine.handle(Input::PacketPubRec { packet_id: 1 });
    let waiting_pubcomp = machine.handle(Input::TimerFired(TimerKey::AckTimeout(1)));
    assert_eq!(waiting_pubcomp.len(), 2);
    assert!(
        matches!(&waiting_pubcomp[0], Action::SendBytes(bytes) if bytes.as_slice() == [0x62, 0x02, 0x00, 0x01])
    );
    assert_eq!(
        waiting_pubcomp[1],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );
}

#[test]
fn subscribe_and_suback_cancel_timer_and_emit_session_action() {
    let mut machine = connected_machine();

    let subscribe_actions = machine.handle(Input::UserSubscribe(subscribe_request()));
    assert_eq!(subscribe_actions.len(), 2);
    assert!(matches!(
        &subscribe_actions[0],
        Action::SendBytes(bytes) if bytes.as_slice() == [0x82, 0x0E, 0x00, 0x01, 0x00, 0x00, 0x08, b's', b'e', b'n', b's', b'o', b'r', b'/', b'#', 0x00]
    ));
    assert_eq!(
        subscribe_actions[1],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );

    let suback_actions = machine.handle(Input::PacketSubAck {
        packet_id: 1,
        reason_codes: suback_reason_codes(0x00),
    });
    assert_eq!(suback_actions.len(), 2);
    assert_eq!(
        suback_actions[0],
        Action::CancelTimer(TimerKey::AckTimeout(1))
    );
    assert_eq!(
        suback_actions[1],
        Action::SessionAction(SessionAction::SubscribeAck {
            packet_id: 1,
            reason_codes: suback_reason_codes(0x00),
        })
    );
}

#[test]
fn suback_reason_codes_preserve_success_and_failure_values() {
    let mut success_machine = connected_machine();
    let _ = success_machine.handle(Input::UserSubscribe(subscribe_request()));

    let success_actions = success_machine.handle(Input::PacketSubAck {
        packet_id: 1,
        reason_codes: suback_reason_codes(0x01),
    });
    assert_eq!(success_actions.len(), 2);
    assert_eq!(
        success_actions[1],
        Action::SessionAction(SessionAction::SubscribeAck {
            packet_id: 1,
            reason_codes: suback_reason_codes(0x01),
        })
    );

    let mut failure_machine = connected_machine();
    let _ = failure_machine.handle(Input::UserSubscribe(subscribe_request()));

    let failure_actions = failure_machine.handle(Input::PacketSubAck {
        packet_id: 1,
        reason_codes: suback_reason_codes(0x80),
    });
    assert_eq!(failure_actions.len(), 2);
    assert_eq!(
        failure_actions[1],
        Action::SessionAction(SessionAction::SubscribeAck {
            packet_id: 1,
            reason_codes: suback_reason_codes(0x80),
        })
    );
}

#[test]
fn subscribe_ack_timeout_retries_subscribe_with_dup_and_rearms_timer() {
    let mut machine = connected_machine();

    let subscribe_actions = machine.handle(Input::UserSubscribe(subscribe_request()));
    assert_eq!(subscribe_actions.len(), 2);
    let subscribe_packet = match &subscribe_actions[0] {
        Action::SendBytes(bytes) => bytes.as_slice().to_vec(),
        action => panic!("expected send bytes action, got {action:?}"),
    };

    let timeout_actions = machine.handle(Input::TimerFired(TimerKey::AckTimeout(1)));
    assert_eq!(timeout_actions.len(), 2);
    assert!(matches!(&timeout_actions[0], Action::SendBytes(bytes) if {
        let packet = bytes.as_slice();
        packet[0] == 0x82 && packet[1..] == subscribe_packet[1..]
    }));
    assert_eq!(
        timeout_actions[1],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );
}

#[test]
fn duplicate_pubrec_while_waiting_for_pubcomp_retries_pubrel_and_rearms_timer() {
    let mut machine = connected_machine();

    let _ = machine.handle(Input::UserPublish(publish_qos2()));
    let _ = machine.handle(Input::PacketPubRec { packet_id: 1 });

    let duplicate_pubrec_actions = machine.handle(Input::PacketPubRec { packet_id: 1 });
    assert_eq!(duplicate_pubrec_actions.len(), 2);
    assert!(
        matches!(&duplicate_pubrec_actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0x62, 0x02, 0x00, 0x01])
    );
    assert_eq!(
        duplicate_pubrec_actions[1],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );
}
