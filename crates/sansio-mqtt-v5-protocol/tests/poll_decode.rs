use core::time::Duration;
use std::string::String;
use std::vec::Vec;

use sansio::Protocol;
use sansio_mqtt_v5_contract::{Action, ConnectOptions, ProtocolError, SubscribeRequest};
use sansio_mqtt_v5_protocol::{MqttProtocol, ProtocolEvent};
use sansio_mqtt_v5_types::{Qos, Topic, Utf8String};

fn connect_options(connect_timeout_ms: u32, keep_alive_secs: Option<u16>) -> ConnectOptions {
    ConnectOptions {
        connect_timeout: Duration::from_millis(u64::from(connect_timeout_ms)),
        keep_alive: keep_alive_secs.map(|s| Duration::from_secs(u64::from(s))),
        ..ConnectOptions::default()
    }
}

fn bytes(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

#[test]
fn user_connect_enqueues_connect_bytes_and_connect_timeout() {
    let mut protocol = MqttProtocol::new();

    let result = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(4_321, Some(30))),
    );

    assert_eq!(result, Ok(()));
    assert_eq!(
        Protocol::poll_write(&mut protocol),
        Some(bytes(&[0x10, 0x00]))
    );
    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(4_321));
}

#[test]
fn connack_read_cancels_connect_timeout_and_schedules_keepalive() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(10_000, Some(7))),
    );
    let _ = Protocol::poll_write(&mut protocol);

    let result = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));

    assert_eq!(result, Ok(()));
    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(7_000));
}

#[test]
fn timer_actions_update_deadlines_across_timeout_and_read_paths() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(1_000, Some(3))),
    );
    let _ = Protocol::poll_write(&mut protocol);
    let _ = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));

    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(3_000));

    let timeout_result = Protocol::handle_timeout(&mut protocol, 3_000);
    assert_eq!(timeout_result, Ok(()));
    assert_eq!(
        Protocol::poll_write(&mut protocol),
        Some(bytes(&[0xC0, 0x00]))
    );
    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(13_000));

    let pingresp_result = Protocol::handle_read(&mut protocol, bytes(&[0xD0, 0x00]));
    assert_eq!(pingresp_result, Ok(()));
    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(6_000));
}

#[test]
fn pingresp_timeout_emits_disconnect_bytes_and_session_event() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(1_000, Some(5))),
    );
    let _ = Protocol::poll_write(&mut protocol);
    let _ = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));
    let _ = Protocol::handle_timeout(&mut protocol, 5_000);
    let _ = Protocol::poll_write(&mut protocol);

    let timeout_result = Protocol::handle_timeout(&mut protocol, 15_000);

    assert_eq!(timeout_result, Ok(()));
    assert_eq!(
        Protocol::poll_write(&mut protocol),
        Some(bytes(&[0xE0, 0x00]))
    );
    assert_eq!(
        Protocol::poll_event(&mut protocol),
        Some(Action::DisconnectedByTimeout)
    );
    assert_eq!(Protocol::poll_timeout(&mut protocol), None);
}

#[test]
fn puback_read_dispatches_ack_timeout_cancel() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(1_000, Some(30))),
    );
    let _ = Protocol::poll_write(&mut protocol);
    let _ = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));

    let publish = sansio_mqtt_v5_contract::PublishRequest {
        topic: Topic::try_from(Utf8String::try_from("a/b").expect("valid utf8"))
            .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        ..sansio_mqtt_v5_contract::PublishRequest::default()
    };
    let _ = Protocol::handle_event(&mut protocol, ProtocolEvent::Publish(publish));
    let _ = Protocol::poll_write(&mut protocol);
    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(5_000));

    let result = Protocol::handle_read(&mut protocol, bytes(&[0x40, 0x02, 0x00, 0x01]));

    assert_eq!(result, Ok(()));
    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(30_000));
}

#[test]
fn suback_read_dispatches_session_event() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(1_000, Some(30))),
    );
    let _ = Protocol::poll_write(&mut protocol);
    let _ = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));

    let subscribe = SubscribeRequest {
        topic_filter: Utf8String::try_from("sensor/#").expect("valid filter"),
        ..SubscribeRequest::default()
    };
    let _ = Protocol::handle_event(&mut protocol, ProtocolEvent::Subscribe(subscribe));
    let _ = Protocol::poll_write(&mut protocol);

    let result = Protocol::handle_read(&mut protocol, bytes(&[0x90, 0x04, 0x00, 0x01, 0x00, 0x00]));

    assert_eq!(result, Ok(()));
    assert_eq!(
        Protocol::poll_event(&mut protocol),
        Some(Action::SubscribeAck {
            packet_id: 1,
            reason_codes: bytes(&[0x00]),
        })
    );
    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(30_000));
}

#[test]
fn qos1_publish_read_sends_puback_and_dispatches_publish_received() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(1_000, Some(30))),
    );
    let _ = Protocol::poll_write(&mut protocol);
    let _ = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));

    let result = Protocol::handle_read(
        &mut protocol,
        bytes(&[
            0x32, 0x0A, 0x00, 0x03, b'a', b'/', b'b', 0x00, 0x2A, 0x00, b'h', b'i',
        ]),
    );

    assert_eq!(result, Ok(()));
    assert_eq!(
        Protocol::poll_write(&mut protocol),
        Some(bytes(&[0x40, 0x02, 0x00, 0x2A]))
    );

    let topic = String::from("a/b");
    assert_eq!(
        Protocol::poll_event(&mut protocol),
        Some(Action::PublishReceived {
            topic,
            payload: bytes(b"hi"),
        })
    );
}

#[test]
fn qos2_publish_read_enters_pubrec_flow() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(1_000, Some(30))),
    );
    let _ = Protocol::poll_write(&mut protocol);
    let _ = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));

    let publish_result = Protocol::handle_read(
        &mut protocol,
        bytes(&[
            0x34, 0x09, 0x00, 0x03, b'a', b'/', b'b', 0x12, 0x34, 0x00, b'z',
        ]),
    );

    assert_eq!(publish_result, Ok(()));
    assert_eq!(
        Protocol::poll_write(&mut protocol),
        Some(bytes(&[0x50, 0x02, 0x12, 0x34]))
    );

    let pubrel_result = Protocol::handle_read(&mut protocol, bytes(&[0x62, 0x02, 0x12, 0x34]));

    assert_eq!(pubrel_result, Ok(()));
    assert_eq!(
        Protocol::poll_write(&mut protocol),
        Some(bytes(&[0x70, 0x02, 0x12, 0x34]))
    );

    let topic = String::from("a/b");
    assert_eq!(
        Protocol::poll_event(&mut protocol),
        Some(Action::PublishReceived {
            topic,
            payload: bytes(b"z"),
        })
    );
}

#[test]
fn malformed_qos1_publish_without_packet_id_returns_decode_error() {
    let mut protocol = MqttProtocol::new();

    let result = Protocol::handle_read(
        &mut protocol,
        bytes(&[0x32, 0x06, 0x00, 0x03, b'a', b'/', b'b', 0x00]),
    );

    assert_eq!(result, Err(ProtocolError::DecodeError));
}

#[test]
fn suback_read_parses_multi_byte_property_length() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(1_000, Some(30))),
    );
    let _ = Protocol::poll_write(&mut protocol);
    let _ = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));

    let subscribe = SubscribeRequest {
        topic_filter: Utf8String::try_from("sensor/#").expect("valid filter"),
        ..SubscribeRequest::default()
    };
    let _ = Protocol::handle_event(&mut protocol, ProtocolEvent::Subscribe(subscribe));
    let _ = Protocol::poll_write(&mut protocol);

    let mut packet = vec![0x90, 0x85, 0x01, 0x00, 0x01, 0x80, 0x01];
    packet.extend(vec![0x00; 128]);
    packet.push(0x00);

    let result = Protocol::handle_read(&mut protocol, packet);

    assert_eq!(result, Ok(()));
    assert_eq!(
        Protocol::poll_event(&mut protocol),
        Some(Action::SubscribeAck {
            packet_id: 1,
            reason_codes: bytes(&[0x00]),
        })
    );
}

#[test]
fn unknown_packet_type_returns_decode_error() {
    let mut protocol = MqttProtocol::new();

    let result = Protocol::handle_read(&mut protocol, bytes(&[0xF0, 0x00]));

    assert_eq!(
        result,
        Err(sansio_mqtt_v5_contract::ProtocolError::DecodeError)
    );
}

#[test]
fn pubrel_with_invalid_flags_returns_decode_error() {
    let mut protocol = MqttProtocol::new();

    let result = Protocol::handle_read(&mut protocol, bytes(&[0x60, 0x02, 0x12, 0x34]));

    assert_eq!(result, Err(ProtocolError::DecodeError));
}

#[test]
fn connack_and_pingresp_with_invalid_flags_return_decode_error() {
    let mut protocol = MqttProtocol::new();

    let connack_result =
        Protocol::handle_read(&mut protocol, bytes(&[0x21, 0x03, 0x00, 0x00, 0x00]));
    assert_eq!(connack_result, Err(ProtocolError::DecodeError));

    let pingresp_result = Protocol::handle_read(&mut protocol, bytes(&[0xD1, 0x00]));
    assert_eq!(pingresp_result, Err(ProtocolError::DecodeError));
}

#[test]
fn puback_with_inconsistent_remaining_length_returns_decode_error() {
    let mut protocol = MqttProtocol::new();

    let result = Protocol::handle_read(&mut protocol, bytes(&[0x40, 0x03, 0x00, 0x01]));

    assert_eq!(result, Err(ProtocolError::DecodeError));
}

#[test]
fn keepalive_timeout_maps_to_timer_fired_for_expected_timer_key() {
    let mut protocol = MqttProtocol::new();
    let _ = Protocol::handle_event(
        &mut protocol,
        ProtocolEvent::Connect(connect_options(1_000, Some(2))),
    );
    let _ = Protocol::poll_write(&mut protocol);
    let _ = Protocol::handle_read(&mut protocol, bytes(&[0x20, 0x03, 0x00, 0x00, 0x00]));

    assert_eq!(Protocol::poll_timeout(&mut protocol), Some(2_000));
    let _ = Protocol::handle_timeout(&mut protocol, 2_000);
    assert_eq!(
        Protocol::poll_write(&mut protocol),
        Some(bytes(&[0xC0, 0x00]))
    );
}
