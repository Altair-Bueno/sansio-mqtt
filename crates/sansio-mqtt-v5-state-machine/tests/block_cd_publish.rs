use sansio_mqtt_v5_contract::{Action, ConnectOptions, Input, PublishRequest, TimerKey};
use sansio_mqtt_v5_state_machine::StateMachine;
use sansio_mqtt_v5_types::Qos;

const TOPIC: &str = "sensor/temp";
const PAYLOAD: u8 = 0x2A;

fn connected_machine() -> StateMachine {
    let mut machine = StateMachine::new_default();
    let _ = machine.handle(Input::UserConnect(ConnectOptions::default()));
    let _ = machine.handle(Input::PacketConnAck);
    machine
}

fn publish_with_qos(qos: Qos) -> PublishRequest {
    let mut publish = PublishRequest {
        qos,
        ..PublishRequest::default()
    };
    publish.topic = TOPIC.to_owned();
    publish.payload.push(PAYLOAD);
    publish
}

fn assert_publish_packet(
    bytes: &[u8],
    expected_header: u8,
    expected_packet_id: Option<u16>,
    expected_payload: &[u8],
) {
    let topic_len = TOPIC.len();
    let packet_id_len = if expected_packet_id.is_some() { 2 } else { 0 };
    let remaining = 2 + topic_len + packet_id_len + 1 + expected_payload.len();

    assert_eq!(bytes[0], expected_header);
    assert_eq!(bytes[1], remaining as u8);
    assert_eq!(bytes[2], 0x00);
    assert_eq!(bytes[3], topic_len as u8);
    assert_eq!(&bytes[4..(4 + topic_len)], TOPIC.as_bytes());

    let mut cursor = 4 + topic_len;
    if let Some(packet_id) = expected_packet_id {
        assert_eq!(bytes[cursor], (packet_id >> 8) as u8);
        assert_eq!(bytes[cursor + 1], (packet_id & 0xFF) as u8);
        cursor += 2;
    }

    assert_eq!(bytes[cursor], 0x00, "expected zero PUBLISH properties");
    cursor += 1;
    assert_eq!(&bytes[cursor..], expected_payload);
}

fn decode_remaining_length(bytes: &[u8]) -> (usize, usize) {
    let mut value = 0usize;
    let mut multiplier = 1usize;
    let mut consumed = 0usize;

    for &encoded in &bytes[1..] {
        consumed += 1;
        value += usize::from(encoded & 0x7F) * multiplier;

        if (encoded & 0x80) == 0 {
            return (value, consumed);
        }

        multiplier *= 128;
    }

    panic!("remaining length must terminate");
}

#[test]
fn qos0_publish_sends_once_and_stays_idle() {
    let mut machine = connected_machine();

    let actions = machine.handle(Input::UserPublish(publish_with_qos(Qos::AtMostOnce)));

    assert_eq!(actions.len(), 1);
    match &actions[0] {
        Action::SendBytes(bytes) => assert_publish_packet(bytes.as_slice(), 0x30, None, &[PAYLOAD]),
        action => panic!("expected send bytes action, got {action:?}"),
    }

    let second = machine.handle(Input::UserPublish(publish_with_qos(Qos::AtMostOnce)));
    assert_eq!(
        second.len(),
        1,
        "expected machine to remain in idle after QoS0"
    );
}

#[test]
fn qos1_publish_allocates_packet_id_and_waits_for_puback() {
    let mut machine = connected_machine();

    let actions = machine.handle(Input::UserPublish(publish_with_qos(Qos::AtLeastOnce)));

    assert_eq!(actions.len(), 2);
    match &actions[0] {
        Action::SendBytes(bytes) => {
            assert_publish_packet(bytes.as_slice(), 0x32, Some(1), &[PAYLOAD]);
        }
        action => panic!("expected send bytes action, got {action:?}"),
    }
    assert_eq!(
        actions[1],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );
}

#[test]
fn qos1_puback_cancels_timer_and_returns_idle() {
    let mut machine = connected_machine();
    let _ = machine.handle(Input::UserPublish(publish_with_qos(Qos::AtLeastOnce)));

    let actions = machine.handle(Input::PacketPubAck { packet_id: 1 });

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], Action::CancelTimer(TimerKey::AckTimeout(1)));

    let idle_actions = machine.handle(Input::UserPublish(publish_with_qos(Qos::AtMostOnce)));
    assert_eq!(
        idle_actions.len(),
        1,
        "expected machine to return to idle after matching PUBACK"
    );
}

#[test]
fn qos1_ack_timeout_resends_publish_and_reschedules_timer() {
    let mut machine = connected_machine();
    let _ = machine.handle(Input::UserPublish(publish_with_qos(Qos::AtLeastOnce)));

    let actions = machine.handle(Input::TimerFired(TimerKey::AckTimeout(1)));

    assert_eq!(actions.len(), 2);
    match &actions[0] {
        Action::SendBytes(bytes) => {
            assert_publish_packet(bytes.as_slice(), 0x3A, Some(1), &[PAYLOAD]);
        }
        action => panic!("expected send bytes action, got {action:?}"),
    }
    assert_eq!(
        actions[1],
        Action::ScheduleTimer {
            key: TimerKey::AckTimeout(1),
            delay_ms: 5_000,
        }
    );
}

#[test]
fn qos0_publish_encodes_remaining_length_as_vbi_for_payload_over_127() {
    let mut machine = connected_machine();
    let mut publish = publish_with_qos(Qos::AtMostOnce);
    publish.payload.clear();
    for _ in 0..130 {
        publish.payload.push(PAYLOAD);
    }

    let actions = machine.handle(Input::UserPublish(publish));

    assert_eq!(actions.len(), 1);
    let bytes = match &actions[0] {
        Action::SendBytes(bytes) => bytes.as_slice(),
        action => panic!("expected send bytes action, got {action:?}"),
    };

    assert_eq!(bytes[0], 0x30);
    let (remaining_len, remaining_len_bytes) = decode_remaining_length(bytes);
    assert_eq!(remaining_len, 144);
    assert_eq!(remaining_len_bytes, 2);
    assert_eq!(&bytes[1..3], &[0x90, 0x01]);
}
