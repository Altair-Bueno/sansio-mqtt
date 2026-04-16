use core::time::Duration;
use sansio_mqtt_v5_contract::{Action, ConnectOptions, PublishRequest, SubscribeRequest, TimerKey};
use sansio_mqtt_v5_state_machine::{Event, StateMachine};
use sansio_mqtt_v5_types::{Payload, Qos, Topic, Utf8String};

const TOPIC: &str = "sensor/temp";
const PAYLOAD: &[u8] = &[0x2A, 0x2B];

fn connected_machine() -> StateMachine {
    let mut machine = StateMachine::new_default();
    let _ = machine.handle(Event::UserConnect(ConnectOptions {
        keep_alive: Some(Duration::from_secs(60)),
        ..ConnectOptions::default()
    }));
    let _ = machine.handle(Event::PacketConnAck);
    machine
}

fn inbound_publish(qos: Qos, packet_id: Option<u16>) -> Event {
    let topic =
        Topic::try_from(Utf8String::try_from(TOPIC).expect("valid utf8")).expect("valid topic");
    let payload = Payload::from(PAYLOAD.to_vec());

    Event::PacketPublish {
        topic,
        payload,
        qos,
        packet_id,
    }
}

fn qos1_publish() -> PublishRequest {
    let mut publish = PublishRequest {
        qos: Qos::AtLeastOnce,
        ..PublishRequest::default()
    };
    publish.topic =
        Topic::try_from(Utf8String::try_from(TOPIC).expect("valid utf8")).expect("valid topic");
    publish.payload = Payload::from(vec![PAYLOAD[0]]);
    publish
}

fn qos2_publish() -> PublishRequest {
    let mut publish = PublishRequest {
        qos: Qos::ExactlyOnce,
        ..PublishRequest::default()
    };
    publish.topic =
        Topic::try_from(Utf8String::try_from(TOPIC).expect("valid utf8")).expect("valid topic");
    publish.payload = Payload::from(vec![PAYLOAD[0]]);
    publish
}

fn subscribe_request() -> SubscribeRequest {
    SubscribeRequest {
        topic_filter: Utf8String::try_from(TOPIC).expect("valid topic filter"),
        ..SubscribeRequest::default()
    }
}

fn expected_publish_received_action() -> Action {
    let topic = TOPIC.to_owned();
    Action::PublishReceived {
        topic,
        payload: PAYLOAD.to_vec(),
    }
}

#[test]
fn inbound_qos1_publish_sends_puback_then_publish_received() {
    let mut machine = connected_machine();

    let actions = machine.handle(inbound_publish(Qos::AtLeastOnce, Some(7)));

    assert_eq!(actions.len(), 2);
    assert!(
        matches!(&actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0x40, 0x02, 0x00, 0x07])
    );
    assert_eq!(actions[1], expected_publish_received_action());
}

#[test]
fn inbound_qos0_publish_emits_publish_received_only() {
    let mut machine = connected_machine();

    let actions = machine.handle(inbound_publish(Qos::AtMostOnce, None));

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], expected_publish_received_action());
}

#[test]
fn inbound_qos2_publish_then_pubrel_sends_pubrec_then_pubcomp_and_publish_received() {
    let mut machine = connected_machine();

    let publish_actions = machine.handle(inbound_publish(Qos::ExactlyOnce, Some(10)));

    assert_eq!(publish_actions.len(), 1);
    assert!(
        matches!(&publish_actions[0], Action::SendBytes(bytes) if bytes.as_slice() == [0x50, 0x02, 0x00, 0x0A])
    );

    let pubrel_actions = machine.handle(Event::PacketPubRel { packet_id: 10 });
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
    let _ = machine.handle(Event::TimerFired(TimerKey::Keepalive));
    None
}

fn prepare_waiting_for_puback(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Event::UserPublish(qos1_publish()));
    Some(1)
}

fn prepare_waiting_for_pubrec(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Event::UserPublish(qos2_publish()));
    Some(1)
}

fn prepare_waiting_for_pubcomp(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Event::UserPublish(qos2_publish()));
    let _ = machine.handle(Event::PacketPubRec { packet_id: 1 });
    Some(1)
}

fn prepare_waiting_for_suback(machine: &mut StateMachine) -> Option<u16> {
    let _ = machine.handle(Event::UserSubscribe(subscribe_request()));
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

        let actions = machine.handle(inbound_publish(Qos::AtLeastOnce, None));

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
            Action::DisconnectedByProtocolViolation,
            "missing protocol error disconnect for {}",
            case.name
        );

        let disconnected_actions = machine.handle(Event::UserPublish(PublishRequest::default()));
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

        let actions = machine.handle(inbound_publish(Qos::ExactlyOnce, None));

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
            Action::DisconnectedByProtocolViolation,
            "missing protocol error disconnect for {}",
            case.name
        );

        let disconnected_actions = machine.handle(Event::UserPublish(PublishRequest::default()));
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

        let actions = machine.handle(Event::UserDisconnect);

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
            Action::DisconnectedByLocalRequest,
            "missing disconnected session action for {}",
            case.name
        );

        let disconnected_actions = machine.handle(Event::UserPublish(PublishRequest::default()));
        assert!(
            disconnected_actions.is_empty(),
            "expected disconnected state to ignore publish for {}",
            case.name
        );
    }
}
