use std::num::NonZero;

use encode::Encodable;
use sansio_mqtt5::parser::Settings;
use sansio_mqtt5::types::Auth;
use sansio_mqtt5::types::AuthProperties;
use sansio_mqtt5::types::AuthenticationKind;
use sansio_mqtt5::types::ControlPacket;
use sansio_mqtt5::types::Disconnect;
use sansio_mqtt5::types::DisconnectProperties;
use sansio_mqtt5::types::MQTTString;
use sansio_mqtt5::types::PingReq;
use sansio_mqtt5::types::PingResp;
use sansio_mqtt5::types::PubRel;
use sansio_mqtt5::types::PubRelProperties;
use sansio_mqtt5::types::ReasonCode;
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
    let packet = ControlPacket::SubAck(sansio_mqtt5::types::SubAck {
        packet_id: NonZero::new(0x1234).unwrap(),
        properties: sansio_mqtt5::types::SubAckProperties {
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
    let packet = ControlPacket::UnsubAck(sansio_mqtt5::types::UnsubAck {
        packet_id: NonZero::new(0x1234).unwrap(),
        properties: sansio_mqtt5::types::UnsubAckProperties {
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
    let packet = ControlPacket::PubAck(sansio_mqtt5::types::PubAck {
        packet_id: NonZero::new(0x1234).unwrap(),
        reason_code: ReasonCode::Success,
        properties: sansio_mqtt5::types::PubAckProperties {
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
    let packet = ControlPacket::PubRec(sansio_mqtt5::types::PubRec {
        packet_id: NonZero::new(0x1234).unwrap(),
        reason_code: ReasonCode::Success,
        properties: sansio_mqtt5::types::PubRecProperties {
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
    let packet = ControlPacket::PubComp(sansio_mqtt5::types::PubComp {
        packet_id: NonZero::new(0x1234).unwrap(),
        reason_code: ReasonCode::Success,
        properties: sansio_mqtt5::types::PubCompProperties {
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