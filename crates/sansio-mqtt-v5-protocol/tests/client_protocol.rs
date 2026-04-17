use bytes::Bytes;
use core::num::NonZero;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_v5_protocol::{
    Client, ClientMessage, Config, ConnectionOptions, DriverEventIn, Error, SubscribeOptions,
    UserWriteIn, UserWriteOut,
};
use sansio_mqtt_v5_types::{
    ConnAck, ConnAckKind, ConnAckProperties, ConnackReasonCode, ControlPacket, Disconnect,
    DisconnectProperties, DisconnectReasonCode, Payload, Publish, PublishKind, PublishProperties,
    Qos, Settings, Topic, Utf8String,
};

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

#[test]
fn client_message_exposes_qos_field() {
    let message = ClientMessage::default();
    let _: Qos = message.qos;

    assert_eq!(message.qos, Qos::AtMostOnce);
}

#[test]
fn config_and_error_are_instantiable() {
    let config = Config::default();
    let settings = Settings::default();
    assert_eq!(config.parser_max_bytes_string, settings.max_bytes_string);
    assert_eq!(
        config.parser_max_bytes_binary_data,
        settings.max_bytes_binary_data
    );
    assert_eq!(
        config.parser_max_remaining_bytes,
        settings.max_remaining_bytes
    );
    assert_eq!(
        config.parser_max_subscriptions_len,
        settings.max_subscriptions_len
    );
    assert_eq!(
        config.parser_max_user_properties_len,
        settings.max_user_properties_len
    );

    let malformed = Error::MalformedPacket;
    let protocol = Error::ProtocolError;
    let invalid_state = Error::InvalidStateTransition;
    let unsupported_qos = Error::UnsupportedQosForMvp {
        qos: Qos::AtLeastOnce,
    };
    let packet_too_large = Error::PacketTooLarge;
    let receive_maximum_exceeded = Error::ReceiveMaximumExceeded;
    let encode_failure = Error::EncodeFailure;

    assert_eq!(malformed.to_string(), "malformed packet");
    assert_eq!(protocol.to_string(), "protocol error");
    assert_eq!(invalid_state.to_string(), "invalid state transition");
    assert_eq!(
        unsupported_qos.to_string(),
        "unsupported qos for mvp: AtLeastOnce"
    );
    assert_eq!(packet_too_large.to_string(), "packet too large");
    assert_eq!(
        receive_maximum_exceeded.to_string(),
        "receive maximum exceeded"
    );
    assert_eq!(encode_failure.to_string(), "encode failure");
}

#[test]
fn socket_connected_emits_connect_bytes() {
    let mut client = Client::<u64>::default();

    let result = client.handle_event(DriverEventIn::SocketConnected);

    assert_eq!(result, Ok(()));
    assert_eq!(
        client.poll_write(),
        Some(bytes::Bytes::from_static(&[
            0x10, 0x0d, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x05, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00,
        ]))
    );
}

#[test]
fn socket_closed_emits_disconnected_event() {
    let mut client = Client::<u64>::default();

    let result = client.handle_event(DriverEventIn::SocketClosed);

    assert_eq!(result, Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));
}

#[test]
fn socket_connected_in_connecting_state_returns_invalid_transition() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let result = client.handle_event(DriverEventIn::SocketConnected);

    assert_eq!(result, Err(Error::InvalidStateTransition));
    assert_eq!(client.poll_write(), None);
}

#[test]
fn fragmented_packet_is_buffered_until_complete() {
    let mut client = Client::<u64>::default();

    let first_fragment = client.handle_read(Bytes::from_static(&[0xD0]));

    assert_eq!(first_fragment, Ok(()));
    assert_eq!(client.poll_write(), None);
    assert_eq!(client.poll_event(), None);

    let second_fragment = client.handle_read(Bytes::from_static(&[0x00]));

    assert_eq!(second_fragment, Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
}

#[test]
fn malformed_packet_triggers_close_action() {
    let mut client = Client::<u64>::default();

    let result = client.handle_read(Bytes::from_static(&[0xD0, 0x01, 0x00]));

    assert_eq!(result, Err(Error::MalformedPacket));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x81, 0x00]))
    );
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
}

#[test]
fn protocol_error_emits_disconnect_bytes_before_close_action_polling() {
    let mut client = Client::<u64>::default();

    let result = client.handle_read(Bytes::from_static(&[0xD0, 0x00]));

    assert_eq!(result, Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
    assert_eq!(client.poll_write(), None);
    assert_eq!(client.poll_event(), None);
}

#[test]
fn connack_transitions_to_connected_and_emits_connected() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));
}

#[test]
fn connack_rejected_reason_closes_without_connected_event() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::NotAuthorized,
        },
        properties: ConnAckProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&connack)),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_read(), None);
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
}

#[test]
fn inbound_publish_qos0_is_forwarded_to_user_queue() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let publish_topic = Topic::try_from(Utf8String::try_from("sensors/temp").expect("valid utf8"))
        .expect("valid topic");
    let publish_payload = Payload::from(&b"27.5"[..]);

    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: publish_payload.clone(),
        topic: publish_topic.clone(),
        properties: PublishProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));

    match client.poll_read() {
        Some(UserWriteOut::ReceivedMessage(message)) => {
            assert_eq!(message.topic, publish_topic);
            assert_eq!(message.payload, publish_payload);
            assert_eq!(message.payload_format_indicator, None);
            assert_eq!(message.message_expiry_interval, None);
            assert_eq!(message.topic_alias, None);
            assert_eq!(message.response_topic, None);
            assert_eq!(message.correlation_data, None);
            assert_eq!(message.subscription_identifier, None);
            assert_eq!(message.content_type, None);
            assert!(message.user_properties.is_empty());
        }
        other => panic!("expected received message, got {other:?}"),
    }
}

#[test]
fn socket_closed_after_disconnect_does_not_duplicate_disconnected_event() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let disconnect = ControlPacket::Disconnect(Disconnect {
        reason_code: DisconnectReasonCode::NormalDisconnection,
        properties: DisconnectProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&disconnect)), Ok(()));
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));
    assert_eq!(client.poll_read(), None);
}

#[test]
fn publish_rejects_qos1_and_qos2_in_mvp() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");

    let qos1 = ClientMessage {
        topic: topic.clone(),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1)),
        Err(Error::UnsupportedQosForMvp {
            qos: Qos::AtLeastOnce
        })
    );
    assert_eq!(client.poll_write(), None);

    let qos2 = ClientMessage {
        topic,
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos2)),
        Err(Error::UnsupportedQosForMvp {
            qos: Qos::ExactlyOnce
        })
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn publish_rejects_packet_exceeding_connack_maximum_packet_size() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_packet_size: NonZero::new(16),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let message = ClientMessage {
        topic,
        qos: Qos::AtMostOnce,
        payload: Payload::from(vec![0; 64]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(message)),
        Err(Error::PacketTooLarge)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_rejects_packet_exceeding_connack_maximum_packet_size() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_packet_size: NonZero::new(20),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let subscriptions = vec![Utf8String::try_from("a/very/long/topic/filter").expect("valid utf8")]
        .try_into()
        .expect("non-empty subscriptions");
    let subscribe = SubscribeOptions {
        subscriptions,
        qos: Qos::AtMostOnce,
        no_local: false,
        retain_as_published: false,
        retain_handling: 0,
        subscription_identifier: None,
        user_properties: Vec::new(),
    };

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(subscribe)),
        Err(Error::PacketTooLarge)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn repeated_connect_does_not_duplicate_open_socket_event() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );
    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );

    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
    assert_eq!(client.poll_event(), None);
}

#[test]
fn rejected_connect_does_not_mutate_pending_connect_options() {
    let mut client = Client::<u64>::default();

    let initial_options = ConnectionOptions {
        client_identifier: Utf8String::try_from("initial-client").expect("valid utf8"),
        ..ConnectionOptions::default()
    };
    let rejected_options = ConnectionOptions {
        client_identifier: Utf8String::try_from("rejected-client").expect("valid utf8"),
        ..ConnectionOptions::default()
    };

    let mut rejected_connect_reference = Client::<u64>::default();
    assert_eq!(
        rejected_connect_reference.handle_write(UserWriteIn::Connect(rejected_options.clone())),
        Ok(())
    );
    assert_eq!(
        rejected_connect_reference.handle_event(DriverEventIn::SocketConnected),
        Ok(())
    );
    let rejected_connect_bytes = rejected_connect_reference
        .poll_write()
        .expect("connect bytes are queued");

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(initial_options)),
        Ok(())
    );
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let first_connect_bytes = client.poll_write().expect("connect bytes are queued");

    assert_ne!(first_connect_bytes, rejected_connect_bytes);

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(rejected_options)),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let reconnect_bytes = client.poll_write().expect("connect bytes are queued");

    assert_eq!(reconnect_bytes, first_connect_bytes);
}

#[test]
fn reconnect_ignores_previous_connack_maximum_packet_size_for_connect() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let first_connect = client.poll_write().expect("connect bytes are queued");

    let connack_with_tiny_limit = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_packet_size: NonZero::new(8),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(
        client.handle_read(encode_packet(&connack_with_tiny_limit)),
        Ok(())
    );
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let reconnect_connect = client.poll_write().expect("connect bytes are queued");

    assert_eq!(reconnect_connect, first_connect);
}

#[test]
fn timeout_in_connected_state_enqueues_pingreq() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    assert_eq!(client.handle_timeout(42), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xC0, 0x00])));
    assert_eq!(client.poll_timeout(), Some(42));
}

#[test]
fn close_enqueues_disconnect_and_close_socket() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    assert_eq!(client.close(), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));
    assert_eq!(client.poll_read(), None);

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert_eq!(client.poll_read(), None);
}

#[test]
fn close_succeeds_even_when_disconnect_packet_exceeds_maximum_packet_size() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_packet_size: NonZero::new(1),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    assert_eq!(client.close(), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));
    assert_eq!(client.poll_read(), None);

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert_eq!(client.poll_read(), None);
}

#[test]
fn user_disconnect_succeeds_even_when_disconnect_packet_exceeds_maximum_packet_size() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_packet_size: NonZero::new(1),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    assert_eq!(client.handle_write(UserWriteIn::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));
    assert_eq!(client.poll_read(), None);

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert_eq!(client.poll_read(), None);
}

#[test]
fn timeout_is_cleared_on_close() {
    let mut close_client = Client::<u64>::default();

    assert_eq!(
        close_client.handle_event(DriverEventIn::SocketConnected),
        Ok(())
    );
    assert!(close_client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(close_client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(close_client.poll_read(), Some(UserWriteOut::Connected));

    assert_eq!(close_client.handle_timeout(42), Ok(()));
    assert_eq!(close_client.poll_timeout(), Some(42));

    assert_eq!(close_client.close(), Ok(()));
    assert_eq!(close_client.poll_timeout(), None);

    let mut socket_closed_client = Client::<u64>::default();

    assert_eq!(
        socket_closed_client.handle_event(DriverEventIn::SocketConnected),
        Ok(())
    );
    assert!(socket_closed_client.poll_write().is_some());
    assert_eq!(
        socket_closed_client.handle_read(encode_packet(&connack)),
        Ok(())
    );
    assert_eq!(
        socket_closed_client.poll_read(),
        Some(UserWriteOut::Connected)
    );

    assert_eq!(socket_closed_client.handle_timeout(99), Ok(()));
    assert_eq!(socket_closed_client.poll_timeout(), Some(99));

    assert_eq!(
        socket_closed_client.handle_event(DriverEventIn::SocketClosed),
        Ok(())
    );
    assert_eq!(socket_closed_client.poll_timeout(), None);
}
