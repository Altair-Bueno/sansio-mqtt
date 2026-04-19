use bytes::Bytes;
use core::num::NonZero;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_v5_protocol::{
    BrokerMessage, Client, ClientMessage, ClientSettings, ConnectionOptions, DriverEventIn,
    DriverEventOut, Error, IncomingRejectReason, SubscribeOptions, UserWriteIn, UserWriteOut,
};
use sansio_mqtt_v5_types::{
    Auth, AuthProperties, AuthReasonCode, ConnAck, ConnAckKind, ConnAckProperties,
    ConnackReasonCode, ControlPacket, Disconnect, DisconnectProperties, DisconnectReasonCode,
    GuaranteedQoS, MaximumQoS, ParserSettings, Payload, PubAck, PubAckProperties, PubAckReasonCode,
    PubComp, PubCompProperties, PubCompReasonCode, PubRec, PubRecProperties, PubRecReasonCode,
    PubRel, PubRelProperties, PubRelReasonCode, Publish, PublishKind, PublishProperties, Qos,
    RetainHandling, Subscription, Topic, Utf8String,
};
use winnow::error::ContextError;
use winnow::Parser;

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

fn make_subscription(topic_filter: &str) -> Subscription {
    Subscription {
        topic_filter: Utf8String::try_from(topic_filter).expect("valid utf8"),
        qos: Qos::AtMostOnce,
        no_local: false,
        retain_as_published: false,
        retain_handling: RetainHandling::SendRetained,
    }
}

#[test]
fn client_message_exposes_qos_field() {
    let message = ClientMessage::default();
    let _: Qos = message.qos;

    assert_eq!(message.qos, Qos::AtMostOnce);
}

#[test]
fn user_write_out_exposes_qos_delivery_events_with_packet_id() {
    let packet_id = NonZero::new(7).expect("non-zero packet id");
    let msg = BrokerMessage::default();

    let no_ack = UserWriteOut::ReceivedMessage(msg.clone());
    assert!(matches!(no_ack, UserWriteOut::ReceivedMessage(_)));

    assert!(matches!(UserWriteOut::Connected, UserWriteOut::Connected));

    let acknowledged = UserWriteOut::PublishAcknowledged(packet_id, PubAckReasonCode::Success);
    let completed = UserWriteOut::PublishCompleted(packet_id, PubCompReasonCode::Success);
    let dropped = UserWriteOut::PublishDroppedDueToSessionNotResumed(packet_id);
    let dropped_by_broker = UserWriteOut::PublishDroppedDueToBrokerRejectedPubRec(
        packet_id,
        PubRecReasonCode::NotAuthorized,
    );

    match acknowledged {
        UserWriteOut::PublishAcknowledged(packet_id, reason_code) => {
            assert_eq!(packet_id.get(), 7);
            assert_eq!(reason_code, PubAckReasonCode::Success);
        }
        other => panic!("expected PublishAcknowledged, got {other:?}"),
    }

    match completed {
        UserWriteOut::PublishCompleted(packet_id, reason_code) => {
            assert_eq!(packet_id.get(), 7);
            assert_eq!(reason_code, PubCompReasonCode::Success);
        }
        other => panic!("expected PublishCompleted, got {other:?}"),
    }

    match dropped {
        UserWriteOut::PublishDroppedDueToSessionNotResumed(packet_id) => {
            assert_eq!(packet_id.get(), 7);
        }
        other => panic!("expected PublishDroppedDueToSessionNotResumed, got {other:?}"),
    }

    match dropped_by_broker {
        UserWriteOut::PublishDroppedDueToBrokerRejectedPubRec(packet_id, reason_code) => {
            assert_eq!(packet_id.get(), 7);
            assert_eq!(reason_code, PubRecReasonCode::NotAuthorized);
        }
        other => panic!("expected PublishDroppedDueToBrokerRejectedPubRec, got {other:?}"),
    }
}

#[test]
fn driver_events_are_pattern_matchable_without_equality() {
    let incoming = DriverEventIn::SocketConnected;
    let outgoing = DriverEventOut::OpenSocket;

    assert!(matches!(incoming, DriverEventIn::SocketConnected));
    assert!(matches!(outgoing, DriverEventOut::OpenSocket));
}

#[test]
fn client_new_uses_default_state_and_blank_scratchpad() {
    let _client = Client::<u64>::with_settings(ClientSettings::default());
}

#[test]
fn client_new_with_state_accepts_preloaded_state() {
    let state = sansio_mqtt_v5_protocol::ClientState::default();
    let _client = Client::<u64>::with_settings_and_state(ClientSettings::default(), state);
}

#[test]
fn clean_start_true_drops_preloaded_state() {
    let mut client = Client::<u64>::with_settings_and_state(
        ClientSettings::default(),
        sansio_mqtt_v5_protocol::ClientState::default(),
    );

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            clean_start: true,
            session_expiry_interval: Some(60),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );

    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
}

#[test]
fn clean_start_false_keeps_preloaded_state_until_session_rules_clear_it() {
    let mut client = Client::<u64>::with_settings_and_state(
        ClientSettings::default(),
        sansio_mqtt_v5_protocol::ClientState::default(),
    );

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            clean_start: false,
            session_expiry_interval: Some(60),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );

    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
}

#[test]
fn config_and_error_are_instantiable() {
    let config = ClientSettings::default();
    let settings = ParserSettings::default();
    assert_eq!(config.max_bytes_string, settings.max_bytes_string);
    assert_eq!(config.max_bytes_binary_data, settings.max_bytes_binary_data);
    assert_eq!(config.max_remaining_bytes, settings.max_remaining_bytes);
    assert_eq!(config.max_subscriptions_len, settings.max_subscriptions_len);
    assert_eq!(
        config.max_user_properties_len,
        settings.max_user_properties_len
    );

    let malformed = Error::MalformedPacket;
    let protocol = Error::ProtocolError;
    let invalid_state = Error::InvalidStateTransition;
    let packet_too_large = Error::PacketTooLarge;
    let receive_maximum_exceeded = Error::ReceiveMaximumExceeded;
    let encode_failure = Error::EncodeFailure;

    let classify = |error: Error| -> &'static str {
        match error {
            Error::MalformedPacket => "malformed packet",
            Error::ProtocolError => "protocol error",
            Error::InvalidStateTransition => "invalid state transition",
            Error::PacketTooLarge => "packet too large",
            Error::ReceiveMaximumExceeded => "receive maximum exceeded",
            Error::EncodeFailure => "encode failure",
        }
    };

    assert_eq!(classify(malformed), "malformed packet");
    assert_eq!(classify(protocol), "protocol error");
    assert_eq!(classify(invalid_state), "invalid state transition");
    assert_eq!(classify(packet_too_large), "packet too large");
    assert_eq!(
        classify(receive_maximum_exceeded),
        "receive maximum exceeded"
    );
    assert_eq!(classify(encode_failure), "encode failure");
}

#[test]
fn client_settings_default_includes_permissive_negotiation_policy() {
    let settings = ClientSettings::default();

    assert!(settings.max_incoming_receive_maximum.is_none());
    assert!(settings.max_incoming_packet_size.is_none());
    assert!(settings.max_incoming_topic_alias_maximum.is_none());
    assert!(settings.max_outgoing_qos.is_none());
    assert!(settings.allow_retain);
    assert!(settings.allow_wildcard_subscriptions);
    assert!(settings.allow_shared_subscriptions);
    assert!(settings.allow_subscription_identifiers);
    assert!(settings.default_request_response_information.is_none());
    assert!(settings.default_request_problem_information.is_none());
    assert!(settings.default_keep_alive.is_none());
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
fn connect_encodes_receive_maximum_when_configured() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            receive_maximum: NonZero::new(42),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let connect_bytes = client.poll_write().expect("connect frame expected");

    let packet = ControlPacket::parser::<_, ContextError, ContextError>(&ParserSettings::default())
        .parse(connect_bytes.as_ref())
        .expect("connect packet should decode");

    let connect = match packet {
        ControlPacket::Connect(connect) => connect,
        other => panic!("expected CONNECT, got {other:?}"),
    };

    assert_eq!(connect.properties.receive_maximum, NonZero::new(42));
}

#[test]
fn connect_encodes_maximum_packet_size_when_configured() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            maximum_packet_size: NonZero::new(8192),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let connect_bytes = client.poll_write().expect("connect frame expected");

    let packet = ControlPacket::parser::<_, ContextError, ContextError>(&ParserSettings::default())
        .parse(connect_bytes.as_ref())
        .expect("connect packet should decode");

    let connect = match packet {
        ControlPacket::Connect(connect) => connect,
        other => panic!("expected CONNECT, got {other:?}"),
    };

    assert_eq!(connect.properties.maximum_packet_size, NonZero::new(8192));
}

#[test]
fn parser_uses_effective_client_limits_after_connect_policy_applied() {
    // Small remaining-length cap intentionally below minimum CONNACK frame size.
    let settings = ClientSettings {
        max_remaining_bytes: 2,
        ..ClientSettings::default()
    };
    let mut client = Client::<u64>::with_settings(settings);

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&connack)),
        Err(Error::MalformedPacket)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x81, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::CloseSocket)
    ));
}

#[test]
fn connect_defaults_use_client_settings_when_connection_options_omit_values() {
    let settings = ClientSettings {
        default_keep_alive: NonZero::new(30),
        default_request_response_information: Some(true),
        default_request_problem_information: Some(false),
        max_incoming_receive_maximum: NonZero::new(7),
        max_incoming_packet_size: NonZero::new(1024),
        max_incoming_topic_alias_maximum: Some(3),
        ..ClientSettings::default()
    };
    let mut client = Client::<u64>::with_settings(settings);

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            receive_maximum: NonZero::new(42),
            maximum_packet_size: NonZero::new(8192),
            topic_alias_maximum: Some(10),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let connect_bytes = client.poll_write().expect("connect frame expected");

    let packet = ControlPacket::parser::<_, ContextError, ContextError>(&ParserSettings::default())
        .parse(connect_bytes.as_ref())
        .expect("connect packet should decode");

    let connect = match packet {
        ControlPacket::Connect(connect) => connect,
        other => panic!("expected CONNECT, got {other:?}"),
    };

    assert_eq!(connect.keep_alive, NonZero::new(30));
    assert_eq!(connect.properties.request_response_information, Some(true));
    assert_eq!(connect.properties.request_problem_information, Some(false));
    assert_eq!(connect.properties.receive_maximum, NonZero::new(7));
    assert_eq!(connect.properties.maximum_packet_size, NonZero::new(1024));
    assert_eq!(connect.properties.topic_alias_maximum, Some(3));
}

#[test]
fn connect_topic_alias_defaults_to_client_settings_when_omitted() {
    let settings = ClientSettings {
        max_incoming_topic_alias_maximum: Some(3),
        ..ClientSettings::default()
    };
    let mut client = Client::<u64>::with_settings(settings);

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let connect_bytes = client.poll_write().expect("connect frame expected");

    let packet = ControlPacket::parser::<_, ContextError, ContextError>(&ParserSettings::default())
        .parse(connect_bytes.as_ref())
        .expect("connect packet should decode");
    let connect = match packet {
        ControlPacket::Connect(connect) => connect,
        other => panic!("expected CONNECT, got {other:?}"),
    };

    assert_eq!(connect.properties.topic_alias_maximum, Some(3));
}

#[test]
fn socket_closed_emits_disconnected_event() {
    let mut client = Client::<u64>::default();

    let result = client.handle_event(DriverEventIn::SocketClosed);

    assert_eq!(result, Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));
}

#[test]
fn socket_connected_in_connecting_state_returns_invalid_transition() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

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
    assert!(matches!(client.poll_event(), None));

    let second_fragment = client.handle_read(Bytes::from_static(&[0x00]));

    assert_eq!(second_fragment, Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
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
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
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
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert_eq!(client.poll_write(), None);
    assert!(matches!(client.poll_event(), None));
}

#[test]
fn connack_transitions_to_connected_and_emits_connected() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

#[test]
fn connack_rejected_reason_closes_without_connected_event() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

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
    assert!(matches!(client.poll_read(), None));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let publish_topic = Topic::try_new("sensors/temp").expect("valid topic");
    let publish_payload = Payload::new(b"27.5".as_slice());

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
fn inbound_publish_registers_topic_alias_then_resolves_alias_only_publish() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            topic_alias_maximum: Some(10),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let alias = NonZero::new(1).expect("non-zero alias");
    let topic = Topic::try_new("alias/topic").expect("valid topic");

    let register_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"first".as_slice()),
        topic: topic.clone(),
        properties: PublishProperties {
            topic_alias: Some(alias),
            ..PublishProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&register_publish)), Ok(()));

    match client.poll_read() {
        Some(UserWriteOut::ReceivedMessage(message)) => {
            assert_eq!(message.topic, topic);
            assert_eq!(message.payload, Payload::new(b"first".as_slice()));
            assert_eq!(message.topic_alias, Some(alias));
        }
        other => panic!("expected received message, got {other:?}"),
    }

    let alias_only_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"second".as_slice()),
        topic: Topic::try_new("").expect("valid topic"),
        properties: PublishProperties {
            topic_alias: Some(alias),
            ..PublishProperties::default()
        },
    });
    assert_eq!(
        client.handle_read(encode_packet(&alias_only_publish)),
        Ok(())
    );

    match client.poll_read() {
        Some(UserWriteOut::ReceivedMessage(message)) => {
            assert_eq!(
                message.topic,
                Topic::try_new("alias/topic").expect("valid topic")
            );
            assert_eq!(message.payload, Payload::new(b"second".as_slice()));
            assert_eq!(message.topic_alias, Some(alias));
        }
        other => panic!("expected received message, got {other:?}"),
    }
}

#[test]
fn inbound_publish_alias_only_unknown_alias_is_protocol_error() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            topic_alias_maximum: Some(10),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let unknown_alias_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"unknown".as_slice()),
        topic: Topic::try_new("").expect("valid topic"),
        properties: PublishProperties {
            topic_alias: Some(NonZero::new(1).expect("non-zero alias")),
            ..PublishProperties::default()
        },
    });

    assert_eq!(
        client.handle_read(encode_packet(&unknown_alias_publish)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn inbound_publish_empty_topic_without_alias_is_protocol_error() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let invalid_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"invalid".as_slice()),
        topic: Topic::try_new("").expect("valid topic"),
        properties: PublishProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&invalid_publish)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn inbound_publish_alias_exceeds_client_alias_max_is_protocol_error() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            topic_alias_maximum: Some(1),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let alias_too_large_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"value".as_slice()),
        topic: Topic::try_new("alias/topic").expect("valid topic"),
        properties: PublishProperties {
            topic_alias: Some(NonZero::new(2).expect("non-zero alias")),
            ..PublishProperties::default()
        },
    });

    assert_eq!(
        client.handle_read(encode_packet(&alias_too_large_publish)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn inbound_qos1_publish_waits_for_app_ack_then_sends_puback() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let packet_id = NonZero::new(7).expect("non-zero packet id");
    let publish_topic = Topic::try_new("sensors/temp").expect("valid topic");
    let publish_payload = Payload::new(b"27.5".as_slice());
    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: false,
        payload: publish_payload.clone(),
        topic: publish_topic.clone(),
        properties: PublishProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));

    let inbound_message_id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, message)) => {
            assert_eq!(message.topic, publish_topic);
            assert_eq!(message.payload, publish_payload);
            id
        }
        other => panic!("expected received message, got {other:?}"),
    };

    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(UserWriteIn::AcknowledgeMessage(inbound_message_id)),
        Ok(())
    );

    let expected_puback = ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn inbound_qos1_publish_reject_sends_puback_failure_reason() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let packet_id = NonZero::new(11).expect("non-zero packet id");
    let publish_topic = Topic::try_new("sensors/humidity").expect("valid topic");
    let publish_payload = Payload::new(b"42".as_slice());
    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: false,
        payload: publish_payload.clone(),
        topic: publish_topic.clone(),
        properties: PublishProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));

    let inbound_message_id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, message)) => {
            assert_eq!(message.topic, publish_topic);
            assert_eq!(message.payload, publish_payload);
            id
        }
        other => panic!("expected received message, got {other:?}"),
    };

    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(UserWriteIn::RejectMessage(
            inbound_message_id,
            IncomingRejectReason::NotAuthorized,
        )),
        Ok(())
    );

    let expected_puback = ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code: PubAckReasonCode::NotAuthorized,
        properties: PubAckProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn inbound_qos2_publish_waits_for_app_ack_then_sends_pubrec_and_completes_on_pubrel() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let packet_id = NonZero::new(13).expect("non-zero packet id");
    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::new(b"qos2".as_slice()),
        topic: Topic::try_new("sensors/pressure").expect("valid topic"),
        properties: PublishProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));
    let inbound_message_id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
        other => panic!("expected received message with acknowledgement, got {other:?}"),
    };
    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(UserWriteIn::AcknowledgeMessage(inbound_message_id)),
        Ok(())
    );

    let expected_pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::Success,
        properties: PubRecProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));

    let pubrel = ControlPacket::PubRel(PubRel {
        packet_id,
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubrel)), Ok(()));

    let expected_pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubcomp)));
    assert!(matches!(client.poll_event(), None));
}

#[test]
fn inbound_qos2_publish_reject_sends_pubrec_failure_and_clears_state() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let packet_id = NonZero::new(13).expect("non-zero packet id");
    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::new(b"qos2".as_slice()),
        topic: Topic::try_new("sensors/pressure").expect("valid topic"),
        properties: PublishProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));
    let inbound_message_id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
        other => panic!("expected received message with acknowledgement, got {other:?}"),
    };
    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(UserWriteIn::RejectMessage(
            inbound_message_id,
            IncomingRejectReason::QuotaExceeded,
        )),
        Ok(())
    );

    let expected_pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::QuotaExceeded,
        properties: PubRecProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));

    let pubrel = ControlPacket::PubRel(PubRel {
        packet_id,
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubrel)), Ok(()));

    let expected_pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::PacketIdentifierNotFound,
        properties: PubCompProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubcomp)));
    assert!(matches!(client.poll_event(), None));
}

#[test]
fn inbound_packet_id_reuse_conflict_causes_protocol_error() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let packet_id = NonZero::new(19).expect("non-zero packet id");
    let qos1_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::new(b"qos1".as_slice()),
        topic: Topic::try_new("state/conflict").expect("valid topic"),
        properties: PublishProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&qos1_publish)), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(
            _,
            _
        ))
    ));

    let qos2_same_packet_id = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::new(b"qos2".as_slice()),
        topic: Topic::try_new("state/conflict").expect("valid topic"),
        properties: PublishProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&qos2_same_packet_id)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn duplicate_qos2_publish_after_reject_resends_same_failure_pubrec_without_redelivery() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let packet_id = NonZero::new(23).expect("non-zero packet id");
    let first_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::new(b"first".as_slice()),
        topic: Topic::try_new("state/reject").expect("valid topic"),
        properties: PublishProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&first_publish)), Ok(()));
    let inbound_message_id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
        other => panic!("expected received message with acknowledgement, got {other:?}"),
    };
    assert_eq!(
        client.handle_write(UserWriteIn::RejectMessage(
            inbound_message_id,
            IncomingRejectReason::NotAuthorized,
        )),
        Ok(())
    );

    let expected_pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::NotAuthorized,
        properties: PubRecProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));

    let duplicate_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: true,
        },
        retain: false,
        payload: Payload::new(b"duplicate".as_slice()),
        topic: Topic::try_new("state/reject").expect("valid topic"),
        properties: PublishProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&duplicate_publish)),
        Ok(())
    );
    assert!(matches!(client.poll_read(), None));
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));
}

#[test]
fn manual_ack_sends_puback_success_for_pending_message_id() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let packet_id = NonZero::new(77).expect("non-zero packet id");
    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::new(b"unknown".as_slice()),
        topic: Topic::try_new("unknown/id").expect("valid topic"),
        properties: PublishProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));

    let inbound_message_id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
        other => panic!("expected received message with acknowledgement, got {other:?}"),
    };

    assert_eq!(
        client.handle_write(UserWriteIn::AcknowledgeMessage(inbound_message_id)),
        Ok(())
    );

    let expected_puback = ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));

    assert!(matches!(client.poll_write(), None));
}

#[test]
fn reject_reason_maps_to_puback_failure_codes_for_qos1() {
    let cases = [
        (
            IncomingRejectReason::UnspecifiedError,
            PubAckReasonCode::UnspecifiedError,
        ),
        (
            IncomingRejectReason::ImplementationSpecificError,
            PubAckReasonCode::ImplementationSpecificError,
        ),
        (
            IncomingRejectReason::NotAuthorized,
            PubAckReasonCode::NotAuthorized,
        ),
        (
            IncomingRejectReason::TopicNameInvalid,
            PubAckReasonCode::TopicNameInvalid,
        ),
        (
            IncomingRejectReason::QuotaExceeded,
            PubAckReasonCode::QuotaExceeded,
        ),
        (
            IncomingRejectReason::PayloadFormatInvalid,
            PubAckReasonCode::PayloadFormatInvalid,
        ),
    ];

    for (offset, (reject_reason, expected_reason_code)) in cases.iter().enumerate() {
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
        assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

        let packet_id = NonZero::new((offset + 1) as u16).expect("non-zero packet id");
        let publish = ControlPacket::Publish(Publish {
            kind: PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            },
            retain: false,
            payload: Payload::new(b"mapping".as_slice()),
            topic: Topic::try_new("reject/reason/qos1").expect("valid topic"),
            properties: PublishProperties::default(),
        });

        assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));
        let inbound_message_id = match client.poll_read() {
            Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
            other => panic!("expected received message with acknowledgement, got {other:?}"),
        };

        assert_eq!(
            client.handle_write(UserWriteIn::RejectMessage(
                inbound_message_id,
                *reject_reason
            )),
            Ok(())
        );

        let expected_puback = ControlPacket::PubAck(PubAck {
            packet_id,
            reason_code: *expected_reason_code,
            properties: PubAckProperties::default(),
        });
        assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));
    }
}

#[test]
fn reject_reason_maps_to_pubrec_failure_codes_for_qos2() {
    let cases = [
        (
            IncomingRejectReason::UnspecifiedError,
            PubRecReasonCode::UnspecifiedError,
        ),
        (
            IncomingRejectReason::ImplementationSpecificError,
            PubRecReasonCode::ImplementationSpecificError,
        ),
        (
            IncomingRejectReason::NotAuthorized,
            PubRecReasonCode::NotAuthorized,
        ),
        (
            IncomingRejectReason::TopicNameInvalid,
            PubRecReasonCode::TopicNameInvalid,
        ),
        (
            IncomingRejectReason::QuotaExceeded,
            PubRecReasonCode::QuotaExceeded,
        ),
        (
            IncomingRejectReason::PayloadFormatInvalid,
            PubRecReasonCode::PayloadFormatInvalid,
        ),
    ];

    for (offset, (reject_reason, expected_reason_code)) in cases.iter().enumerate() {
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
        assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

        let packet_id = NonZero::new((offset + 1) as u16).expect("non-zero packet id");
        let publish = ControlPacket::Publish(Publish {
            kind: PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            },
            retain: false,
            payload: Payload::new(b"mapping".as_slice()),
            topic: Topic::try_new("reject/reason/qos2").expect("valid topic"),
            properties: PublishProperties::default(),
        });

        assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));
        let inbound_message_id = match client.poll_read() {
            Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, _)) => id,
            other => panic!("expected received message with acknowledgement, got {other:?}"),
        };

        assert_eq!(
            client.handle_write(UserWriteIn::RejectMessage(
                inbound_message_id,
                *reject_reason
            )),
            Ok(())
        );

        let expected_pubrec = ControlPacket::PubRec(PubRec {
            packet_id,
            reason_code: *expected_reason_code,
            properties: PubRecProperties::default(),
        });
        assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));
    }
}

#[test]
fn inbound_qos2_unknown_pubrel_sends_pubcomp_packet_identifier_not_found() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let unknown_packet_id = NonZero::new(21).expect("non-zero packet id");
    let pubrel = ControlPacket::PubRel(PubRel {
        packet_id: unknown_packet_id,
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&pubrel)), Ok(()));

    let expected_pubcomp = ControlPacket::PubComp(PubComp {
        packet_id: unknown_packet_id,
        reason_code: PubCompReasonCode::PacketIdentifierNotFound,
        properties: PubCompProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubcomp)));
    assert!(matches!(client.poll_event(), None));
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let disconnect = ControlPacket::Disconnect(Disconnect {
        reason_code: DisconnectReasonCode::NormalDisconnection,
        properties: DisconnectProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&disconnect)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn outbound_qos1_publish_emits_acknowledged_event_on_puback() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");

    let qos1_message = ClientMessage {
        topic: topic.clone(),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message.clone())),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: false,
        payload: qos1_message.payload,
        topic,
        properties: PublishProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_publish)));

    let puback = ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&puback)), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::PublishAcknowledged(id, PubAckReasonCode::Success)) if id == packet_id
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn unexpected_puback_without_matching_qos1_transaction_triggers_protocol_error_close() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let packet_id = NonZero::new(42).expect("non-zero packet id");
    let puback = ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&puback)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn qos2_inflight_receiving_puback_triggers_protocol_error_close() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let qos2_message = ClientMessage {
        topic,
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos2_message)),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let puback = ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&puback)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(client.poll_read(), None));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn qos1_inflight_receiving_pubrec_triggers_protocol_error_close() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let qos1_message = ClientMessage {
        topic,
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::Success,
        properties: PubRecProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&pubrec)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(client.poll_read(), None));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn connack_receive_maximum_only_limits_broker_facing_publish_flow() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            receive_maximum: NonZero::new(1),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let first_message = ClientMessage {
        topic: topic.clone(),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1-first"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(first_message.clone())),
        Ok(())
    );

    let first_packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_first_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: first_packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: false,
        payload: first_message.payload,
        topic,
        properties: PublishProperties::default(),
    });
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&expected_first_publish))
    );

    let inbound_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"inbound-ok".as_slice()),
        topic: Topic::try_new("inbound/unchanged").expect("valid topic"),
        properties: PublishProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&inbound_publish)), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::ReceivedMessage(message)) if message.payload == Payload::new(b"inbound-ok".as_slice())
    ));

    let inbound_qos1_packet_id = NonZero::new(41).expect("non-zero packet id");
    let inbound_qos1_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: inbound_qos1_packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::new(b"inbound-qos1-ok".as_slice()),
        topic: Topic::try_new("inbound/qos1").expect("valid topic"),
        properties: PublishProperties::default(),
    });
    assert_eq!(
        client.handle_read(encode_packet(&inbound_qos1_publish)),
        Ok(())
    );
    let inbound_message_id = match client.poll_read() {
        Some(UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, message)) => {
            assert_eq!(message.payload, Payload::new(b"inbound-qos1-ok".as_slice()));
            id
        }
        other => panic!("expected qos1 inbound message with ack id, got {other:?}"),
    };
    assert_eq!(
        client.handle_write(UserWriteIn::AcknowledgeMessage(inbound_message_id)),
        Ok(())
    );
    let expected_puback = ControlPacket::PubAck(PubAck {
        packet_id: inbound_qos1_packet_id,
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));

    let second_message = ClientMessage {
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1-second"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(second_message)),
        Err(Error::ReceiveMaximumExceeded)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn effective_limits_recompute_on_connect_socketconnected_connack_and_socketclosed() {
    let settings = ClientSettings {
        max_outgoing_qos: Some(MaximumQoS::AtMostOnce),
        ..ClientSettings::default()
    };
    let mut client = Client::<u64>::with_settings(settings);

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let qos1_message = ClientMessage {
        topic,
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_qos: Some(MaximumQoS::AtLeastOnce),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message.clone())),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let second_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_qos: Some(MaximumQoS::AtLeastOnce),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&second_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn effective_limits_recompute_on_connect_applies_pending_connect_options() {
    let mut client = Client::<u64>::with_settings(ClientSettings {
        max_outgoing_qos: None,
        ..ClientSettings::default()
    });

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_qos: Some(MaximumQoS::AtLeastOnce),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let qos2_message = ClientMessage {
        topic: Topic::try_new("test/qos2").expect("valid topic"),
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos2_message)),
        Err(Error::ProtocolError)
    );
}

#[test]
fn effective_limits_recompute_on_connack_applies_broker_receive_maximum() {
    let mut client = Client::<u64>::with_settings(ClientSettings::default());

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            receive_maximum: NonZero::new(1),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let first = ClientMessage {
        topic: Topic::try_new("test/first").expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"1"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(first)),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let second = ClientMessage {
        topic: Topic::try_new("test/second").expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"2"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(second)),
        Err(Error::ReceiveMaximumExceeded)
    );
}

#[test]
fn app_topic_alias_zero_disables_inbound_alias_even_if_connect_requests_more() {
    let settings = ClientSettings {
        max_incoming_topic_alias_maximum: Some(0),
        ..ClientSettings::default()
    };
    let mut client = Client::<u64>::with_settings(settings);

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            topic_alias_maximum: Some(10),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let inbound_publish_with_alias = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"with-alias".as_slice()),
        topic: Topic::try_new("alias/topic").expect("valid topic"),
        properties: PublishProperties {
            topic_alias: Some(NonZero::new(1).expect("non-zero alias")),
            ..PublishProperties::default()
        },
    });

    assert_eq!(
        client.handle_read(encode_packet(&inbound_publish_with_alias)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn app_topic_alias_setting_is_applied_when_connect_option_omits_alias_limit() {
    let settings = ClientSettings {
        max_incoming_topic_alias_maximum: Some(2),
        ..ClientSettings::default()
    };
    let mut client = Client::<u64>::with_settings(settings);

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions::default())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let alias_set_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"alias-set".as_slice()),
        topic: Topic::try_new("alias/source").expect("valid topic"),
        properties: PublishProperties {
            topic_alias: Some(NonZero::new(2).expect("non-zero alias")),
            ..PublishProperties::default()
        },
    });
    assert_eq!(
        client.handle_read(encode_packet(&alias_set_publish)),
        Ok(())
    );
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::ReceivedMessage(_))
    ));

    let alias_over_limit_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        payload: Payload::new(b"alias-over".as_slice()),
        topic: Topic::try_new("alias/over").expect("valid topic"),
        properties: PublishProperties {
            topic_alias: Some(NonZero::new(3).expect("non-zero alias")),
            ..PublishProperties::default()
        },
    });
    assert_eq!(
        client.handle_read(encode_packet(&alias_over_limit_publish)),
        Err(Error::ProtocolError)
    );
}

#[test]
fn app_retain_policy_false_blocks_retain_publish_even_if_broker_allows() {
    let settings = ClientSettings {
        allow_retain: false,
        ..ClientSettings::default()
    };
    let mut client = Client::<u64>::with_settings(settings);

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            retain_available: Some(true),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let retained_message = ClientMessage {
        retain: true,
        topic: Topic::try_new("retain/topic").expect("valid topic"),
        payload: Payload::new(b"retained".as_slice()),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(retained_message)),
        Err(Error::ProtocolError)
    );
}

#[test]
fn app_subscription_policy_flags_override_broker_allowances() {
    let settings = ClientSettings {
        allow_subscription_identifiers: false,
        allow_wildcard_subscriptions: false,
        allow_shared_subscriptions: false,
        ..ClientSettings::default()
    };
    let mut client = Client::<u64>::with_settings(settings);

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            wildcard_subscription_available: Some(true),
            shared_subscription_available: Some(true),
            subscription_identifiers_available: Some(true),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(SubscribeOptions {
            subscription: make_subscription("topic/+"),
            extra_subscriptions: Vec::new(),
            subscription_identifier: Some(NonZero::new(1).expect("non-zero")),
            user_properties: Vec::new(),
        })),
        Err(Error::ProtocolError)
    );

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(SubscribeOptions {
            subscription: make_subscription("$share/g/topic"),
            extra_subscriptions: Vec::new(),
            subscription_identifier: None,
            user_properties: Vec::new(),
        })),
        Err(Error::ProtocolError)
    );
}

#[test]
fn outbound_qos2_publish_emits_completed_event_on_pubcomp() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");

    let qos2_message = ClientMessage {
        topic: topic.clone(),
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos2_message.clone())),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: qos2_message.payload,
        topic,
        properties: PublishProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_publish)));

    let pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::Success,
        properties: PubRecProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubrec)), Ok(()));

    let expected_pubrel = ControlPacket::PubRel(PubRel {
        packet_id,
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrel)));

    let pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubcomp)), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::PublishCompleted(id, PubCompReasonCode::Success)) if id == packet_id
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn unexpected_pubcomp_before_pubrec_transition_triggers_protocol_error_close() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let qos2_message = ClientMessage {
        topic,
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos2_message)),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&pubcomp)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn receive_maximum_full_returns_immediate_error_for_new_qos2_publish() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            receive_maximum: NonZero::new(1),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let first_message = ClientMessage {
        topic: topic.clone(),
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2-first"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(first_message.clone())),
        Ok(())
    );

    let first_packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_first_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: first_packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: first_message.payload,
        topic,
        properties: PublishProperties::default(),
    });
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&expected_first_publish))
    );

    let second_message = ClientMessage {
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2-second"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(second_message)),
        Err(Error::ReceiveMaximumExceeded)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn duplicate_pubrec_in_qos2_await_pubcomp_resends_pubrel_without_disconnect() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let qos2_message = ClientMessage {
        topic: topic.clone(),
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos2_message.clone())),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: qos2_message.payload,
        topic,
        properties: PublishProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_publish)));

    let pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::Success,
        properties: PubRecProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubrec)), Ok(()));

    let expected_pubrel = ControlPacket::PubRel(PubRel {
        packet_id,
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrel)));

    assert_eq!(client.handle_read(encode_packet(&pubrec)), Ok(()));
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrel)));
    assert!(matches!(client.poll_event(), None));

    let pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubcomp)), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::PublishCompleted(id, PubCompReasonCode::Success)) if id == packet_id
    ));
}

#[test]
fn qos2_pubrec_failure_reason_drops_inflight_without_pubrel() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("test/topic").expect("valid utf8"))
        .expect("valid topic");
    let qos2_message = ClientMessage {
        topic,
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos2_message)),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    assert!(client.poll_write().is_some());

    let pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::NotAuthorized,
        properties: PubRecProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubrec)), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::PublishDroppedDueToBrokerRejectedPubRec(id, PubRecReasonCode::NotAuthorized)) if id == packet_id
    ));

    let pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties::default(),
    });
    assert_eq!(
        client.handle_read(encode_packet(&pubcomp)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let subscribe = SubscribeOptions {
        subscription: Subscription {
            topic_filter: Utf8String::try_from("a/very/long/topic/filter").expect("valid utf8"),
            qos: Qos::AtMostOnce,
            no_local: false,
            retain_as_published: false,
            retain_handling: RetainHandling::SendRetained,
        },
        extra_subscriptions: Vec::new(),
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

    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert!(matches!(client.poll_event(), None));
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let reconnect_connect = client.poll_write().expect("connect bytes are queued");

    assert_eq!(reconnect_connect, first_connect);
}

#[test]
fn connack_resume_with_clean_start_is_protocol_error() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            clean_start: true,
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&resumed_connack)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn connack_resume_without_local_state_is_accepted_when_clean_start_false() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            clean_start: false,
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
    assert!(matches!(client.poll_event(), None));
}

#[test]
fn resumed_session_replays_outbound_qos_publish_with_dup_set() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            session_expiry_interval: Some(30),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let initial_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&initial_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("replay/topic").expect("valid utf8"))
        .expect("valid topic");
    let outbound = ClientMessage {
        topic: topic.clone(),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"replay"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(outbound.clone())),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let first_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: false,
        payload: outbound.payload,
        topic,
        properties: PublishProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&first_publish)));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let replay_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::AtLeastOnce,
            dup: true,
        },
        retain: false,
        payload: Payload::from(&b"replay"[..]),
        topic: Topic::try_from(Utf8String::try_from("replay/topic").expect("valid utf8"))
            .expect("valid topic"),
        properties: PublishProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&replay_publish)));

    let puback = ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&puback)), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::PublishAcknowledged(id, PubAckReasonCode::Success)) if id == packet_id
    ));
}

#[test]
fn resumed_session_replay_failure_does_not_emit_connected_and_closes() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            session_expiry_interval: Some(30),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let initial_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&initial_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("replay/failure").expect("valid utf8"))
        .expect("valid topic");
    let outbound = ClientMessage {
        topic: topic.clone(),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(vec![0; 64]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(outbound.clone())),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::Publish(Publish {
            kind: PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            },
            retain: false,
            payload: outbound.payload,
            topic,
            properties: PublishProperties::default(),
        })))
    );

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties {
            maximum_packet_size: NonZero::new(16),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(
        client.handle_read(encode_packet(&resumed_connack)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn resumed_session_replays_unacknowledged_pubrel() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            session_expiry_interval: Some(30),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let initial_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&initial_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("resume/qos2").expect("valid utf8"))
        .expect("valid topic");
    let outbound = ClientMessage {
        topic: topic.clone(),
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(outbound.clone())),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::Publish(Publish {
            kind: PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            },
            retain: false,
            payload: outbound.payload,
            topic,
            properties: PublishProperties::default(),
        })))
    );

    let pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::Success,
        properties: PubRecProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubrec)), Ok(()));
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubRel(PubRel {
            packet_id,
            reason_code: PubRelReasonCode::Success,
            properties: PubRelProperties::default(),
        })))
    );

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubRel(PubRel {
            packet_id,
            reason_code: PubRelReasonCode::Success,
            properties: PubRelProperties::default(),
        })))
    );

    let pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubcomp)), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::PublishCompleted(id, PubCompReasonCode::Success)) if id == packet_id
    ));
}

#[test]
fn non_resumed_session_drops_inflight_and_emits_publish_dropped_events() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            session_expiry_interval: Some(30),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let initial_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&initial_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("drop/topic").expect("valid utf8"))
        .expect("valid topic");

    let qos1 = ClientMessage {
        topic: topic.clone(),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1)),
        Ok(())
    );

    let qos1_packet_id = NonZero::new(1).expect("non-zero packet id");
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::Publish(Publish {
            kind: PublishKind::Repetible {
                packet_id: qos1_packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            },
            retain: false,
            payload: Payload::from(&b"qos1"[..]),
            topic: topic.clone(),
            properties: PublishProperties::default(),
        })))
    );

    let qos2 = ClientMessage {
        topic: topic.clone(),
        qos: Qos::ExactlyOnce,
        payload: Payload::from(&b"qos2"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos2)),
        Ok(())
    );

    let qos2_packet_id = NonZero::new(2).expect("non-zero packet id");
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::Publish(Publish {
            kind: PublishKind::Repetible {
                packet_id: qos2_packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            },
            retain: false,
            payload: Payload::from(&b"qos2"[..]),
            topic: topic.clone(),
            properties: PublishProperties::default(),
        })))
    );

    let inbound_packet_id = NonZero::new(33).expect("non-zero packet id");
    let inbound_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: inbound_packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::from(&b"inbound"[..]),
        topic: Topic::try_from(Utf8String::try_from("inbound/topic").expect("valid utf8"))
            .expect("valid topic"),
        properties: PublishProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&inbound_publish)), Ok(()));
    let _ = client.poll_read();
    assert_eq!(client.poll_write(), None);

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let non_resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(
        client.handle_read(encode_packet(&non_resumed_connack)),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::PublishDroppedDueToSessionNotResumed(id)) if id == qos1_packet_id
    ));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::PublishDroppedDueToSessionNotResumed(id)) if id == qos2_packet_id
    ));
    assert!(matches!(client.poll_read(), None));
    assert_eq!(client.poll_write(), None);

    let pubrel = ControlPacket::PubRel(PubRel {
        packet_id: inbound_packet_id,
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubrel)), Ok(()));
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubComp(PubComp {
            packet_id: inbound_packet_id,
            reason_code: PubCompReasonCode::PacketIdentifierNotFound,
            properties: PubCompProperties::default(),
        })))
    );
}

#[test]
fn non_resumed_connack_discards_all_local_session_state() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let initial_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&initial_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let topic = Topic::try_from(Utf8String::try_from("state/topic").expect("valid utf8"))
        .expect("valid topic");
    let outbound_qos1 = ClientMessage {
        topic: topic.clone(),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(outbound_qos1)),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let inbound_packet_id = NonZero::new(55).expect("non-zero packet id");
    let inbound_publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: inbound_packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::from(&b"inbound"[..]),
        topic: Topic::try_from(Utf8String::try_from("state/inbound").expect("valid utf8"))
            .expect("valid topic"),
        properties: PublishProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&inbound_publish)), Ok(()));
    let _ = client.poll_read();
    assert_eq!(client.poll_write(), None);

    let subscribe = SubscribeOptions {
        subscription: make_subscription("state/sub"),
        extra_subscriptions: Vec::new(),
        subscription_identifier: None,
        user_properties: Vec::new(),
    };
    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(subscribe)),
        Ok(())
    );
    let _ = client.poll_write().expect("subscribe frame expected");

    let unsubscribe = sansio_mqtt_v5_protocol::UnsubscribeOptions {
        filter: Utf8String::try_from("state/sub").expect("valid utf8"),
        ..Default::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::Unsubscribe(unsubscribe)),
        Ok(())
    );
    let _ = client.poll_write().expect("unsubscribe frame expected");

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let non_resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(
        client.handle_read(encode_packet(&non_resumed_connack)),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let pubrel = ControlPacket::PubRel(PubRel {
        packet_id: inbound_packet_id,
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubrel)), Ok(()));
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubComp(PubComp {
            packet_id: inbound_packet_id,
            reason_code: PubCompReasonCode::PacketIdentifierNotFound,
            properties: PubCompProperties::default(),
        })))
    );

    let stale_suback = ControlPacket::SubAck(sansio_mqtt_v5_types::SubAck {
        packet_id: NonZero::new(2).expect("non-zero"),
        properties: sansio_mqtt_v5_types::SubAckProperties::default(),
        reason_codes: vec![sansio_mqtt_v5_types::SubAckReasonCode::SuccessQoS0],
    });
    assert_eq!(
        client.handle_read(encode_packet(&stale_suback)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));

    let mut client = Client::<u64>::default();
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(client.handle_read(encode_packet(&initial_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let unsubscribe = sansio_mqtt_v5_protocol::UnsubscribeOptions {
        filter: Utf8String::try_from("state/unsub").expect("valid utf8"),
        ..Default::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::Unsubscribe(unsubscribe)),
        Ok(())
    );
    let _ = client.poll_write().expect("unsubscribe frame expected");

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(
        client.handle_read(encode_packet(&non_resumed_connack)),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let stale_unsuback = ControlPacket::UnsubAck(sansio_mqtt_v5_types::UnsubAck {
        packet_id: NonZero::new(1).expect("non-zero"),
        properties: sansio_mqtt_v5_types::UnsubAckProperties::default(),
        reason_codes: vec![sansio_mqtt_v5_types::UnsubAckReasonCode::Success],
    });
    assert_eq!(
        client.handle_read(encode_packet(&stale_unsuback)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn stale_read_buffer_is_cleared_on_socket_closed() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_read(Bytes::from_static(&[0x20])), Ok(()));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

#[test]
fn stale_read_buffer_is_cleared_on_socket_error() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_read(Bytes::from_static(&[0x20])), Ok(()));

    assert_eq!(
        client.handle_event(DriverEventIn::SocketError),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

#[test]
fn stale_read_buffer_is_cleared_on_user_disconnect() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_read(Bytes::from_static(&[0x20])), Ok(()));

    assert_eq!(client.handle_write(UserWriteIn::Disconnect), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

#[test]
fn stale_read_buffer_is_cleared_on_close() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_read(Bytes::from_static(&[0x20])), Ok(()));

    assert_eq!(client.close(), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

#[test]
fn timeout_in_connected_state_enqueues_pingreq() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(client.close(), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));
    assert!(matches!(client.poll_read(), None));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), None));
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(client.close(), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));
    assert!(matches!(client.poll_read(), None));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), None));
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(client.handle_write(UserWriteIn::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));
    assert!(matches!(client.poll_read(), None));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn timeout_is_cleared_on_close() {
    let mut close_client = Client::<u64>::default();

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(
        close_client.handle_write(UserWriteIn::Connect(options)),
        Ok(())
    );
    assert!(matches!(
        close_client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
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
    assert!(matches!(
        close_client.poll_read(),
        Some(UserWriteOut::Connected)
    ));

    assert_eq!(close_client.handle_timeout(42), Ok(()));
    assert_eq!(close_client.poll_timeout(), Some(42));

    assert_eq!(close_client.close(), Ok(()));
    assert_eq!(close_client.poll_timeout(), None);

    let mut socket_closed_client = Client::<u64>::default();

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(
        socket_closed_client.handle_write(UserWriteIn::Connect(options)),
        Ok(())
    );
    assert!(matches!(
        socket_closed_client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(
        socket_closed_client.handle_event(DriverEventIn::SocketConnected),
        Ok(())
    );
    assert!(socket_closed_client.poll_write().is_some());
    assert_eq!(
        socket_closed_client.handle_read(encode_packet(&connack)),
        Ok(())
    );
    assert!(matches!(
        socket_closed_client.poll_read(),
        Some(UserWriteOut::Connected)
    ));

    assert_eq!(socket_closed_client.handle_timeout(99), Ok(()));
    assert_eq!(socket_closed_client.poll_timeout(), Some(99));

    assert_eq!(
        socket_closed_client.handle_event(DriverEventIn::SocketClosed),
        Ok(())
    );
    assert_eq!(socket_closed_client.poll_timeout(), None);
}

#[test]
fn connecting_accepts_auth_and_stays_open() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            authentication: Some(sansio_mqtt_v5_types::AuthenticationKind::WithoutData {
                method: Utf8String::try_from("SCRAM").expect("valid utf8"),
            }),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let auth = ControlPacket::Auth(Auth {
        reason_code: AuthReasonCode::ContinueAuthentication,
        properties: AuthProperties {
            authentication: Some(sansio_mqtt_v5_types::AuthenticationKind::WithoutData {
                method: Utf8String::try_from("SCRAM").expect("valid utf8"),
            }),
            ..AuthProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&auth)), Ok(()));
    assert!(matches!(client.poll_event(), None));
}

#[test]
fn connecting_auth_then_connack_success_transitions_connected() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            authentication: Some(sansio_mqtt_v5_types::AuthenticationKind::WithoutData {
                method: Utf8String::try_from("SCRAM").expect("valid utf8"),
            }),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let auth = ControlPacket::Auth(Auth {
        reason_code: AuthReasonCode::ContinueAuthentication,
        properties: AuthProperties {
            authentication: Some(sansio_mqtt_v5_types::AuthenticationKind::WithoutData {
                method: Utf8String::try_from("SCRAM").expect("valid utf8"),
            }),
            ..AuthProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&auth)), Ok(()));

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

#[test]
fn connecting_auth_without_configured_authentication_is_protocol_error() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let auth = ControlPacket::Auth(Auth {
        reason_code: AuthReasonCode::ContinueAuthentication,
        properties: AuthProperties::default(),
    });
    assert_eq!(
        client.handle_read(encode_packet(&auth)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn connecting_auth_with_reason_other_than_continue_is_protocol_error() {
    let mut client = Client::<u64>::default();

    assert_eq!(
        client.handle_write(UserWriteIn::Connect(ConnectionOptions {
            authentication: Some(sansio_mqtt_v5_types::AuthenticationKind::WithoutData {
                method: Utf8String::try_from("SCRAM").expect("valid utf8"),
            }),
            ..ConnectionOptions::default()
        })),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let auth = ControlPacket::Auth(Auth {
        reason_code: AuthReasonCode::Success,
        properties: AuthProperties {
            authentication: Some(sansio_mqtt_v5_types::AuthenticationKind::WithoutData {
                method: Utf8String::try_from("SCRAM").expect("valid utf8"),
            }),
            ..AuthProperties::default()
        },
    });
    assert_eq!(
        client.handle_read(encode_packet(&auth)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn publish_qos_above_server_maximum_qos_is_rejected() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            maximum_qos: Some(MaximumQoS::AtMostOnce),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let message = ClientMessage {
        topic: Topic::try_from(Utf8String::try_from("qos/guard").expect("valid utf8"))
            .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"payload"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(message)),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn publish_retain_when_server_retain_not_available_is_rejected() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            retain_available: Some(false),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let message = ClientMessage {
        topic: Topic::try_from(Utf8String::try_from("retain/guard").expect("valid utf8"))
            .expect("valid topic"),
        payload: Payload::from(&b"payload"[..]),
        ..ClientMessage::default()
    };

    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(ClientMessage {
            retain: true,
            ..message
        })),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_shared_with_no_local_is_rejected() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let subscribe = SubscribeOptions {
        subscription: Subscription {
            no_local: true,
            ..make_subscription("$share/group/topic")
        },
        extra_subscriptions: Vec::new(),
        subscription_identifier: None,
        user_properties: Vec::new(),
    };

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(subscribe)),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_wildcard_when_server_disallows_is_rejected() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            wildcard_subscription_available: Some(false),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let subscribe = SubscribeOptions {
        subscription: make_subscription("topic/#"),
        extra_subscriptions: Vec::new(),
        subscription_identifier: None,
        user_properties: Vec::new(),
    };

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(subscribe)),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_shared_when_server_disallows_is_rejected() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            shared_subscription_available: Some(false),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let subscribe = SubscribeOptions {
        subscription: make_subscription("$share/g/topic"),
        extra_subscriptions: Vec::new(),
        subscription_identifier: None,
        user_properties: Vec::new(),
    };

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(subscribe)),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_identifier_when_server_disallows_is_rejected() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            subscription_identifiers_available: Some(false),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let subscribe = SubscribeOptions {
        subscription: make_subscription("topic/a"),
        extra_subscriptions: Vec::new(),
        subscription_identifier: NonZero::new(1),
        user_properties: Vec::new(),
    };

    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(subscribe)),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn connecting_auth_continue_then_connack_success_connects() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        authentication: Some(sansio_mqtt_v5_types::AuthenticationKind::WithoutData {
            method: Utf8String::try_from("SCRAM").expect("valid utf8"),
        }),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let auth = ControlPacket::Auth(Auth {
        reason_code: AuthReasonCode::ContinueAuthentication,
        properties: AuthProperties {
            authentication: Some(sansio_mqtt_v5_types::AuthenticationKind::WithoutData {
                method: Utf8String::try_from("SCRAM").expect("valid utf8"),
            }),
            ..AuthProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&auth)), Ok(()));

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
}

#[test]
fn auth_in_connected_without_support_is_protocol_error() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let auth = ControlPacket::Auth(Auth {
        reason_code: AuthReasonCode::ContinueAuthentication,
        properties: AuthProperties::default(),
    });
    assert_eq!(
        client.handle_read(encode_packet(&auth)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn keepalive_disabled_without_interval_no_pingreq() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(client.handle_timeout(1), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn connack_server_keep_alive_zero_disables_keepalive_without_panic() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            server_keep_alive: Some(0),
            ..ConnAckProperties::default()
        },
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(client.handle_timeout(1), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn keepalive_timeout_without_pingresp_closes_connection() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    assert_eq!(client.handle_timeout(1), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xC0, 0x00])));

    assert_eq!(client.handle_timeout(2), Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x8D, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}

#[test]
fn clean_start_true_clears_local_session_before_connect() {
    let mut client = Client::<u64>::default();

    let first_options = ConnectionOptions {
        session_expiry_interval: Some(30),
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::Connect(first_options)),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let qos1_message = ClientMessage {
        topic: Topic::try_from(Utf8String::try_from("clean/start").expect("valid utf8"))
            .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"qos1"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_write(UserWriteIn::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    let clean_start_options = ConnectionOptions {
        clean_start: true,
        session_expiry_interval: Some(30),
        ..ConnectionOptions::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::Connect(clean_start_options)),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(
        client.handle_read(encode_packet(&resumed_connack)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), None));
}

#[test]
fn session_with_expiry_keeps_inflight_across_graceful_disconnect() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        session_expiry_interval: Some(30),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let qos1_message = ClientMessage {
        topic: Topic::try_from(Utf8String::try_from("session/persist").expect("valid utf8"))
            .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"persist"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Ok(())
    );
    let publish = client.poll_write().expect("publish expected");

    assert_eq!(client.handle_write(UserWriteIn::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
    let replay_publish = client.poll_write().expect("replayed publish expected");
    assert_eq!(replay_publish.len(), publish.len());
    assert_eq!(replay_publish[0], publish[0] | 0b0000_1000);
    assert_eq!(&replay_publish[1..], &publish[1..]);
}

#[test]
fn zero_session_expiry_clears_inflight_on_disconnect() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        session_expiry_interval: Some(0),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let qos1_message = ClientMessage {
        topic: Topic::try_from(Utf8String::try_from("session/clear").expect("valid utf8"))
            .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"clear"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_write(UserWriteIn::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));
    assert_eq!(client.poll_write(), None);
    assert!(matches!(client.poll_event(), None));
}

#[test]
fn zero_session_expiry_clears_inflight_on_socket_closed() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        session_expiry_interval: Some(0),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let qos1_message = ClientMessage {
        topic: Topic::try_from(Utf8String::try_from("session/close-clear").expect("valid utf8"))
            .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"clear"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Ok(())
    );
    let first_publish = client.poll_write().expect("publish expected");

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::Disconnected)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let replay = client.poll_write();
    assert_eq!(replay, None);
    assert_ne!(replay, Some(first_publish));
}

#[test]
fn keepalive_timeout_with_session_expiry_preserves_inflight_for_resume() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        session_expiry_interval: Some(30),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let qos1_message = ClientMessage {
        topic: Topic::try_from(
            Utf8String::try_from("session/timeout-persist").expect("valid utf8"),
        )
        .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"persist"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Ok(())
    );
    let first_publish = client.poll_write().expect("publish expected");

    assert_eq!(client.handle_timeout(1), Ok(()));
    assert_eq!(client.poll_write(), None);

    assert_eq!(client.handle_timeout(2), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xC0, 0x00])));

    assert_eq!(client.handle_timeout(3), Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x8D, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let replay_publish = client.poll_write().expect("replayed publish expected");
    assert_eq!(replay_publish.len(), first_publish.len());
    assert_eq!(replay_publish[0], first_publish[0] | 0b0000_1000);
    assert_eq!(&replay_publish[1..], &first_publish[1..]);
}

#[test]
fn subscribe_tracks_packet_id_until_suback() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let subscribe = SubscribeOptions {
        subscription: make_subscription("topic/a"),
        extra_subscriptions: Vec::new(),
        subscription_identifier: None,
        user_properties: Vec::new(),
    };
    assert_eq!(
        client.handle_write(UserWriteIn::Subscribe(subscribe)),
        Ok(())
    );
    let subscribe_frame = client.poll_write().expect("subscribe frame expected");

    let qos1_message = ClientMessage {
        topic: Topic::try_from(Utf8String::try_from("topic/pub").expect("valid utf8"))
            .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"payload"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Ok(())
    );
    let publish_frame = client.poll_write().expect("publish frame expected");
    assert_ne!(publish_frame, subscribe_frame);

    let suback = ControlPacket::SubAck(sansio_mqtt_v5_types::SubAck {
        packet_id: NonZero::new(1).expect("non-zero"),
        properties: sansio_mqtt_v5_types::SubAckProperties::default(),
        reason_codes: vec![sansio_mqtt_v5_types::SubAckReasonCode::SuccessQoS0],
    });
    assert_eq!(client.handle_read(encode_packet(&suback)), Ok(()));
}

#[test]
fn unsubscribe_tracks_packet_id_until_unsuback() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let unsubscribe = sansio_mqtt_v5_protocol::UnsubscribeOptions {
        filter: Utf8String::try_from("topic/a").expect("valid utf8"),
        ..Default::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::Unsubscribe(unsubscribe)),
        Ok(())
    );
    let unsub_frame = client.poll_write().expect("unsubscribe frame expected");

    let qos1_message = ClientMessage {
        topic: Topic::try_from(Utf8String::try_from("topic/pub").expect("valid utf8"))
            .expect("valid topic"),
        qos: Qos::AtLeastOnce,
        payload: Payload::from(&b"payload"[..]),
        ..ClientMessage::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::PublishMessage(qos1_message)),
        Ok(())
    );
    let publish_frame = client.poll_write().expect("publish frame expected");
    assert_ne!(publish_frame, unsub_frame);

    let unsuback = ControlPacket::UnsubAck(sansio_mqtt_v5_types::UnsubAck {
        packet_id: NonZero::new(1).expect("non-zero"),
        properties: sansio_mqtt_v5_types::UnsubAckProperties::default(),
        reason_codes: vec![sansio_mqtt_v5_types::UnsubAckReasonCode::Success],
    });
    assert_eq!(client.handle_read(encode_packet(&unsuback)), Ok(()));
}

#[test]
fn unknown_suback_or_unsuback_is_protocol_error() {
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
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let suback = ControlPacket::SubAck(sansio_mqtt_v5_types::SubAck {
        packet_id: NonZero::new(123).expect("non-zero"),
        properties: sansio_mqtt_v5_types::SubAckProperties::default(),
        reason_codes: vec![sansio_mqtt_v5_types::SubAckReasonCode::SuccessQoS0],
    });
    assert_eq!(
        client.handle_read(encode_packet(&suback)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));

    let mut client = Client::<u64>::default();
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert!(matches!(client.poll_read(), Some(UserWriteOut::Connected)));

    let unsuback = ControlPacket::UnsubAck(sansio_mqtt_v5_types::UnsubAck {
        packet_id: NonZero::new(123).expect("non-zero"),
        properties: sansio_mqtt_v5_types::UnsubAckProperties::default(),
        reason_codes: vec![sansio_mqtt_v5_types::UnsubAckReasonCode::Success],
    });
    assert_eq!(
        client.handle_read(encode_packet(&unsuback)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    ));
}
