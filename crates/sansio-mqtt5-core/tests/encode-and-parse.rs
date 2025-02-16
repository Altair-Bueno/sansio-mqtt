use std::num::NonZero;

use encode::Encodable;
use sansio_mqtt5::parser::Settings;
use sansio_mqtt5::types::*;
use winnow::error::ContextError;
use winnow::Parser;

#[test]
fn assert_that_auth_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::Auth(Auth {
        reason_code: ReasonCode::Success,
        properties: AuthProperties {
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test".to_string()).unwrap(),
                data: vec![0x01, 0x02, 0x03].into(),
            }),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_pingreq_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::PingReq(PingReq {});

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_pingresp_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::PingResp(PingResp {});

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_disconnect_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::Disconnect(Disconnect {
        reason_code: ReasonCode::NormalDisconnection,
        properties: DisconnectProperties {
            server_reference: None,
            session_expiry_interval: Some(0x12345678),
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}
#[test]
fn assert_that_suback_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::SubAck(SubAck {
        packet_id: NonZero::new(0x1234).unwrap(),
        properties: SubAckProperties {
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
        reason_codes: vec![ReasonCode::GrantedQoS0].into(),
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_unsuback_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::UnsubAck(UnsubAck {
        packet_id: NonZero::new(0x1234).unwrap(),
        properties: UnsubAckProperties {
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
        reason_codes: vec![ReasonCode::Success].into(),
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_puback_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::PubAck(PubAck {
        packet_id: NonZero::new(0x1234).unwrap(),
        reason_code: ReasonCode::Success,
        properties: PubAckProperties {
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_pubrel_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::PubRel(PubRel {
        packet_id: NonZero::new(0x1234).unwrap(),
        reason_code: ReasonCode::Success,
        properties: PubRelProperties {
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_pubrec_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::PubRec(PubRec {
        packet_id: NonZero::new(0x1234).unwrap(),
        reason_code: ReasonCode::Success,
        properties: PubRecProperties {
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_pubcomp_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::PubComp(PubComp {
        packet_id: NonZero::new(0x1234).unwrap(),
        reason_code: ReasonCode::Success,
        properties: PubCompProperties {
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_subscribe_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::Subscribe(Subscribe {
        packet_id: NonZero::new(0x1234).unwrap(),
        properties: SubscribeProperties {
            subscription_identifier: NonZero::new(0x1234),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
        subscriptions: vec1::vec1![Subscription {
            topic_filter: MQTTString::new("test".to_string()).unwrap(),
            qos: Qos::AtLeastOnce,
            no_local: true,
            retain_as_published: true,
            retain_handling: RetainHandling::DoNotSend,
        }],
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_unsubscribe_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::Unsubscribe(Unsubscribe {
        packet_id: NonZero::new(0x1234).unwrap(),
        properties: UnsubscribeProperties {
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
        topics: vec1::vec1![MQTTString::new("test".to_string()).unwrap()],
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_connack_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties {
            session_expiry_interval: Some(0x12345678),
            receive_maximum: NonZero::new(0x1234),
            maximum_qos: Some(MaximumQoS::AtMostOnce),
            retain_available: Some(true),
            maximum_packet_size: NonZero::new(0x12345678),
            assigned_client_identifier: Some(MQTTString::new("test".to_string()).unwrap()),
            topic_alias_maximum: Some(0x1234),
            reason_string: Some(MQTTString::new("test".to_string()).unwrap()),
            wildcard_subscription_available: Some(true),
            subscription_identifiers_available: Some(true),
            shared_subscription_available: Some(true),
            server_keep_alive: Some(0x1234),
            response_information: Some(MQTTString::new("test".to_string()).unwrap()),
            server_reference: Some(MQTTString::new("test".to_string()).unwrap()),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test".to_string()).unwrap(),
                data: vec![0x01, 0x02, 0x03].into(),
            }),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_publish_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: NonZero::new(0x1234).unwrap(),
            qos: GuaranteedQoS::AtLeastOnce,
            dup: true,
        },
        retain: true,
        topic: PublishTopic::new("test".to_string()).unwrap(),
        payload: vec![0x01, 0x02, 0x03].into(),
        properties: PublishProperties {
            payload_format_indicator: Some(FormatIndicator::Utf8),
            message_expiry_interval: Some(0x12345678),
            topic_alias: NonZero::new(0x1234),
            response_topic: Some(PublishTopic::new("test".to_string()).unwrap()),
            correlation_data: Some(vec![0x01, 0x02, 0x03].into()),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
            subscription_identifier: NonZero::new(0x1234),
            content_type: Some(MQTTString::new("test".to_string()).unwrap()),
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}

#[test]
fn assert_that_connect_packet_can_be_encoded_and_decoded() {
    let packet = ControlPacket::Connect(Connect {
        protocol_name: MQTTString::new("MQTT".to_string()).unwrap(),
        protocol_version: 5,
        clean_start: false,
        client_identifier: MQTTString::new("test".to_string()).unwrap(),
        will: Some(Will {
            topic: PublishTopic::new("test".to_string()).unwrap(),
            payload: vec![0x01, 0x02, 0x03].into(),
            qos: Qos::ExactlyOnce,
            retain: true,
            properties: WillProperties {
                will_delay_interval: Some(0x12345678),
                payload_format_indicator: Some(FormatIndicator::Utf8),
                message_expiry_interval: Some(0x12345678),
                content_type: Some(MQTTString::new("test".to_string()).unwrap()),
                response_topic: Some(PublishTopic::new("test".to_string()).unwrap()),
                correlation_data: Some(vec![0x01, 0x02, 0x03].into()),
                user_properties: vec![(
                    MQTTString::new("test".to_string()).unwrap(),
                    MQTTString::new("test".to_string()).unwrap(),
                )],
            },
        }),
        user_name: Some(MQTTString::new("test".to_string()).unwrap()),
        password: Some(vec![0x01, 0x02, 0x03].into()),
        keep_alive: 0x1234,
        properties: ConnectProperties {
            session_expiry_interval: Some(0x12345678),
            receive_maximum: NonZero::new(0x1234),
            maximum_packet_size: NonZero::new(0x12345678),
            topic_alias_maximum: Some(0x1234),
            request_response_information: Some(true),
            request_problem_information: Some(true),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test".to_string()).unwrap(),
                data: vec![0x01, 0x02, 0x03].into(),
            }),
            user_properties: vec![(
                MQTTString::new("test".to_string()).unwrap(),
                MQTTString::new("test".to_string()).unwrap(),
            )],
        },
    });

    let mut buffer = Vec::new();
    packet.encode(&mut buffer).unwrap();

    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(buffer.as_slice())
        .unwrap();

    assert_eq!(packet, decoded_packet);
}
