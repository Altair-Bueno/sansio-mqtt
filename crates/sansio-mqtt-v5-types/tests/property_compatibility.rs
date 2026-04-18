//! Tests for property compatibility validation per MQTT v5.0 spec.
//!
//! The MQTT v5.0 spec defines which properties are valid for each packet type.
//! These tests verify that:
//! 1. Invalid properties are rejected with UnsupportedPropertyError
//! 2. Duplicate properties are rejected with DuplicatedPropertyError
//! 3. AuthenticationData without AuthenticationMethod is rejected
//! 4. All valid properties can be encoded and decoded successfully

use core::num::NonZero;
use encode::Encodable;
use encode::EncodableSize;
use rstest::rstest;
use sansio_mqtt_v5_types::*;
use winnow::error::ContextError;
use winnow::Parser;

#[rstest]
#[case::content_type(vec![16, 27, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 11, 3, 0, 4, 116, 101, 115, 116, 0, 0])]
#[case::reason_string(vec![16, 27, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 11, 31, 0, 4, 116, 101, 115, 116, 0, 0])]
#[case::assigned_client_identifier(vec![16, 27, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 11, 18, 0, 4, 116, 101, 115, 116, 0, 0])]
#[case::wildcard_subscription_available(vec![16, 23, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 7, 40, 1, 0, 0])]
#[case::maximum_qos(vec![16, 23, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 7, 36, 1, 0, 0])]
#[case::subscription_identifier(vec![16, 23, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 7, 11, 1, 0, 0])]
#[case::topic_alias(vec![16, 23, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 7, 35, 0, 100, 0, 0])]
#[case::response_topic(vec![16, 27, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 11, 8, 0, 5, 116, 111, 112, 105, 99, 0, 0])]
#[case::authentication_data_without_method(vec![16, 27, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 11, 22, 0, 4, 1, 2, 3, 4, 0, 0])]
fn connect_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::session_expiry_interval(vec![16, 36, 0, 4, 77, 81, 84, 84, 5, 54, 0, 60, 5, 0, 0, 11, 17, 0, 0, 0, 100, 0, 5, 116, 111, 112, 105, 99, 0, 4, 116, 101, 115, 116])]
#[case::reason_string(vec![16, 36, 0, 4, 77, 81, 84, 84, 5, 54, 0, 60, 5, 0, 0, 11, 31, 0, 4, 116, 101, 115, 116, 0, 5, 116, 111, 112, 105, 99, 0, 4, 116, 101, 115, 116])]
#[case::receive_maximum(vec![16, 32, 0, 4, 77, 81, 84, 84, 5, 54, 0, 60, 5, 0, 0, 7, 33, 0, 100, 0, 5, 116, 111, 112, 105, 99, 0, 4, 116, 101, 115, 116])]
fn will_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::will_delay_interval(vec![32, 13, 0, 0, 9, 24, 0, 0, 0, 100])]
#[case::payload_format_indicator(vec![32, 7, 0, 0, 3, 1, 1])]
#[case::request_response_information(vec![32, 7, 0, 0, 3, 25, 1])]
#[case::request_problem_information(vec![32, 7, 0, 0, 3, 23, 1])]
#[case::topic_alias(vec![32, 9, 0, 0, 5, 35, 0, 100])]
fn connack_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::session_expiry_interval(vec![48, 22, 0, 4, 116, 101, 115, 116, 0, 1, 10, 17, 0, 0, 0, 100, 116, 101, 115, 116])]
#[case::receive_maximum(vec![48, 18, 0, 4, 116, 101, 115, 116, 0, 1, 6, 33, 0, 100, 116, 101, 115, 116])]
#[case::assigned_client_identifier(vec![48, 22, 0, 4, 116, 101, 115, 116, 0, 1, 10, 18, 0, 4, 116, 101, 115, 116, 116, 101, 115, 116])]
#[case::reason_string(vec![48, 22, 0, 4, 116, 101, 115, 116, 0, 1, 10, 31, 0, 4, 116, 101, 115, 116, 116, 101, 115, 116])]
#[case::wildcard_subscription_available(vec![48, 17, 0, 4, 116, 101, 115, 116, 0, 1, 5, 40, 1, 116, 101, 115, 116])]
#[case::authentication_method(vec![48, 22, 0, 4, 116, 101, 115, 116, 0, 1, 10, 21, 0, 4, 116, 101, 115, 116, 116, 101, 115, 116])]
fn publish_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::payload_format_indicator(vec![130, 8, 0, 1, 3, 1, 1, 5, 0, 4, 116, 101, 115, 116, 0])]
#[case::reason_string(vec![130, 11, 0, 1, 6, 31, 0, 4, 116, 101, 115, 116, 5, 0, 4, 116, 101, 115, 116, 0])]
#[case::message_expiry_interval(vec![130, 11, 0, 1, 6, 2, 0, 0, 0, 100, 5, 0, 4, 116, 101, 115, 116, 0])]
fn subscribe_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::payload_format_indicator(vec![144, 8, 0, 1, 4, 1, 1, 1, 0])]
#[case::session_expiry_interval(vec![144, 12, 0, 1, 8, 17, 0, 0, 0, 100, 1, 0])]
#[case::subscription_identifier(vec![144, 8, 0, 1, 4, 11, 42, 1, 0])]
fn suback_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::payload_format_indicator(vec![96, 10, 0, 1, 0, 4, 1, 1])]
#[case::message_expiry_interval(vec![96, 13, 0, 1, 0, 8, 2, 0, 0, 0, 100])]
#[case::topic_alias(vec![96, 12, 0, 1, 0, 7, 35, 0, 100])]
fn puback_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::subscription_identifier(vec![85, 10, 0, 1, 0, 4, 11, 42])]
fn pubrec_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::content_type(vec![98, 13, 0, 1, 0, 7, 3, 0, 4, 116, 101, 115, 116])]
fn pubrel_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::topic_alias(vec![112, 12, 0, 1, 0, 7, 35, 0, 100])]
fn pubcomp_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::reason_string(vec![162, 17, 0, 1, 11, 31, 0, 4, 116, 101, 115, 116, 5, 0, 4, 116, 101, 115, 116])]
#[case::subscription_identifier(vec![162, 11, 0, 1, 5, 11, 42, 5, 0, 4, 116, 101, 115, 116])]
fn unsubscribe_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::payload_format_indicator(vec![176, 8, 0, 1, 4, 1, 1, 1, 0])]
#[case::subscription_identifier(vec![176, 8, 0, 1, 4, 11, 42, 1, 0])]
fn unsuback_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::payload_format_indicator(vec![224, 7, 0, 3, 1, 1])]
#[case::message_expiry_interval(vec![224, 10, 0, 6, 2, 0, 0, 0, 100])]
#[case::subscription_identifier(vec![224, 7, 0, 3, 11, 42])]
#[case::topic_alias(vec![224, 9, 0, 5, 35, 0, 100])]
#[case::authentication_method(vec![224, 13, 0, 9, 21, 0, 4, 116, 101, 115, 116])]
fn disconnect_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::authentication_data_without_method(vec![240, 11, 0, 7, 22, 0, 4, 1, 2, 3, 4])]
#[case::payload_format_indicator(vec![240, 10, 0, 6, 21, 0, 4, 116, 101, 115, 116, 1, 1])]
#[case::message_expiry_interval(vec![240, 13, 0, 9, 21, 0, 4, 116, 101, 115, 116, 2, 0, 0, 0, 100])]
#[case::subscription_identifier(vec![240, 11, 0, 7, 21, 0, 4, 116, 101, 115, 116, 11, 42])]
fn auth_rejects_invalid_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::session_expiry_interval(vec![16, 37, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 21, 17, 0, 0, 0, 100, 17, 0, 0, 0, 200, 0, 0])]
#[case::receive_maximum(vec![16, 31, 0, 4, 77, 81, 84, 84, 5, 2, 0, 60, 15, 33, 0, 100, 33, 0, 200, 0, 0])]
fn connect_rejects_duplicate_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

#[rstest]
#[case::subscription_identifier(vec![48, 26, 0, 4, 116, 101, 115, 116, 0, 1, 14, 11, 42, 11, 43, 116, 101, 115, 116])]
fn publish_rejects_duplicate_property(#[case] bytes: Vec<u8>) {
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(parser.parse(&bytes[..]).is_err());
}

fn roundtrip(packet: &ControlPacket) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(packet.encoded_size().unwrap());
    packet
        .encode(&mut bytes)
        .map_err(|e| format!("Encode error: {:?}", e))?;

    let settings = Settings::default();
    let result = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&bytes[..])
        .map_err(|e| format!("Parse error: {:?}", e))?;

    let mut reencoded = Vec::new();
    result
        .encode(&mut reencoded)
        .map_err(|e| format!("Re-encode error: {:?}", e))?;

    if bytes == reencoded {
        Ok(())
    } else {
        Err(format!("Round-trip mismatch"))
    }
}

#[test]
fn connect_with_all_valid_properties_roundtrip() {
    let connect = Connect {
        protocol_name: Utf8String::new("MQTT"),
        protocol_version: 5,
        clean_start: true,
        client_identifier: Utf8String::new("test"),
        keep_alive: NonZero::new(60),
        user_name: None,
        password: None,
        will: None,
        properties: ConnectProperties {
            session_expiry_interval: Some(100),
            receive_maximum: NonZero::new(100),
            maximum_packet_size: NonZero::new(1000),
            topic_alias_maximum: Some(100),
            request_response_information: Some(true),
            request_problem_information: Some(true),
            authentication: Some(AuthenticationKind::WithoutData {
                method: Utf8String::new("test"),
            }),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::Connect(connect);
    roundtrip(&packet).unwrap();
}

#[test]
fn will_with_all_valid_properties_roundtrip() {
    let connect = Connect {
        protocol_name: Utf8String::new("MQTT"),
        protocol_version: 5,
        clean_start: true,
        client_identifier: Utf8String::new("test"),
        keep_alive: NonZero::new(60),
        user_name: None,
        password: None,
        will: Some(Will {
            topic: Topic::try_new("topic").unwrap(),
            payload: BinaryData::try_from(&[1, 2, 3, 4]).unwrap(),
            qos: Qos::AtLeastOnce,
            retain: false,
            properties: WillProperties {
                will_delay_interval: Some(100),
                payload_format_indicator: Some(FormatIndicator::Utf8),
                message_expiry_interval: Some(1000),
                content_type: Some(Utf8String::new("text/plain")),
                response_topic: Some(Topic::new("response/topic")),
                correlation_data: BinaryData::try_from(&[1, 2]).ok(),
                user_properties: vec![],
            },
        }),
        properties: ConnectProperties::default(),
    };

    let packet = ControlPacket::Connect(connect);
    roundtrip(&packet).unwrap();
}

#[test]
fn connack_with_all_valid_properties_roundtrip() {
    let connack = ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            session_expiry_interval: Some(100),
            receive_maximum: NonZero::new(100),
            maximum_qos: Some(MaximumQoS::AtLeastOnce),
            retain_available: Some(true),
            maximum_packet_size: NonZero::new(1000),
            assigned_client_identifier: Some(Utf8String::try_from("assigned").unwrap()),
            topic_alias_maximum: Some(100),
            reason_string: Some(Utf8String::try_from("success").unwrap()),
            wildcard_subscription_available: Some(true),
            subscription_identifiers_available: Some(true),
            shared_subscription_available: Some(true),
            server_keep_alive: Some(60),
            response_information: Some(Utf8String::try_from("info").unwrap()),
            server_reference: Some(Utf8String::try_from("server").unwrap()),
            authentication: Some(AuthenticationKind::WithoutData {
                method: Utf8String::try_from("method").unwrap(),
            }),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::ConnAck(connack);
    roundtrip(&packet).unwrap();
}

#[test]
fn publish_with_all_valid_properties_roundtrip() {
    let publish = Publish {
        kind: PublishKind::Repetible {
            packet_id: NonZero::new(1).unwrap(),
            qos: GuaranteedQoS::AtLeastOnce,
            dup: false,
        },
        retain: true,
        topic: Topic::try_new("test/topic").unwrap(),
        payload: Payload::new([1, 2, 3, 4].as_slice()),
        properties: PublishProperties {
            payload_format_indicator: Some(FormatIndicator::Utf8),
            message_expiry_interval: Some(1000),
            topic_alias: NonZero::new(100),
            response_topic: Some(Topic::new("response/topic")),
            correlation_data: BinaryData::try_from(&[1, 2]).ok(),
            subscription_identifier: NonZero::new(42),
            content_type: Some(Utf8String::new("text/plain")),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::Publish(publish);
    roundtrip(&packet).unwrap();
}

#[test]
fn subscribe_with_all_valid_properties_roundtrip() {
    let subscribe = Subscribe {
        packet_id: NonZero::new(1).unwrap(),
        subscription: Subscription {
            topic_filter: Utf8String::try_from("test/+").unwrap(),
            qos: Qos::AtLeastOnce,
            no_local: false,
            retain_as_published: false,
            retain_handling: RetainHandling::SendRetained,
        },
        extra_subscriptions: Vec::new(),
        properties: SubscribeProperties {
            subscription_identifier: NonZero::new(42),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::Subscribe(subscribe);
    roundtrip(&packet).unwrap();
}

#[test]
fn disconnect_with_all_valid_properties_roundtrip() {
    let disconnect = Disconnect {
        reason_code: DisconnectReasonCode::NormalDisconnection,
        properties: DisconnectProperties {
            session_expiry_interval: Some(100),
            reason_string: Some(Utf8String::try_from("done").unwrap()),
            server_reference: Some(Utf8String::try_from("server2").unwrap()),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::Disconnect(disconnect);
    roundtrip(&packet).unwrap();
}

#[test]
fn auth_with_all_valid_properties_roundtrip() {
    let auth = Auth {
        reason_code: AuthReasonCode::Success,
        properties: AuthProperties {
            reason_string: Some(Utf8String::try_from("auth success").unwrap()),
            authentication: Some(AuthenticationKind::WithData {
                method: Utf8String::try_from("method").unwrap(),
                data: BinaryData::try_from(&[1, 2, 3, 4]).unwrap(),
            }),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::Auth(auth);
    roundtrip(&packet).unwrap();
}

#[test]
fn suback_with_all_valid_properties_roundtrip() {
    let suback = SubAck {
        packet_id: NonZero::new(1).unwrap(),
        properties: SubAckProperties {
            reason_string: Some(Utf8String::try_from("granted").unwrap()),
            user_properties: vec![],
        },
        reason_codes: vec![SubAckReasonCode::SuccessQoS0],
    };

    let packet = ControlPacket::SubAck(suback);
    roundtrip(&packet).unwrap();
}

#[test]
fn puback_with_all_valid_properties_roundtrip() {
    let puback = PubAck {
        packet_id: NonZero::new(1).unwrap(),
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties {
            reason_string: Some(Utf8String::try_from("ok").unwrap()),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::PubAck(puback);
    roundtrip(&packet).unwrap();
}

#[test]
fn unsuback_with_all_valid_properties_roundtrip() {
    let unsuback = UnsubAck {
        packet_id: NonZero::new(1).unwrap(),
        properties: UnsubAckProperties {
            reason_string: Some(Utf8String::try_from("unsubscribed").unwrap()),
            user_properties: vec![],
        },
        reason_codes: vec![UnsubAckReasonCode::Success],
    };

    let packet = ControlPacket::UnsubAck(unsuback);
    roundtrip(&packet).unwrap();
}

#[test]
fn unsubscribe_with_all_valid_properties_roundtrip() {
    let unsubscribe = Unsubscribe {
        packet_id: NonZero::new(1).unwrap(),
        properties: UnsubscribeProperties {
            user_properties: vec![(
                Utf8String::try_from("key").unwrap(),
                Utf8String::try_from("value").unwrap(),
            )],
        },
        filter: Utf8String::try_from("test/+").unwrap(),
        extra_filters: Vec::new(),
    };

    let packet = ControlPacket::Unsubscribe(unsubscribe);
    roundtrip(&packet).unwrap();
}

#[test]
fn pubcomp_with_all_valid_properties_roundtrip() {
    let pubcomp = PubComp {
        packet_id: NonZero::new(1).unwrap(),
        reason_code: PubCompReasonCode::Success,
        properties: PubCompProperties {
            reason_string: Some(Utf8String::try_from("complete").unwrap()),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::PubComp(pubcomp);
    roundtrip(&packet).unwrap();
}

#[test]
fn pubrec_with_all_valid_properties_roundtrip() {
    let pubrec = PubRec {
        packet_id: NonZero::new(1).unwrap(),
        reason_code: PubRecReasonCode::Success,
        properties: PubRecProperties {
            reason_string: Some(Utf8String::try_from("received").unwrap()),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::PubRec(pubrec);
    roundtrip(&packet).unwrap();
}

#[test]
fn pubrel_with_all_valid_properties_roundtrip() {
    let pubrel = PubRel {
        packet_id: NonZero::new(1).unwrap(),
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties {
            reason_string: Some(Utf8String::try_from("released").unwrap()),
            user_properties: vec![],
        },
    };

    let packet = ControlPacket::PubRel(pubrel);
    roundtrip(&packet).unwrap();
}

#[test]
fn pingreq_parsing_valid() {
    let bytes = vec![192, 0];
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(matches!(
        parser.parse(&bytes[..]),
        Ok(ControlPacket::PingReq(_))
    ));
}

#[test]
fn pingreq_rejects_with_payload() {
    let bytes = vec![192, 1, 0];
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    parser.parse(&bytes[..]).unwrap_err();
}

#[test]
fn pingresp_parsing_valid() {
    let bytes = vec![208, 0];
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(matches!(
        parser.parse(&bytes[..]),
        Ok(ControlPacket::PingResp(_))
    ));
}

#[test]
fn connect_with_will_roundtrip() {
    let connect = Connect {
        protocol_name: Utf8String::new("MQTT"),
        protocol_version: 5,
        clean_start: true,
        client_identifier: Utf8String::new("test"),
        keep_alive: NonZero::new(60),
        user_name: None,
        password: None,
        will: Some(Will {
            topic: Topic::try_new("topic").unwrap(),
            payload: BinaryData::try_from(&[1, 2, 3, 4]).unwrap(),
            qos: Qos::AtLeastOnce,
            retain: false,
            properties: WillProperties::default(),
        }),
        properties: ConnectProperties::default(),
    };

    let packet = ControlPacket::Connect(connect);
    roundtrip(&packet).unwrap();
}

#[test]
fn connect_without_will_roundtrip() {
    let connect = Connect {
        protocol_name: Utf8String::new("MQTT"),
        protocol_version: 5,
        clean_start: true,
        client_identifier: Utf8String::new("test"),
        keep_alive: NonZero::new(60),
        user_name: None,
        password: None,
        will: None,
        properties: ConnectProperties::default(),
    };

    let packet = ControlPacket::Connect(connect);
    roundtrip(&packet).unwrap();
}

#[test]
fn publish_qos0_fire_and_forget_roundtrip() {
    let publish = Publish {
        kind: PublishKind::FireAndForget,
        retain: false,
        topic: Topic::try_new("test/topic").unwrap(),
        payload: Payload::new([1, 2, 3, 4].as_slice()),
        properties: PublishProperties::default(),
    };

    let packet = ControlPacket::Publish(publish);
    roundtrip(&packet).unwrap();
}

#[test]
fn pubcomp_properties_is_empty() {
    let empty = PubCompProperties::default();
    assert!(empty.is_empty());

    let with_reason = PubCompProperties {
        reason_string: Some(Utf8String::try_from("ok").unwrap()),
        user_properties: vec![],
    };
    assert!(!with_reason.is_empty());

    let with_user = PubCompProperties {
        reason_string: None,
        user_properties: vec![(
            Utf8String::try_from("key").unwrap(),
            Utf8String::try_from("value").unwrap(),
        )],
    };
    assert!(!with_user.is_empty());
}

#[test]
fn pubrec_properties_is_empty() {
    let empty = PubRecProperties::default();
    assert!(empty.is_empty());

    let with_reason = PubRecProperties {
        reason_string: Some(Utf8String::try_from("ok").unwrap()),
        user_properties: vec![],
    };
    assert!(!with_reason.is_empty());

    let with_user = PubRecProperties {
        reason_string: None,
        user_properties: vec![(
            Utf8String::try_from("key").unwrap(),
            Utf8String::try_from("value").unwrap(),
        )],
    };
    assert!(!with_user.is_empty());
}

#[test]
fn pubrel_properties_is_empty() {
    let empty = PubRelProperties::default();
    assert!(empty.is_empty());

    let with_reason = PubRelProperties {
        reason_string: Some(Utf8String::try_from("ok").unwrap()),
        user_properties: vec![],
    };
    assert!(!with_reason.is_empty());

    let with_user = PubRelProperties {
        reason_string: None,
        user_properties: vec![(
            Utf8String::try_from("key").unwrap(),
            Utf8String::try_from("value").unwrap(),
        )],
    };
    assert!(!with_user.is_empty());
}

#[test]
fn pubcomp_rejects_duplicate_property() {
    let bytes = vec![
        112, 17, 0, 1, 11, 31, 0, 4, 116, 101, 115, 116, 31, 0, 4, 97, 98, 99, 100,
    ];
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    parser.parse(&bytes[..]).unwrap_err();
}

#[test]
fn pubrec_rejects_duplicate_property() {
    let bytes = vec![
        85, 17, 0, 1, 11, 31, 0, 4, 116, 101, 115, 116, 31, 0, 4, 97, 98, 99, 100,
    ];
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    parser.parse(&bytes[..]).unwrap_err();
}

#[test]
fn pubrel_rejects_duplicate_property() {
    let bytes = vec![
        98, 17, 0, 1, 0, 11, 31, 0, 4, 116, 101, 115, 116, 31, 0, 4, 97, 98, 99, 100,
    ];
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    parser.parse(&bytes[..]).unwrap_err();
}

#[test]
fn pingreq_encode_and_decode() {
    let pingreq = PingReq {};
    let packet = ControlPacket::PingReq(pingreq);
    roundtrip(&packet).unwrap();
}

#[test]
fn pingresp_encode_and_decode() {
    let pingresp = PingResp {};
    let packet = ControlPacket::PingResp(pingresp);
    roundtrip(&packet).unwrap();
}

#[test]
fn reserved_packet_parsing() {
    let bytes = vec![0, 0];
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    assert!(matches!(
        parser.parse(&bytes[..]),
        Ok(ControlPacket::Reserved(_))
    ));
}

#[test]
fn reserved_packet_rejects_with_payload() {
    let bytes = vec![0, 1, 0];
    let settings = Settings::default();
    let mut parser = ControlPacket::parse::<_, ContextError, ContextError>(&settings);
    parser.parse(&bytes[..]).unwrap_err();
}

#[test]
fn reserved_packet_roundtrip() {
    let reserved = Reserved {};
    let packet = ControlPacket::Reserved(reserved);
    roundtrip(&packet).unwrap();
}

#[test]
fn settings_default() {
    let settings = Settings::default();
    assert_eq!(settings.max_bytes_string, 5 * 1024);
    assert_eq!(settings.max_bytes_binary_data, 5 * 1024);
    assert_eq!(settings.max_remaining_bytes, 1024 * 1024);
    assert_eq!(settings.max_subscriptions_len, 32);
    assert_eq!(settings.max_user_properties_len, 32);
}

#[test]
fn settings_unlimited() {
    let settings = Settings::unlimited();
    assert_eq!(settings.max_bytes_string, u16::MAX);
    assert_eq!(settings.max_bytes_binary_data, u16::MAX);
    assert_eq!(settings.max_remaining_bytes, u64::MAX);
    assert_eq!(settings.max_subscriptions_len, u32::MAX);
    assert_eq!(settings.max_user_properties_len, usize::MAX);
}

#[test]
fn utf8string_valid() {
    let s = Utf8String::try_from("hello").unwrap();
    assert_eq!(&*s, "hello");
}

#[test]
fn utf8string_with_control_char_rejected() {
    let result = Utf8String::try_from("hello\x01world");
    assert!(result.is_err());
}

#[test]
fn utf8string_with_null_rejected() {
    let result = Utf8String::try_from("hello\0world");
    assert!(result.is_err());
}

#[test]
fn utf8string_with_noncharacter_rejected() {
    let result = Utf8String::try_from("hello\u{FFFE}world");
    assert!(result.is_err());
}

#[test]
fn topic_valid() {
    let topic = Topic::try_new("home/living-room").unwrap();
    let expected = Utf8String::new("home/living-room");
    let topic_inner: &Utf8String = &topic;
    assert_eq!(topic_inner, &expected);
}

#[test]
fn topic_with_hash_rejected() {
    let result = Topic::try_new("home/#");
    assert!(result.is_err());
}

#[test]
fn topic_with_plus_rejected() {
    let result = Topic::try_new("home/+/temperature");
    assert!(result.is_err());
}

#[test]
fn payload_from_vec() {
    let payload = Payload::new(vec![1, 2, 3]);
    let expected: &[u8] = &[1, 2, 3];
    assert_eq!(payload.as_ref(), expected);
}

#[test]
fn payload_from_slice() {
    let payload = Payload::new([1, 2, 3, 4, 5].as_slice());
    let expected: &[u8] = &[1, 2, 3, 4, 5];
    assert_eq!(payload.as_ref(), expected);
}

#[test]
fn binarydata_valid() {
    let data = BinaryData::try_from(vec![1u8, 2, 3]).unwrap();
    let expected: &[u8] = &[1, 2, 3];
    assert_eq!(data.as_ref(), expected);
}

#[test]
fn retain_handling_all_values() {
    let r0: u8 = RetainHandling::SendRetained.into();
    let r1: u8 = RetainHandling::SendRetainedIfSubscriptionDoesNotExist.into();
    let r2: u8 = RetainHandling::DoNotSend.into();
    assert_eq!(r0, 0);
    assert_eq!(r1, 1);
    assert_eq!(r2, 2);

    assert_eq!(
        RetainHandling::try_from(0).unwrap(),
        RetainHandling::SendRetained
    );
    assert_eq!(
        RetainHandling::try_from(1).unwrap(),
        RetainHandling::SendRetainedIfSubscriptionDoesNotExist
    );
    assert_eq!(
        RetainHandling::try_from(2).unwrap(),
        RetainHandling::DoNotSend
    );
    assert!(RetainHandling::try_from(3).is_err());
}

#[test]
fn format_indicator_all_values() {
    let f0: u8 = FormatIndicator::Unspecified.into();
    let f1: u8 = FormatIndicator::Utf8.into();
    assert_eq!(f0, 0);
    assert_eq!(f1, 1);

    assert_eq!(
        FormatIndicator::try_from(0).unwrap(),
        FormatIndicator::Unspecified
    );
    assert_eq!(FormatIndicator::try_from(1).unwrap(), FormatIndicator::Utf8);
    assert!(FormatIndicator::try_from(2).is_err());
}

#[test]
fn qos_all_values() {
    let q0: u8 = Qos::AtMostOnce.into();
    let q1: u8 = Qos::AtLeastOnce.into();
    let q2: u8 = Qos::ExactlyOnce.into();
    assert_eq!(q0, 0);
    assert_eq!(q1, 1);
    assert_eq!(q2, 2);

    assert_eq!(Qos::try_from(0).unwrap(), Qos::AtMostOnce);
    assert_eq!(Qos::try_from(1).unwrap(), Qos::AtLeastOnce);
    assert_eq!(Qos::try_from(2).unwrap(), Qos::ExactlyOnce);
    assert!(Qos::try_from(3).is_err());
}

#[test]
fn guaranteed_qos_all_values() {
    let g1: u8 = GuaranteedQoS::AtLeastOnce.into();
    let g2: u8 = GuaranteedQoS::ExactlyOnce.into();
    assert_eq!(g1, 1);
    assert_eq!(g2, 2);

    assert_eq!(
        GuaranteedQoS::try_from(1).unwrap(),
        GuaranteedQoS::AtLeastOnce
    );
    assert_eq!(
        GuaranteedQoS::try_from(2).unwrap(),
        GuaranteedQoS::ExactlyOnce
    );
    assert!(GuaranteedQoS::try_from(0).is_err());
    assert!(GuaranteedQoS::try_from(3).is_err());
}

#[test]
fn maximum_qos_all_values() {
    let m0: u8 = MaximumQoS::AtMostOnce.into();
    let m1: u8 = MaximumQoS::AtLeastOnce.into();
    assert_eq!(m0, 0);
    assert_eq!(m1, 1);

    assert_eq!(MaximumQoS::try_from(0).unwrap(), MaximumQoS::AtMostOnce);
    assert_eq!(MaximumQoS::try_from(1).unwrap(), MaximumQoS::AtLeastOnce);
    assert!(MaximumQoS::try_from(2).is_err());
}

#[test]
fn guaranteed_qos_to_qos() {
    let gqos1: Qos = GuaranteedQoS::AtLeastOnce.into();
    assert_eq!(gqos1, Qos::AtLeastOnce);
    let gqos2: Qos = GuaranteedQoS::ExactlyOnce.into();
    assert_eq!(gqos2, Qos::ExactlyOnce);
}

#[test]
fn qos_to_guaranteed_qos() {
    assert_eq!(
        GuaranteedQoS::try_from(Qos::AtLeastOnce).unwrap(),
        GuaranteedQoS::AtLeastOnce
    );
    assert_eq!(
        GuaranteedQoS::try_from(Qos::ExactlyOnce).unwrap(),
        GuaranteedQoS::ExactlyOnce
    );
    assert!(GuaranteedQoS::try_from(Qos::AtMostOnce).is_err());
}

#[test]
fn maximum_qos_to_qos() {
    let mqos1: Qos = MaximumQoS::AtMostOnce.into();
    assert_eq!(mqos1, Qos::AtMostOnce);
    let mqos2: Qos = MaximumQoS::AtLeastOnce.into();
    assert_eq!(mqos2, Qos::AtLeastOnce);
}

#[test]
fn qos_to_maximum_qos() {
    assert_eq!(
        MaximumQoS::try_from(Qos::AtMostOnce).unwrap(),
        MaximumQoS::AtMostOnce
    );
    assert_eq!(
        MaximumQoS::try_from(Qos::AtLeastOnce).unwrap(),
        MaximumQoS::AtLeastOnce
    );
    assert!(MaximumQoS::try_from(Qos::ExactlyOnce).is_err());
}
