use bytes::Bytes;
use core::num::NonZero;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_v5_protocol::{
    Client, ClientMessage, Config, ConnectionOptions, DriverEventIn, Error, PublishDroppedReason,
    SubscribeOptions, UserWriteIn, UserWriteOut,
};
use sansio_mqtt_v5_types::{
    Auth, AuthProperties, AuthReasonCode, ConnAck, ConnAckKind, ConnAckProperties,
    ConnackReasonCode, ControlPacket, Disconnect, DisconnectProperties, DisconnectReasonCode,
    GuaranteedQoS, Payload, PubAck, PubAckProperties, PubAckReasonCode, PubComp, PubCompProperties,
    PubCompReasonCode, PubRec, PubRecProperties, PubRecReasonCode, PubRel, PubRelProperties,
    PubRelReasonCode, Publish, PublishKind, PublishProperties, Qos, Settings, Topic, Utf8String,
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
fn user_write_out_exposes_qos_delivery_events_with_packet_id() {
    let packet_id = NonZero::new(7).expect("non-zero packet id");

    let acknowledged = UserWriteOut::PublishAcknowledged {
        packet_id,
        reason_code: PubAckReasonCode::Success,
    };
    let completed = UserWriteOut::PublishCompleted {
        packet_id,
        reason_code: PubCompReasonCode::Success,
    };
    let dropped = UserWriteOut::PublishDropped {
        packet_id,
        reason: PublishDroppedReason::SessionNotResumed,
    };
    let dropped_by_broker = UserWriteOut::PublishDropped {
        packet_id,
        reason: PublishDroppedReason::BrokerRejectedPubRec {
            reason_code: PubRecReasonCode::NotAuthorized,
        },
    };

    match acknowledged {
        UserWriteOut::PublishAcknowledged {
            packet_id,
            reason_code,
        } => {
            assert_eq!(packet_id.get(), 7);
            assert_eq!(reason_code, PubAckReasonCode::Success);
        }
        other => panic!("expected PublishAcknowledged, got {other:?}"),
    }

    match completed {
        UserWriteOut::PublishCompleted {
            packet_id,
            reason_code,
        } => {
            assert_eq!(packet_id.get(), 7);
            assert_eq!(reason_code, PubCompReasonCode::Success);
        }
        other => panic!("expected PublishCompleted, got {other:?}"),
    }

    match dropped {
        UserWriteOut::PublishDropped { packet_id, reason } => {
            assert_eq!(packet_id.get(), 7);
            assert_eq!(reason, PublishDroppedReason::SessionNotResumed);
        }
        other => panic!("expected PublishDropped, got {other:?}"),
    }

    match dropped_by_broker {
        UserWriteOut::PublishDropped { packet_id, reason } => {
            assert_eq!(packet_id.get(), 7);
            assert_eq!(
                reason,
                PublishDroppedReason::BrokerRejectedPubRec {
                    reason_code: PubRecReasonCode::NotAuthorized,
                }
            );
        }
        other => panic!("expected PublishDropped, got {other:?}"),
    }
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
    let _ = client.poll_write().expect("connect frame expected");

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
fn inbound_qos1_publish_sends_puback_and_emits_message_once() {
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

    let packet_id = NonZero::new(7).expect("non-zero packet id");
    let publish_topic = Topic::try_from(Utf8String::try_from("sensors/temp").expect("valid utf8"))
        .expect("valid topic");
    let publish_payload = Payload::from(&b"27.5"[..]);
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

    match client.poll_read() {
        Some(UserWriteOut::ReceivedMessage(message)) => {
            assert_eq!(message.topic, publish_topic);
            assert_eq!(message.payload, publish_payload);
        }
        other => panic!("expected received message, got {other:?}"),
    }

    let expected_puback = ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    });
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));
    assert_eq!(client.poll_read(), None);
}

#[test]
fn inbound_qos2_duplicate_publish_resends_pubrec_without_duplicate_delivery() {
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

    let packet_id = NonZero::new(11).expect("non-zero packet id");
    let publish_topic =
        Topic::try_from(Utf8String::try_from("sensors/humidity").expect("valid utf8"))
            .expect("valid topic");
    let publish_payload = Payload::from(&b"42"[..]);
    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
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
        }
        other => panic!("expected received message, got {other:?}"),
    }

    let expected_pubrec = ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code: PubRecReasonCode::Success,
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
        payload: publish_payload,
        topic: publish_topic,
        properties: PublishProperties::default(),
    });

    assert_eq!(
        client.handle_read(encode_packet(&duplicate_publish)),
        Ok(())
    );
    assert_eq!(client.poll_read(), None);
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));
    assert_eq!(client.poll_event(), None);
}

#[test]
fn inbound_qos2_pubrel_completes_with_pubcomp() {
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

    let packet_id = NonZero::new(13).expect("non-zero packet id");
    let publish = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id,
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        payload: Payload::from(&b"qos2"[..]),
        topic: Topic::try_from(Utf8String::try_from("sensors/pressure").expect("valid utf8"))
            .expect("valid topic"),
        properties: PublishProperties::default(),
    });

    assert_eq!(client.handle_read(encode_packet(&publish)), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(UserWriteOut::ReceivedMessage(_))
    ));

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
    assert_eq!(client.poll_event(), None);
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_event(), None);
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_read(),
        Some(UserWriteOut::PublishAcknowledged {
            packet_id,
            reason_code: PubAckReasonCode::Success,
        })
    );
    assert_eq!(client.poll_read(), None);
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_read(), None);
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_read(), None);
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
}

#[test]
fn receive_maximum_full_returns_immediate_error_for_new_qos1_publish() {
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_read(),
        Some(UserWriteOut::PublishCompleted {
            packet_id,
            reason_code: PubCompReasonCode::Success,
        })
    );
    assert_eq!(client.poll_read(), None);
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_event(), None);

    let pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubcomp)), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(UserWriteOut::PublishCompleted {
            packet_id,
            reason_code: PubCompReasonCode::Success,
        })
    );
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_read(),
        Some(UserWriteOut::PublishDropped {
            packet_id,
            reason: PublishDroppedReason::BrokerRejectedPubRec {
                reason_code: PubRecReasonCode::NotAuthorized,
            },
        })
    );

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
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
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
fn resumed_session_replays_outbound_qos_publish_with_dup_set() {
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_read(),
        Some(UserWriteOut::PublishAcknowledged {
            packet_id,
            reason_code: PubAckReasonCode::Success,
        })
    );
}

#[test]
fn resumed_session_replay_failure_does_not_emit_connected_and_closes() {
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
    assert_eq!(client.poll_read(), None);
}

#[test]
fn resumed_session_with_qos2_await_pubcomp_continues_without_replay_publish() {
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));
    assert_eq!(client.poll_write(), None);

    let pubcomp = ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&pubcomp)), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(UserWriteOut::PublishCompleted {
            packet_id,
            reason_code: PubCompReasonCode::Success,
        })
    );
}

#[test]
fn non_resumed_session_drops_inflight_and_emits_publish_dropped_events() {
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubRec(PubRec {
            packet_id: inbound_packet_id,
            reason_code: PubRecReasonCode::Success,
            properties: PubRecProperties::default(),
        })))
    );

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));
    assert_eq!(
        client.poll_read(),
        Some(UserWriteOut::PublishDropped {
            packet_id: qos1_packet_id,
            reason: PublishDroppedReason::SessionNotResumed,
        })
    );
    assert_eq!(
        client.poll_read(),
        Some(UserWriteOut::PublishDropped {
            packet_id: qos2_packet_id,
            reason: PublishDroppedReason::SessionNotResumed,
        })
    );
    assert_eq!(client.poll_read(), None);
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
fn stale_read_buffer_is_cleared_on_socket_closed() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_read(Bytes::from_static(&[0x20])), Ok(()));

    assert_eq!(client.handle_event(DriverEventIn::SocketClosed), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

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
fn stale_read_buffer_is_cleared_on_socket_error() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_read(Bytes::from_static(&[0x20])), Ok(()));

    assert_eq!(
        client.handle_event(DriverEventIn::SocketError),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );

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
fn stale_read_buffer_is_cleared_on_user_disconnect() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_read(Bytes::from_static(&[0x20])), Ok(()));

    assert_eq!(client.handle_write(UserWriteIn::Disconnect), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );

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
fn stale_read_buffer_is_cleared_on_close() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_read(Bytes::from_static(&[0x20])), Ok(()));

    assert_eq!(client.close(), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );

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
fn timeout_in_connected_state_enqueues_pingreq() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

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

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(
        close_client.handle_write(UserWriteIn::Connect(options)),
        Ok(())
    );
    assert_eq!(
        close_client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
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

    let options = ConnectionOptions {
        keep_alive: NonZero::new(10),
        ..ConnectionOptions::default()
    };
    assert_eq!(
        socket_closed_client.handle_write(UserWriteIn::Connect(options)),
        Ok(())
    );
    assert_eq!(
        socket_closed_client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
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

#[test]
fn connecting_accepts_auth_and_stays_open() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let auth = ControlPacket::Auth(Auth {
        reason_code: AuthReasonCode::ContinueAuthentication,
        properties: AuthProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&auth)), Ok(()));
    assert_eq!(client.poll_event(), None);
}

#[test]
fn connecting_auth_then_connack_success_transitions_connected() {
    let mut client = Client::<u64>::default();

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let auth = ControlPacket::Auth(Auth {
        reason_code: AuthReasonCode::ContinueAuthentication,
        properties: AuthProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&auth)), Ok(()));

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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    assert_eq!(client.handle_timeout(1), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xC0, 0x00])));

    assert_eq!(client.handle_timeout(2), Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x8D, 0x00]))
    );
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

    let clean_start_options = ConnectionOptions {
        clean_start: true,
        session_expiry_interval: Some(30),
        ..ConnectionOptions::default()
    };
    assert_eq!(
        client.handle_write(UserWriteIn::Connect(clean_start_options)),
        Ok(())
    );
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    let _ = client.poll_write().expect("connect frame expected");

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));
    assert_eq!(client.poll_write(), None);
}

#[test]
fn session_with_expiry_keeps_inflight_across_graceful_disconnect() {
    let mut client = Client::<u64>::default();

    let options = ConnectionOptions {
        session_expiry_interval: Some(30),
        ..ConnectionOptions::default()
    };
    assert_eq!(client.handle_write(UserWriteIn::Connect(options)), Ok(()));
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));
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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket)
    );
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
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
    assert_eq!(client.poll_read(), Some(UserWriteOut::Disconnected));

    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    });
    assert_eq!(client.handle_read(encode_packet(&resumed_connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));
    assert_eq!(client.poll_write(), None);
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let subscribe = SubscribeOptions {
        subscriptions: vec![Utf8String::try_from("topic/a").expect("valid utf8")]
            .try_into()
            .expect("one topic"),
        ..SubscribeOptions::default()
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let unsubscribe = sansio_mqtt_v5_protocol::UnsubscribeOptions {
        subscriptions: vec![Utf8String::try_from("topic/a").expect("valid utf8")]
            .try_into()
            .expect("one topic"),
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
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let suback = ControlPacket::SubAck(sansio_mqtt_v5_types::SubAck {
        packet_id: NonZero::new(123).expect("non-zero"),
        properties: sansio_mqtt_v5_types::SubAckProperties::default(),
        reason_codes: vec![sansio_mqtt_v5_types::SubAckReasonCode::SuccessQoS0],
    });
    assert_eq!(
        client.handle_read(encode_packet(&suback)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );

    let mut client = Client::<u64>::default();
    assert_eq!(client.handle_event(DriverEventIn::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(client.handle_read(encode_packet(&connack)), Ok(()));
    assert_eq!(client.poll_read(), Some(UserWriteOut::Connected));

    let unsuback = ControlPacket::UnsubAck(sansio_mqtt_v5_types::UnsubAck {
        packet_id: NonZero::new(123).expect("non-zero"),
        properties: sansio_mqtt_v5_types::UnsubAckProperties::default(),
        reason_codes: vec![sansio_mqtt_v5_types::UnsubAckReasonCode::Success],
    });
    assert_eq!(
        client.handle_read(encode_packet(&unsuback)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_event(),
        Some(sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket)
    );
}
