use sansio_mqtt_v5_contract::{
    Action, ConnectOptions, DisconnectReason, Input, ProtocolError, PublishRequest, SessionAction,
    SubscribeRequest, TimerKey,
};

#[test]
fn exports_required_boundary_types() {
    let _: Option<Action> = None;
    let _: Option<SessionAction> = None;
    let _: Option<Input> = None;
    let _: Option<ConnectOptions> = None;
    let _: Option<PublishRequest> = None;
    let _: Option<SubscribeRequest> = None;
    let _: Option<TimerKey> = None;
    let _: Option<DisconnectReason> = None;
    let _: Option<ProtocolError> = None;
}

#[test]
fn input_contract_shape_and_basic_construction() {
    let bytes = Input::BytesReceived(&[0x10, 0x00]);
    assert!(matches!(bytes, Input::BytesReceived(&[0x10, 0x00])));

    let timer_elapsed = Input::TimerFired(TimerKey::Keepalive);
    assert!(matches!(
        timer_elapsed,
        Input::TimerFired(TimerKey::Keepalive)
    ));

    let user_connect = Input::UserConnect(ConnectOptions::default());
    assert!(matches!(user_connect, Input::UserConnect(_)));

    let user_publish = Input::UserPublish(PublishRequest::default());
    assert!(matches!(user_publish, Input::UserPublish(_)));

    let user_subscribe = Input::UserSubscribe(SubscribeRequest::default());
    assert!(matches!(user_subscribe, Input::UserSubscribe(_)));

    let user_disconnect = Input::UserDisconnect;
    assert!(matches!(user_disconnect, Input::UserDisconnect));

    let topic = "sensors/temp".to_owned();
    let packet_publish = Input::PacketPublish {
        topic,
        payload: vec![1, 2, 3],
        qos: sansio_mqtt_v5_contract::Qos::AtLeast,
        packet_id: Some(42),
    };
    assert!(matches!(
        packet_publish,
        Input::PacketPublish {
            topic: _,
            payload: _,
            qos: _,
            packet_id: Some(42)
        }
    ));

    let packet_pubrel = Input::PacketPubRel { packet_id: 42 };
    assert!(matches!(
        packet_pubrel,
        Input::PacketPubRel { packet_id: 42 }
    ));
}

#[test]
fn action_contract_shape_and_basic_construction() {
    let send_bytes = Action::SendBytes(vec![0x20, 0x00]);
    assert!(matches!(send_bytes, Action::SendBytes(_)));

    let timer_action = Action::ScheduleTimer {
        key: TimerKey::ConnectTimeout,
        delay_ms: 1_000,
    };
    assert!(matches!(
        timer_action,
        Action::ScheduleTimer {
            key: TimerKey::ConnectTimeout,
            delay_ms: 1_000
        }
    ));

    let cancel_timer = Action::CancelTimer(TimerKey::AckTimeout(7));
    assert!(matches!(
        cancel_timer,
        Action::CancelTimer(TimerKey::AckTimeout(7))
    ));

    let session_action = Action::SessionAction(SessionAction::Connected);
    assert!(matches!(
        session_action,
        Action::SessionAction(SessionAction::Connected)
    ));
}

#[test]
fn session_action_contract_shape_and_basic_construction() {
    let connected = SessionAction::Connected;
    assert!(matches!(connected, SessionAction::Connected));

    let disconnected = SessionAction::Disconnected {
        reason: DisconnectReason::ProtocolError,
    };
    assert!(matches!(
        disconnected,
        SessionAction::Disconnected {
            reason: DisconnectReason::ProtocolError
        }
    ));

    let topic = "sensors/temp".to_owned();

    let publish_received = SessionAction::PublishReceived {
        topic,
        payload: vec![1, 2, 3],
    };
    assert!(matches!(
        publish_received,
        SessionAction::PublishReceived { .. }
    ));

    let subscribe_ack = SessionAction::SubscribeAck {
        packet_id: 42,
        reason_codes: vec![0x00],
    };
    assert!(matches!(
        subscribe_ack,
        SessionAction::SubscribeAck {
            packet_id: 42,
            reason_codes: _
        }
    ));
}

#[test]
fn suback_input_carries_reason_codes() {
    let suback = Input::PacketSubAck {
        packet_id: 7,
        reason_codes: vec![0x00, 0x80],
    };

    assert!(matches!(
        suback,
        Input::PacketSubAck {
            packet_id: 7,
            reason_codes: _
        }
    ));
}

#[test]
fn timer_key_contract_shape() {
    assert!(matches!(TimerKey::Keepalive, TimerKey::Keepalive));
    assert!(matches!(
        TimerKey::PingRespTimeout,
        TimerKey::PingRespTimeout
    ));
    assert!(matches!(TimerKey::AckTimeout(5), TimerKey::AckTimeout(5)));
    assert!(matches!(TimerKey::ConnectTimeout, TimerKey::ConnectTimeout));
}

#[test]
fn protocol_error_contract_shape() {
    assert!(matches!(
        ProtocolError::DecodeError,
        ProtocolError::DecodeError
    ));
    assert!(matches!(
        ProtocolError::UnexpectedPacket,
        ProtocolError::UnexpectedPacket
    ));
    assert!(matches!(ProtocolError::Timeout, ProtocolError::Timeout));
    assert!(matches!(
        ProtocolError::PacketIdExhausted,
        ProtocolError::PacketIdExhausted
    ));
}

#[test]
fn options_are_default_constructible() {
    let connect = ConnectOptions::default();
    let publish = PublishRequest::default();
    let subscribe = SubscribeRequest::default();

    assert_eq!(connect.connect_timeout_ms, 10_000);

    assert!(matches!(
        publish,
        PublishRequest {
            topic: _,
            payload: _,
            retain: _,
            ..
        }
    ));

    assert!(matches!(
        subscribe,
        SubscribeRequest {
            topic_filter: _,
            ..
        }
    ));
}

#[test]
fn subscribe_request_single_accepts_valid_topic_filter() {
    let request = SubscribeRequest::single("sensors/temperature").expect("fits");

    assert_eq!(request.topic_filter.as_str(), "sensors/temperature");
}

#[test]
fn subscribe_request_single_accepts_long_topic_filter() {
    let long_filter = "a".repeat(2_048);

    let request = SubscribeRequest::single(&long_filter).expect("unbounded filter accepted");

    assert_eq!(request.topic_filter, long_filter);
}
