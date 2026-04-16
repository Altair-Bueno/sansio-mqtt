use sansio_mqtt_v5_contract::{
    Action, ConnectOptions, DisconnectReason, Input, PublishRequest, Qos, SessionAction,
    SubscribeRequest, TimerKey, SESSION_ACTION_PAYLOAD_CAPACITY, SESSION_ACTION_TOPIC_CAPACITY,
    TOPIC_CAPACITY,
};
use sansio_mqtt_v5_state_machine::StateMachine;

const TOPIC: &str = "sensor/temp";
const PAYLOAD: &[u8] = &[0x2A, 0x2B];

fn connected_machine() -> StateMachine {
    let mut machine = StateMachine::new_default();
    let _ = machine.handle(Input::UserConnect(ConnectOptions {
        keep_alive_secs: Some(60),
        ..ConnectOptions::default()
    }));
    let _ = machine.handle(Input::PacketConnAck);
    machine
}

fn inbound_publish(qos: Qos, packet_id: Option<u16>) -> Input<'static> {
    let mut topic = heapless::String::<SESSION_ACTION_TOPIC_CAPACITY>::new();
    assert!(topic.push_str(TOPIC).is_ok());
    let payload = heapless::Vec::<u8, SESSION_ACTION_PAYLOAD_CAPACITY>::from_slice(PAYLOAD)
        .expect("payload fits");

    Input::PacketPublish {
        topic,
        payload,
        qos,
        packet_id,
    }
}

fn qos1_publish() -> PublishRequest {
    let mut publish = PublishRequest {
        qos: Qos::AtLeast,
        ..PublishRequest::default()
    };
    let mut topic = heapless::String::<TOPIC_CAPACITY>::new();
    assert!(topic.push_str(TOPIC).is_ok());
    publish.topic = topic;
    assert!(publish.payload.push(PAYLOAD[0]).is_ok());
    publish
}

fn qos2_publish() -> PublishRequest {
    let mut publish = PublishRequest {
        qos: Qos::Exactly,
        ..PublishRequest::default()
    };
    let mut topic = heapless::String::<TOPIC_CAPACITY>::new();
    assert!(topic.push_str(TOPIC).is_ok());
    publish.topic = topic;
    assert!(publish.payload.push(PAYLOAD[0]).is_ok());
    publish
}

fn subscribe_request() -> SubscribeRequest {
    SubscribeRequest::single(TOPIC).expect("topic fits")
}

fn expected_publish_received_action() -> Action {
    let mut topic = heapless::String::<SESSION_ACTION_TOPIC_CAPACITY>::new();
    assert!(topic.push_str(TOPIC).is_ok());
    Action::SessionAction(SessionAction::PublishReceived {
        topic,
        payload: heapless::Vec::from_slice(PAYLOAD).expect("fits"),
    })
}

#[test]
fn inbound_qos1_publish_sends_puback_then_publish_received() {
    let mut machine = connected_machine();

    let actions = machine.handle(inbound_publish(Qos::AtLeast, Some(7)));

    assert_eq!(actions.len(), 2);
    assert!(
        matches!(&actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0x40, 0x02, 0x00, 0x07])
    );
    assert_eq!(actions[1], expected_publish_received_action());
}

#[test]
fn inbound_qos0_publish_emits_publish_received_only() {
    let mut machine = connected_machine();

    let actions = machine.handle(inbound_publish(Qos::AtMost, None));

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], expected_publish_received_action());
}

#[test]
fn inbound_qos2_publish_then_pubrel_sends_pubrec_then_pubcomp_and_publish_received() {
    let mut machine = connected_machine();

    let publish_actions = machine.handle(inbound_publish(Qos::Exactly, Some(10)));

    assert_eq!(publish_actions.len(), 1);
    assert!(
        matches!(&publish_actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0x50, 0x02, 0x00, 0x0A])
    );

    let pubrel_actions = machine.handle(Input::PacketPubRel { packet_id: 10 });
    assert_eq!(pubrel_actions.len(), 2);
    assert!(
        matches!(&pubrel_actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0x70, 0x02, 0x00, 0x0A])
    );
    assert_eq!(pubrel_actions[1], expected_publish_received_action());
}

fn prepare_idle(_machine: &mut StateMachine) -> Option<u16> {
    None
}

fn prepare_waiting_for_pingresp(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Input::TimerFired(TimerKey::Keepalive));
    None
}

fn prepare_waiting_for_puback(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Input::UserPublish(qos1_publish()));
    Some(1)
}

fn prepare_waiting_for_pubrec(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Input::UserPublish(qos2_publish()));
    Some(1)
}

fn prepare_waiting_for_pubcomp(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Input::UserPublish(qos2_publish()));
    let _ = machine.handle(Input::PacketPubRec { packet_id: 1 });
    Some(1)
}

fn prepare_waiting_for_suback(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Input::UserSubscribe(subscribe_request()));
    Some(1)
}

#[test]
fn malformed_inbound_qos1_without_packet_id_disconnects_from_connected_substates() {
    struct Case {
        name: &'static str,
        prepare: fn(&mut StateMachine) -> Option<u16>,
    }

    let cases = [
        Case {
            name: "idle",
            prepare: prepare_idle,
        },
        Case {
            name: "waiting_for_pingresp",
            prepare: prepare_waiting_for_pingresp,
        },
        Case {
            name: "waiting_for_puback",
            prepare: prepare_waiting_for_puback,
        },
        Case {
            name: "waiting_for_pubrec",
            prepare: prepare_waiting_for_pubrec,
        },
        Case {
            name: "waiting_for_pubcomp",
            prepare: prepare_waiting_for_pubcomp,
        },
        Case {
            name: "waiting_for_suback",
            prepare: prepare_waiting_for_suback,
        },
    ];

    for case in cases {
        let mut machine = connected_machine();
        let _ = (case.prepare)(&mut machine);

        let actions = machine.handle(inbound_publish(Qos::AtLeast, None));

        assert_eq!(
            actions.len(),
            2,
            "unexpected action count for {}",
            case.name
        );
        assert!(
            matches!(&actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0xE0, 0x00]),
            "missing DISCONNECT packet for {}",
            case.name
        );
        assert_eq!(
            actions[1],
            Action::SessionAction(SessionAction::Disconnected {
                reason: DisconnectReason::ProtocolError,
            }),
            "missing protocol error disconnect for {}",
            case.name
        );

        let disconnected_actions = machine.handle(Input::UserPublish(PublishRequest::default()));
        assert!(
            disconnected_actions.is_empty(),
            "expected disconnected state to ignore publish for {}",
            case.name
        );
    }
}

#[test]
fn malformed_inbound_qos2_without_packet_id_disconnects_from_connected_substates() {
    struct Case {
        name: &'static str,
        prepare: fn(&mut StateMachine) -> Option<u16>,
    }

    let cases = [
        Case {
            name: "idle",
            prepare: prepare_idle,
        },
        Case {
            name: "waiting_for_pingresp",
            prepare: prepare_waiting_for_pingresp,
        },
        Case {
            name: "waiting_for_puback",
            prepare: prepare_waiting_for_puback,
        },
        Case {
            name: "waiting_for_pubrec",
            prepare: prepare_waiting_for_pubrec,
        },
        Case {
            name: "waiting_for_pubcomp",
            prepare: prepare_waiting_for_pubcomp,
        },
        Case {
            name: "waiting_for_suback",
            prepare: prepare_waiting_for_suback,
        },
    ];

    for case in cases {
        let mut machine = connected_machine();
        let _ = (case.prepare)(&mut machine);

        let actions = machine.handle(inbound_publish(Qos::Exactly, None));

        assert_eq!(
            actions.len(),
            2,
            "unexpected action count for {}",
            case.name
        );
        assert!(
            matches!(&actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0xE0, 0x00]),
            "missing DISCONNECT packet for {}",
            case.name
        );
        assert_eq!(
            actions[1],
            Action::SessionAction(SessionAction::Disconnected {
                reason: DisconnectReason::ProtocolError,
            }),
            "missing protocol error disconnect for {}",
            case.name
        );

        let disconnected_actions = machine.handle(Input::UserPublish(PublishRequest::default()));
        assert!(
            disconnected_actions.is_empty(),
            "expected disconnected state to ignore publish for {}",
            case.name
        );
    }
}

#[test]
fn user_disconnect_from_connected_substates_emits_disconnect_cancels_timers_and_transitions() {
    struct Case {
        name: &'static str,
        prepare: fn(&mut StateMachine) -> Option<u16>,
    }

    let cases = [
        Case {
            name: "idle",
            prepare: prepare_idle,
        },
        Case {
            name: "waiting_for_pingresp",
            prepare: prepare_waiting_for_pingresp,
        },
        Case {
            name: "waiting_for_puback",
            prepare: prepare_waiting_for_puback,
        },
        Case {
            name: "waiting_for_pubrec",
            prepare: prepare_waiting_for_pubrec,
        },
        Case {
            name: "waiting_for_pubcomp",
            prepare: prepare_waiting_for_pubcomp,
        },
        Case {
            name: "waiting_for_suback",
            prepare: prepare_waiting_for_suback,
        },
    ];

    for case in cases {
        let mut machine = connected_machine();
        let ack_packet_id = (case.prepare)(&mut machine);

        let actions = machine.handle(Input::UserDisconnect);

        let expected_len = if ack_packet_id.is_some() { 5 } else { 4 };
        assert_eq!(
            actions.len(),
            expected_len,
            "unexpected action count for {}",
            case.name
        );
        assert!(
            matches!(&actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0xE0, 0x00])
        );
        assert_eq!(
            actions[1],
            Action::CancelTimer(TimerKey::Keepalive),
            "missing keepalive cancel for {}",
            case.name
        );
        assert_eq!(
            actions[2],
            Action::CancelTimer(TimerKey::PingRespTimeout),
            "missing pingresp timeout cancel for {}",
            case.name
        );

        let disconnected_index = if let Some(packet_id) = ack_packet_id {
            assert_eq!(
                actions[3],
                Action::CancelTimer(TimerKey::AckTimeout(packet_id)),
                "missing ack timeout cancel for {}",
                case.name
            );
            4
        } else {
            3
        };

        assert_eq!(
            actions[disconnected_index],
            Action::SessionAction(SessionAction::Disconnected {
                reason: DisconnectReason::Normal,
            }),
            "missing disconnected session action for {}",
            case.name
        );

        let disconnected_actions = machine.handle(Input::UserPublish(PublishRequest::default()));
        assert!(
            disconnected_actions.is_empty(),
            "expected disconnected state to ignore publish for {}",
            case.name
        );
    }
}
