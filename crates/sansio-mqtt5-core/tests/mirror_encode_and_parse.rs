//! Adapted from https://github.com/mqttjs/mqtt-packet/blob/e39fb28c10628720fb50e5eb355492684ff0caaa/test.js
//!
//! Note that some tests were not spec complaint, so they were adapted.

use encode::Encodable;
use encode::EncodableSize;
use sansio_mqtt5_core::parser::Settings;
use sansio_mqtt5_core::types::*;
use winnow::error::ContextError;
use winnow::Parser;

#[test]
#[should_panic]
fn invalid_variable_byte_integer() {
    let input = [16, 255, 255, 255, 255];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_variable_byte_integer2() {
    let input = [16, 255, 255, 255, 128];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_variable_byte_integer3() {
    let input = [16, 255, 255, 255, 255, 1];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_variable_byte_integer4() {
    let input = [16, 255, 255, 255, 255, 127];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_variable_byte_integer5() {
    let input = [16, 255, 255, 255, 255, 128];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_variable_byte_integer6() {
    let input = [16, 255, 255, 255, 255, 255, 1];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_connect_protocol_id() {
    let input = [16, 4, 0, 6, 77, 81];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_connect_missing_protocol_version() {
    let input = [
        16, 8, // Header
        0, 6, // Protocol ID length
        77, 81, 73, 115, 100, 112, // Protocol ID
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_connect_missing_keep_alive() {
    let input = [
        16, 10, // Header
        0, 6, // Protocol ID length
        77, 81, 73, 115, 100, 112, // Protocol ID
        3,   // Protocol version
        246, // Connect flags
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_connect_missing_client_id() {
    let input = [
        16, 10, // Header
        0, 6, // Protocol ID length
        77, 81, 73, 115, 100, 112, // Protocol ID
        3,   // Protocol version
        246, // Connect flags
        0, 30, // Keepalive
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_connect_missing_will_topic() {
    let input = [
        16, 16, // Header
        0, 6, // Protocol ID length
        77, 81, 73, 115, 100, 112, // Protocol ID
        3,   // Protocol version
        246, // Connect flags
        0, 30, // Keepalive
        0, 2, // Will topic length
        0, 0, // Will topic
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}
#[test]
#[should_panic]
fn invalid_connect_missing_will_payload() {
    let input = [
        16, 23, // Header
        0, 6, // Protocol ID length
        77, 81, 73, 115, 100, 112, // Protocol ID
        3,   // Protocol version
        246, // Connect flags
        0, 30, // Keepalive
        0, 5, // Will topic length
        116, 111, 112, 105, 99, // Will topic
        0, 2, // Will payload length
        0, 0, // Will payload
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}
#[test]
#[should_panic]
fn invalid_username_on_connect() {
    let input = [
        16, 32, // Header
        0, 6, // Protocol ID length
        77, 81, 73, 115, 100, 112, // Protocol ID
        3,   // Protocol version
        246, // Connect flags
        0, 30, // Keepalive
        0, 5, // Will topic length
        116, 111, 112, 105, 99, // Will topic
        0, 7, // Will payload length
        112, 97, 121, 108, 111, 97, 100, // Will payload
        0, 2, // Username length
        0, 0, // Username
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_connect_missing_password() {
    let input = [
        16, 42, // Header
        0, 6, // Protocol ID length
        77, 81, 73, 115, 100, 112, // Protocol ID
        3,   // Protocol version
        246, // Connect flags
        0, 30, // Keepalive
        0, 5, // Will topic length
        116, 111, 112, 105, 99, // Will topic
        0, 7, // Will payload length
        112, 97, 121, 108, 111, 97, 100, // Will payload
        0, 8, // Username length
        117, 115, 101, 114, 110, 97, 109, 101, // Username
        0, 2, // Password length
        0, 0, // Password
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_header_flags_bits_on_connect() {
    let input = [
        18, 10, // Header
        0, 4, // Protocol ID length
        0x4d, 0x51, 0x54, 0x54, // Protocol ID
        3,    // Protocol version
        2,    // Connect flags
        0, 30, // Keepalive
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}
#[test]
#[should_panic]
fn invalid_connect_flag_bit_0_must_be_0() {
    let input = [
        16, 10, // Header
        0, 4, // Protocol ID length
        0x4d, 0x51, 0x54, 0x54, // Protocol ID
        3,    // Protocol version
        3,    // Connect flags
        0, 30, // Keepalive
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_publish_will_retain_flag_must_be_zero_when_will_flag_is_zero() {
    let input = [
        16, 10, // Header
        0, 4, // Protocol ID length
        0x4d, 0x51, 0x54, 0x54, // Protocol ID
        3,    // Protocol version
        0x22, // Connect flags
        0, 30, // Keepalive
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_invalid_will_qos_must_be_zero_when_will_flag_is_zero() {
    let input = [
        16, 10, // Header
        0, 4, // Protocol ID length
        0x4d, 0x51, 0x54, 0x54, // Protocol ID
        3,    // Protocol version
        0x12, // Connect flags
        0, 30, // Keepalive
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}
#[test]
#[should_panic]
fn invalid_publish_will_qos_must_be_zero_when_will_flag_is_set_to_zero() {
    let input = [
        16, 10, // Header
        0, 4, // Protocol ID length
        0x4d, 0x51, 0x54, 0x54, // Protocol ID
        3,    // Protocol version
        0xa,  // Connect flags
        0, 30, // Keepalive
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_connect_packet_too_short() {
    let input = [
        16, // Header
        8,  // Packet length
        0, 4, // Protocol ID length
        77, 81, 84, 84, // MQTT
        5,  // Version
        2,  // Clean Start enabled
        0, 0, // Keep-Alive
        0, // Property Length
        0, 0, // Properties
           // No payload
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_subscribe_no_payload() {
    let input = [
        130, // Header
        0,   // Packet length
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_suback_no_payload() {
    let input = [
        144, // Header
        0,   // Packet length
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_unsubscribe_no_payload() {
    let input = [
        162, // Header
        0,   // Packet length
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_unsuback_v5_no_payload() {
    let input = [
        176, // Header
        0,   // Packet length
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_unsuback_v3_payload_length_must_be_2() {
    let input = [
        176, // Header
        1,   // Packet length
        1,
    ];
    let settings = Settings::default();

    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_header_flag_bits_must_be_0x0_for_connack_packet() {
    let input = [
        33, 2, // header
        0, // flags
        5, // return code
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}
#[test]
#[should_panic]
fn invalid_connack_flags_bits_7_1_must_be_set_to_0() {
    let input = [
        32, 2, // header
        2, // flags
        5, // return code
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_return_code() {
    let input = [
        32, 2,    // header
        0,    // flags
        b'5', // return code as character
    ];
    let settings = Settings::default();
    ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
}

#[test]
#[should_panic]
fn invalid_publish_packet_due_to_repeated_subscription_identifiers_property() {
    let input = [
        61, 22, // Header
        0, 4, // Topic length
        116, 101, 115, 116, // Topic (test)
        0, 10, // Message ID
        9,  // properties length
        1, 0, // payloadFormatIndicator
        11, 1, // subscriptionIdentifier
        11, 255, 255, 255, 127, // subscriptionIdentifier (max value)
        116, 101, 115, 116, // Payload (test)
    ];
    let packet = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: NonZero::new(10).unwrap(),
            qos: GuaranteedQoS::ExactlyOnce,
            dup: true,
        },
        retain: true,
        topic: PublishTopic::new("test").unwrap(),
        payload: [116, 101, 115, 116][..].into(),
        properties: PublishProperties {
            payload_format_indicator: Some(FormatIndicator::Unspecified),
            subscription_identifier: NonZero::new(u64::MAX),
            ..Default::default()
        },
    });
    let settings = Settings::default();
    let result = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&input[..])
        .unwrap();
    assert_eq!(result, packet);
}

#[rstest::rstest]
#[case::connect_mqtt5(
    vec! [
        16, 125, // Header
        0, 4, // Protocol ID length
        77, 81, 84, 84, // Protocol ID
        5,  // Protocol version
        54, // Connect flags
        0, 30, // Keepalive
        47, // properties length
        17, 0, 0, 4, 210, // sessionExpiryInterval
        33, 1, 176, // receiveMaximum
        39, 0, 0, 0, 100, // maximumPacketSize
        34, 1, 200, // topicAliasMaximum
        25, 1, // requestResponseInformation
        23, 1, // requestProblemInformation,
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // userProperties,
        21, 0, 4, 116, 101, 115, 116, // authenticationMethod
        22, 0, 4, 1, 2, 3, 4, // authenticationData
        0, 4, // Client ID length
        116, 101, 115, 116, // Client ID
        47,  // will properties
        24, 0, 0, 4, 210, // will delay interval
        1, 0, // payload format indicator
        2, 0, 0, 16, 225, // message expiry interval
        3, 0, 4, 116, 101, 115, 116, // content type
        8, 0, 5, 116, 111, 112, 105, 99, // response topic
        9, 0, 4, 1, 2, 3, 4, // corelation data
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // user properties
        0, 5, // Will topic length
        116, 111, 112, 105, 99, // Will topic
        0, 4, // Will payload length
        4, 3, 2, 1, // Will payload
    ],
    ControlPacket::Connect(Connect {
        protocol_name: MQTTString::new("MQTT").unwrap(),
        protocol_version: 5,
        clean_start: true,
        client_identifier: MQTTString::new("test").unwrap(),
        keep_alive: 30,
        user_name: None,
        password: None,
        will: Some(Will {
            topic: PublishTopic::new("topic").unwrap(),
            payload: [4, 3, 2, 1][..].into(),
            qos: Qos::ExactlyOnce,
            retain: true,
            properties: WillProperties {
                will_delay_interval: Some(1234),
                payload_format_indicator: Some(FormatIndicator::Unspecified),
                message_expiry_interval: Some(4321),
                content_type: MQTTString::new("test"),
                response_topic: PublishTopic::new("topic"),
                correlation_data: Some([1, 2, 3, 4][..].into()),
                user_properties: vec! [(
                    MQTTString::new("test").unwrap(),
                    MQTTString::new("test").unwrap(),
                )],
            },
        }),
        properties: ConnectProperties {
            session_expiry_interval: Some(1234),
            receive_maximum: NonZero::new(432),
            maximum_packet_size: NonZero::new(100),
            topic_alias_maximum: Some(456),
            request_response_information: Some(true),
            request_problem_information: Some(true),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test").unwrap(),
                data: [1, 2, 3, 4][..].into(),
            }),
            user_properties: vec! [(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
    })
)]
#[case::connect_mqtt_5_with_will_properties_but_with_empty_will_payload(
    vec! [
        16, 121, // Header
        0, 4, // Protocol ID length
        77, 81, 84, 84, // Protocol ID
        5,  // Protocol version
        54, // Connect flags
        0, 30, // Keepalive
        47, // properties length
        17, 0, 0, 4, 210, // sessionExpiryInterval
        33, 1, 176, // receiveMaximum
        39, 0, 0, 0, 100, // maximumPacketSize
        34, 1, 200, // topicAliasMaximum
        25, 1, // requestResponseInformation
        23, 1, // requestProblemInformation,
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // userProperties,
        21, 0, 4, 116, 101, 115, 116, // authenticationMethod
        22, 0, 4, 1, 2, 3, 4, // authenticationData
        0, 4, // Client ID length
        116, 101, 115, 116, // Client ID
        47,  // will properties
        24, 0, 0, 4, 210, // will delay interval
        1, 0, // payload format indicator
        2, 0, 0, 16, 225, // message expiry interval
        3, 0, 4, 116, 101, 115, 116, // content type
        8, 0, 5, 116, 111, 112, 105, 99, // response topic
        9, 0, 4, 1, 2, 3, 4, // corelation data
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // user properties
        0, 5, // Will topic length
        116, 111, 112, 105, 99, // Will topic
        0, 0, // Will payload length
    ],
    ControlPacket::Connect(Connect {
        protocol_name: MQTTString::new("MQTT").unwrap(),
        protocol_version: 5,
        clean_start: true,
        client_identifier: MQTTString::new("test").unwrap(),
        keep_alive: 30,
        user_name: None,
        password: None,
        will: Some(Will {
            topic: PublishTopic::new("topic").unwrap(),
            payload: [][..].into(),
            qos: Qos::ExactlyOnce,
            retain: true,
            properties: WillProperties {
                will_delay_interval: Some(1234),
                payload_format_indicator: Some(FormatIndicator::Unspecified),
                message_expiry_interval: Some(4321),
                content_type: MQTTString::new("test"),
                response_topic: PublishTopic::new("topic"),
                correlation_data: Some([1, 2, 3, 4][..].into()),
                user_properties: vec! [(
                    MQTTString::new("test").unwrap(),
                    MQTTString::new("test").unwrap(),
                )],
            },
        }),
        properties: ConnectProperties {
            session_expiry_interval: Some(1234),
            receive_maximum: NonZero::new(432),
            maximum_packet_size: NonZero::new(100),
            topic_alias_maximum: Some(456),
            request_response_information: Some(true),
            request_problem_information: Some(true),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test").unwrap(),
                data: [1, 2, 3, 4][..].into(),
            }),
            user_properties: vec! [(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
    })
)]
#[case::connect_mqtt_5_without_will_properties(
    vec! [
        16, 78, // Header
        0, 4, // Protocol ID length
        77, 81, 84, 84, // Protocol ID
        5,  // Protocol version
        54, // Connect flags
        0, 30, // Keepalive
        47, // properties length
        17, 0, 0, 4, 210, // sessionExpiryInterval
        33, 1, 176, // receiveMaximum
        39, 0, 0, 0, 100, // maximumPacketSize
        34, 1, 200, // topicAliasMaximum
        25, 1, // requestResponseInformation
        23, 1, // requestProblemInformation,
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // userProperties,
        21, 0, 4, 116, 101, 115, 116, // authenticationMethod
        22, 0, 4, 1, 2, 3, 4, // authenticationData
        0, 4, // Client ID length
        116, 101, 115, 116, // Client ID
        0,   // will properties
        0, 5, // Will topic length
        116, 111, 112, 105, 99, // Will topic
        0, 4, // Will payload length
        4, 3, 2, 1, // Will payload
    ],
    ControlPacket::Connect(Connect {
        protocol_name: MQTTString::new("MQTT").unwrap(),
        protocol_version: 5,
        clean_start: true,
        client_identifier: MQTTString::new("test").unwrap(),
        keep_alive: 30,
        user_name: None,
        password: None,
        will: Some(Will {
            topic: PublishTopic::new("topic").unwrap(),
            payload: [4, 3, 2, 1][..].into(),
            qos: Qos::ExactlyOnce,
            retain: true,
            properties: WillProperties::default(),
        }),
        properties: ConnectProperties {
            session_expiry_interval: Some(1234),
            receive_maximum: NonZero::new(432),
            maximum_packet_size: NonZero::new(100),
            topic_alias_maximum: Some(456),
            request_response_information: Some(true),
            request_problem_information: Some(true),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test").unwrap(),
                data: [1, 2, 3, 4][..].into(),
            }),
            user_properties: vec! [(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
    })
)]
#[case::no_clientid_with_5(
    vec! [
        16, 16, // Header
        0, 4, // Protocol ID length
        77, 81, 84, 84, // Protocol ID
        5, // Protocol version
        2, // Connect flags
        0, 60, // Keepalive
        3, // Property Length
        33, 0, 20, // receiveMaximum
        0, 0, // Client ID length
    ],
    ControlPacket::Connect(Connect {
        protocol_name: MQTTString::new("MQTT").unwrap(),
        protocol_version: 5,
        clean_start: true,
        client_identifier: MQTTString::new("").unwrap(),
        keep_alive: 60,
        user_name: None,
        password: None,
        will: None,
        properties: ConnectProperties {
            receive_maximum: NonZero::new(20),
            ..Default::default()
        },
    })
)]
// Note: The original test was not MQTT5 compliant because the properties were missing.
#[case::utf8_clientid_with_5(
    vec! [
        16, 24, // Header
        0, 4, // Protocol ID length
        77, 81, 84, 84, // Protocol ID
        5,  // Protocol version
        2,  // Connect flags
        0, 30, // Keepalive
        0, // Property Length
        0, 11, // Client ID length
        197, 166, // Ŧ (UTF-8: 0xc5a6)
        196, 151, // ė (UTF-8: 0xc497)
        197, 155, // ś (utf-8: 0xc59b)
        116, // t (utf-8: 0x74)
        240, 159, 156, 132, // 🜄 (utf-8: 0xf09f9c84)
    ],
    ControlPacket::Connect(Connect {
        protocol_name: MQTTString::new("MQTT").unwrap(),
        protocol_version: 5,
        clean_start: true,
        client_identifier: MQTTString::new("Ŧėśt🜄").unwrap(),
        keep_alive: 30,
        user_name: None,
        password: None,
        will: None,
        properties: ConnectProperties::default(),
    })
)]
#[case::version_5_conack(
    vec! [
        32, 3, // Fixed Header (CONNACK, Remaining Length)
        0, 140, // Variable Header (Session not present, Bad authentication method)
        0,   // Property Length Zero
    ],
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::BadAuthenticationMethod,
        },
        properties: ConnAckProperties::default(),
    })
)]
#[case::version_5_puback(
    vec! [
        64, 2, // Fixed Header (PUBACK, Remaining Length)
        0, 42, // Variable Header (Message ID)
    ],
    ControlPacket::PubAck(PubAck {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    })
)]
// Note: This test had a reason code of 0, which by itself is not valid.
#[case::version_5_puback_2_1(
    vec! [
        64, 2, // Fixed Header (PUBACK, Remaining Length)
        0, 42,
    ],
    ControlPacket::PubAck(PubAck {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    })
)]
#[case::version_5_puback_3(
    vec! [
        64, 4, // Fixed Header (PUBACK, Remaining Length)
        0, 42, 0, // Variable Header (2 Bytes: Packet Identifier 42, Reason code: 0 Success)
        0, // no properties
    ],
    ControlPacket::PubAck(PubAck {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: PubAckReasonCode::Success,
        properties: PubAckProperties::default(),
    })
)]
// Note: The original test was not MQTT5 compliant because the properties and reason code were missing.
#[case::version_5_connack_1(
    vec! [
        32, 3, // Fixed Header (CONNACK, Remaining Length)
        1, // Variable Header. Session present set
        0, 0,
    ],
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    })
)]
#[case::version_5_connack_3(
    vec! [
        32, 3, // Fixed Header (CONNACK, Remaining Length)
        1, 0, // Variable Header (Session Present: 1 => true, Implied Reason code: Success)
        0, // no properties
    ],
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::ResumePreviousSession,
        properties: ConnAckProperties::default(),
    })
)]
#[case::version_5_disconnect_1(
    vec! [
        224, 0, // Fixed Header (DISCONNECT, Remaining Length)
    ],
    ControlPacket::Disconnect(Disconnect {
        reason_code: DisconnectReasonCode::NormalDisconnection,
        properties: DisconnectProperties::default(),
    })
)]
#[case::version_5_disconnect_2(
    vec! [
        224, 2, // Fixed Header (DISCONNECT, Remaining Length)
        0, // Variable Header (Reason code: 0 Success)
        0, // no properties
    ],
    ControlPacket::Disconnect(Disconnect {
        reason_code: DisconnectReasonCode::NormalDisconnection,
        properties: DisconnectProperties::default(),
    })
)]
// Note: This test was invalid because the properties were missing.
#[case::connack_with_return_code_0(
    vec! [
        32, 3, // Fixed Header (CONNACK, Remaining Length)
        0, 0, // Variable Header (Session not present, Return code: 0 Success)
        0, // Property Length Zero
    ],
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties::default(),
    })
)]
#[case::connack_mqtt_5_with_properties(
    vec! [
        32, 87, 0, 0, 84, // properties length
        17, 0, 0, 4, 210, // sessionExpiryInterval
        33, 1, 176, // receiveMaximum
        36, 1, // Maximum qos
        37, 1, // retainAvailable
        39, 0, 0, 0, 100, // maximumPacketSize
        18, 0, 4, 116, 101, 115, 116, // assignedClientIdentifier
        34, 1, 200, // topicAliasMaximum
        31, 0, 4, 116, 101, 115, 116, // reasonString
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // userProperties
        40, 1, // wildcardSubscriptionAvailable
        41, 1, // subscriptionIdentifiersAvailable
        42, 0, // sharedSubscriptionAvailable
        19, 4, 210, // serverKeepAlive
        26, 0, 4, 116, 101, 115, 116, // responseInformation
        28, 0, 4, 116, 101, 115, 116, // serverReference
        21, 0, 4, 116, 101, 115, 116, // authenticationMethod
        22, 0, 4, 1, 2, 3, 4, // authenticationData
    ],
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            session_expiry_interval: Some(1234),
            receive_maximum: NonZero::new(432),
            maximum_qos: Some(MaximumQoS::AtLeastOnce),
            retain_available: Some(true),
            maximum_packet_size: NonZero::new(100),
            assigned_client_identifier: Some(MQTTString::new("test").unwrap()),
            topic_alias_maximum: Some(456),
            reason_string: Some(MQTTString::new("test").unwrap()),
            user_properties: vec! [(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
            wildcard_subscription_available: Some(true),
            subscription_identifiers_available: Some(true),
            shared_subscription_available: Some(false),
            server_keep_alive: Some(1234),
            response_information: Some(MQTTString::new("test").unwrap()),
            server_reference: Some(MQTTString::new("test").unwrap()),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test").unwrap(),
                data: [1, 2, 3, 4][..].into(),
            }),
        },
    })
)]
// Note: This test was modified as the maximum_qos value was not spec compliant
#[case::connack_mqtt_5_with_properties_and_doubled_user_properties(
    vec! [
        32, 100, 0, 0, 97, // properties length
        17, 0, 0, 4, 210, // sessionExpiryInterval
        33, 1, 176, // receiveMaximum
        36, 1, // Maximum qos
        37, 1, // retainAvailable
        39, 0, 0, 0, 100, // maximumPacketSize
        18, 0, 4, 116, 101, 115, 116, // assignedClientIdentifier
        34, 1, 200, // topicAliasMaximum
        31, 0, 4, 116, 101, 115, 116, // reasonString
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, 38, 0, 4, 116, 101, 115, 116, 0, 4,
        116, 101, 115, 116, // userProperties
        40, 1, // wildcardSubscriptionAvailable
        41, 1, // subscriptionIdentifiersAvailable
        42, 0, // sharedSubscriptionAvailable
        19, 4, 210, // serverKeepAlive
        26, 0, 4, 116, 101, 115, 116, // responseInformation
        28, 0, 4, 116, 101, 115, 116, // serverReference
        21, 0, 4, 116, 101, 115, 116, // authenticationMethod
        22, 0, 4, 1, 2, 3, 4, // authenticationData
    ],
    ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        },
        properties: ConnAckProperties {
            session_expiry_interval: Some(1234),
            receive_maximum: NonZero::new(432),
            maximum_qos: Some(MaximumQoS::AtLeastOnce),
            retain_available: Some(true),
            maximum_packet_size: NonZero::new(100),
            assigned_client_identifier: Some(MQTTString::new("test").unwrap()),
            topic_alias_maximum: Some(456),
            reason_string: Some(MQTTString::new("test").unwrap()),
            user_properties: vec! [
                (
                    MQTTString::new("test").unwrap(),
                    MQTTString::new("test").unwrap(),
                ),
                (
                    MQTTString::new("test").unwrap(),
                    MQTTString::new("test").unwrap(),
                ),
            ],
            wildcard_subscription_available: Some(true),
            subscription_identifiers_available: Some(true),
            shared_subscription_available: Some(false),
            server_keep_alive: Some(1234),
            response_information: Some(MQTTString::new("test").unwrap()),
            server_reference: Some(MQTTString::new("test").unwrap()),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test").unwrap(),
                data: [1, 2, 3, 4][..].into(),
            }),
        },
    })
)]
#[case::publish_mqtt_5_properties(
    vec! [
        61, 86, // Header
        0, 4, // Topic length
        116, 101, 115, 116, // Topic (test)
        0, 10, // Message ID
        73, // properties length
        1, 1, // payloadFormatIndicator
        2, 0, 0, 16, 225, // message expiry interval
        35, 0, 100, // topicAlias
        8, 0, 5, 116, 111, 112, 105, 99, // response topic
        9, 0, 4, 1, 2, 3, 4, // correlationData
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // userProperties
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // userProperties
        38, 0, 4, 116, 101, 115, 116, 0, 4, 116, 101, 115, 116, // userProperties
        11, 120, // subscriptionIdentifier
        3, 0, 4, 116, 101, 115, 116, // content type
        116, 101, 115, 116, // Payload (test)
    ],
    ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: NonZero::new(10).unwrap(),
            qos: GuaranteedQoS::ExactlyOnce,
            dup: true,
        },
        retain: true,
        topic: PublishTopic::new("test").unwrap(),
        payload: [116, 101, 115, 116][..].into(),
        properties: PublishProperties {
            payload_format_indicator: Some(FormatIndicator::Utf8),
            message_expiry_interval: Some(4321),
            topic_alias: NonZero::new(100),
            response_topic: PublishTopic::new("topic"),
            correlation_data: Some([1, 2, 3, 4][..].into()),
            user_properties: vec! [
                (
                    MQTTString::new("test").unwrap(),
                    MQTTString::new("test").unwrap(),
                ),
                (
                    MQTTString::new("test").unwrap(),
                    MQTTString::new("test").unwrap(),
                ),
                (
                    MQTTString::new("test").unwrap(),
                    MQTTString::new("test").unwrap(),
                ),
            ],
            subscription_identifier: NonZero::new(120),
            content_type: MQTTString::new("test"),
        },
    })
)]
fn assert_that_different_packets_can_be_decoded_and_encoded(
    #[case] encoded_packet_buffer: Vec<u8>,
    #[case] expected_packet: ControlPacket,
) {
    let settings = Settings::default();
    let decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&*encoded_packet_buffer)
        .unwrap();
    assert_eq!(
        decoded_packet, expected_packet,
        "The decoded packet should be equal to the expected packet"
    );

    let mut encoded_packet_buffer = Vec::with_capacity(expected_packet.encoded_size().unwrap());
    expected_packet.encode(&mut encoded_packet_buffer).unwrap();
    let settings = Settings::default();
    let re_decoded_packet = ControlPacket::parse::<_, ContextError, ContextError>(&settings)
        .parse(&encoded_packet_buffer[..])
        .unwrap();
    assert_eq!(re_decoded_packet, expected_packet);
}
