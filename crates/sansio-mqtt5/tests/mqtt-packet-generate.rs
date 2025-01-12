//! Uses the `mqtt-packet` npm package to generate MQTT packets for testing the parser.
//!
//! Requires `deno` to be installed and available in the PATH. (optionally set the `DENO` environment variable)

use std::error::Error;
use std::fmt::Display;
use std::io::Write;
use std::num::NonZero;
use std::process::Command;
use std::process::Stdio;

use sansio_mqtt5::parser::Settings;
use sansio_mqtt5::types::*;
use vec1::vec1;
use winnow::error::ContextError;
use winnow::Parser as _;

#[test]
fn assert_parser_handles_example_connect_packet() {
    let name = "Example connect packet";
    let packet = r#"{
                cmd: 'connect',
                protocolId: 'MQTT',
                protocolVersion: 5, 
                clean: true,
                clientId: 'my-device',
                keepalive: 0, 
                username: 'matteo',
                password: 'collina',
                qos: 0,
                will: {
                    topic: 'mydevice/status',
                    payload: 'dead',
                    properties: {
                        willDelayInterval: 1234,
                        payloadFormatIndicator: false,
                        messageExpiryInterval: 4321,
                        contentType: 'test',
                        responseTopic: 'topic',
                        userProperties: {
                            'test': 'test'
                        }
                    }
                }
            }"#;
    let expected = ControlPacket::Connect(Connect {
        protocol_name: MQTTString::new("MQTT").unwrap(),
        protocol_version: 5,
        clean_start: true,
        client_identifier: MQTTString::new("my-device").unwrap(),
        properties: ConnectProperties::default(),
        will: Some(Will {
            properties: WillProperties {
                will_delay_interval: Some(1234),
                payload_format_indicator: Some(FormatIndicator::Unspecified),
                message_expiry_interval: Some(4321),
                content_type: MQTTString::new("test"),
                response_topic: PublishTopic::new("topic"),
                user_properties: vec![(
                    MQTTString::new("test").unwrap(),
                    MQTTString::new("test").unwrap(),
                )],
                ..Default::default()
            },
            topic: PublishTopic::new("mydevice/status").unwrap(),
            payload: Into::into(b"dead" as &[u8]),
            qos: Qos::AtMostOnce,
            retain: false,
        }),
        user_name: MQTTString::new("matteo"),
        password: Some(Into::into(b"collina" as &[u8])),
        keep_alive: 0,
    });

    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_lean_connect_packet() {
    let name = "Lean connect packet";
    let packet = r#"{
                cmd: 'connect',
                protocolId: 'MQTT',
                protocolVersion: 5, 
                clean: true,
                clientId: '',
                keepalive: 0, 
            }"#;
    let expected = ControlPacket::Connect(Connect {
        protocol_name: MQTTString::new("MQTT").unwrap(),
        protocol_version: 5,
        clean_start: true,
        client_identifier: MQTTString::new("").unwrap(),
        properties: ConnectProperties::default(),
        will: None,
        user_name: None,
        password: None,
        keep_alive: 0,
    });

    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_connectack_packets() {
    let name = "Example connectack packet";
    let packet = r#"{
  cmd: 'connack',
  returnCode: 0, // Or whatever else you see fit MQTT < 5.0
  sessionPresent: false, // Can also be true.
  reasonCode: 0, // reason code MQTT 5.0
  properties: { // MQTT 5.0 properties
      sessionExpiryInterval: 1234,
      receiveMaximum: 432,
      maximumQoS: 1,
      retainAvailable: true,
      maximumPacketSize: 100,
      assignedClientIdentifier: 'test',
      topicAliasMaximum: 456,
      reasonString: 'test',
      userProperties: {
        'test': 'test'
      },
      wildcardSubscriptionAvailable: true,
      subscriptionIdentifiersAvailable: true,
      sharedSubscriptionAvailable: false,
      serverKeepAlive: 1234,
      responseInformation: 'test',
      serverReference: 'test',
      authenticationMethod: 'test',
      authenticationData: Buffer.from([1, 2, 3, 4])
  }
}"#;
    let expected = ControlPacket::ConnAck(ConnAck {
        kind: ConnAckKind::Other {
            reason_code: ReasonCode::Success,
        },
        properties: ConnAckProperties {
            session_expiry_interval: Some(1234),
            receive_maximum: NonZero::new(432),
            maximum_qos: Some(MaximumQoS::AtLeastOnce),
            retain_available: Some(true),
            maximum_packet_size: NonZero::new(100),
            assigned_client_identifier: MQTTString::new("test"),
            topic_alias_maximum: Some(456),
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
            wildcard_subscription_available: Some(true),
            subscription_identifiers_available: Some(true),
            shared_subscription_available: Some(false),
            server_keep_alive: Some(1234),
            response_information: MQTTString::new("test"),
            server_reference: MQTTString::new("test"),
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test").unwrap(),
                data: Cow::from([1u8, 2, 3, 4].as_ref()),
            }),
        },
    });

    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_subscribe_packets() {
    let name = "Example subscribe packet";
    let packet = r#"{
  cmd: 'subscribe',
  messageId: 42,
  properties: { // MQTT 5.0 properties
    subscriptionIdentifier: 145,
    userProperties: {
      test: 'test'
    }
  },
  subscriptions: [{
    topic: 'test',
    qos: 0,
    nl: false, // no Local MQTT 5.0 flag
    rap: true, // Retain as Published MQTT 5.0 flag
    rh: 1 // Retain Handling MQTT 5.0
  }]
}"#;
    let expected = ControlPacket::Subscribe(Subscribe {
        packet_id: NonZero::new(42).unwrap(),
        properties: SubscribeProperties {
            subscription_identifier: NonZero::new(145),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
        subscriptions: vec1![Subscription {
            topic_filter: MQTTString::new("test").unwrap(),
            qos: Qos::AtMostOnce,
            no_local: false,
            retain_as_published: true,
            retain_handling: RetainHandling::SendRetainedIfSubscriptionDoesNotExist,
        }],
    });

    assert_parser_handles_impl(name, packet, expected);
}
#[test]
fn assert_parser_handles_example_suback_packets() {
    let name = "Example suback packet";
    let packet = r#"{
  cmd: 'suback',
  messageId: 42,
  properties: { // MQTT 5.0 properties
    reasonString: 'test',
    userProperties: {
      'test': 'test'
    }
  },
  granted: [0, 1, 2, 128]
}"#;
    let expected = ControlPacket::SubAck(SubAck {
        packet_id: NonZero::new(42).unwrap(),
        properties: SubAckProperties {
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
        reason_codes: vec![
            ReasonCode::GrantedQoS0,
            ReasonCode::GrantedQoS1,
            ReasonCode::GrantedQoS2,
            ReasonCode::UnspecifiedError,
        ],
    });

    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_unsubscribe_packets() {
    let name = "Example unsubscribe packet";
    let packet = r#"{
  cmd: 'unsubscribe',
  messageId: 42,
  properties: { // MQTT 5.0 properties
    userProperties: {
      'test': 'test'
    }
  },
  unsubscriptions: [
    'test',
    'a/topic'
  ]
}"#;
    let expected = ControlPacket::Unsubscribe(Unsubscribe {
        packet_id: NonZero::new(42).unwrap(),
        properties: UnsubscribeProperties {
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
        topics: vec1![
            MQTTString::new("test").unwrap(),
            MQTTString::new("a/topic").unwrap(),
        ],
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_unsuback_packets() {
    let name = "Example unsuback packet";
    let packet = r#"{
  cmd: 'unsuback',
  messageId: 42,
  properties: { // MQTT 5.0 properties
    reasonString: 'test',
    userProperties: {
      'test': 'test'
    }
  },
  granted: [0, 17]
}"#;
    let expected = ControlPacket::UnsubAck(UnsubAck {
        packet_id: NonZero::new(42).unwrap(),
        properties: UnsubAckProperties {
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
        reason_codes: vec![ReasonCode::Success, ReasonCode::NoSubscriptionExisted],
    });
    assert_parser_handles_impl(name, packet, expected);
}
#[test]
fn assert_parser_handles_example_publish_packets() {
    let name = "Example publish packet";
    let packet = r#"{
  cmd: 'publish',
  messageId: 42,
  qos: 2,
  dup: false,
  topic: 'test',
  payload: Buffer.from('test'),
  retain: false,
  properties: { // optional properties MQTT 5.0
      payloadFormatIndicator: true,
      messageExpiryInterval: 4321,
      topicAlias: 100,
      responseTopic: 'topic',
      correlationData: Buffer.from([1, 2, 3, 4]),
      userProperties: {
        'test': 'test'
      },
      subscriptionIdentifier: 120, // can be an Array in message from broker, if message included in few another subscriptions
      contentType: 'test'
   }
}"#;
    let expected = ControlPacket::Publish(Publish {
        kind: PublishKind::Repetible {
            packet_id: NonZero::new(42).unwrap(),
            qos: GuaranteedQoS::ExactlyOnce,
            dup: false,
        },
        retain: false,
        topic: PublishTopic::new("test").unwrap(),
        payload: Into::into(b"test" as &[u8]),
        properties: PublishProperties {
            payload_format_indicator: Some(FormatIndicator::Utf8),
            message_expiry_interval: Some(4321),
            topic_alias: NonZero::new(100),
            response_topic: PublishTopic::new("topic"),
            correlation_data: Some(Into::into(&[1u8, 2, 3, 4] as &[u8])),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
            subscription_identifier: NonZero::new(120),
            content_type: MQTTString::new("test"),
        },
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_puback_packets() {
    let name = "Example puback packet";
    let packet = r#"{
  cmd: 'puback',
  messageId: 42,
  reasonCode: 16, // only for MQTT 5.0
  properties: { // MQTT 5.0 properties
      reasonString: 'test',
      userProperties: {
        'test': 'test'
      }
  }
}
"#;
    let expected = ControlPacket::PubAck(PubAck {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: ReasonCode::NoMatchingSubscribers,
        properties: PubAckProperties {
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_lean_puback_packets() {
    let name = "Lean puback packet";
    let packet = r#"{
  cmd: 'puback',
  messageId: 42
}"#;
    let expected = ControlPacket::PubAck(PubAck {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: ReasonCode::Success,
        properties: Default::default(),
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_pubrec_packets() {
    let name = "Example pubrec packet";
    let packet = r#"{
  cmd: 'pubrec',
  messageId: 42,
  reasonCode: 16, // only for MQTT 5.0
  properties: { // properties MQTT 5.0
    reasonString: 'test',
    userProperties: {
      'test': 'test'
    }
  }
}
"#;
    let expected = ControlPacket::PubRec(PubRec {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: ReasonCode::NoMatchingSubscribers,
        properties: PubRecProperties {
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_lean_pubrec_packets() {
    let name = "Lean pubrec packet";
    let packet = r#"{
  cmd: 'pubrec',
  messageId: 42
}"#;
    let expected = ControlPacket::PubRec(PubRec {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: ReasonCode::Success,
        properties: Default::default(),
    });
    assert_parser_handles_impl(name, packet, expected);
}
#[test]
fn assert_parser_handles_example_pubrel_packets() {
    let name = "Example pubrel packet";
    let packet = r#"{
  cmd: 'pubrel',
  messageId: 42,
  reasonCode: 0, // only for MQTT 5.0
  properties: { // properties MQTT 5.0
     reasonString: 'test',
     userProperties: {
       'test': 'test'
     }
  }
}
"#;
    let expected = ControlPacket::PubRel(PubRel {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: ReasonCode::Success,
        properties: PubRelProperties {
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_lean_pubrel_packets() {
    let name = "Lean pubrel packet";
    let packet = r#"{
  cmd: 'pubrel',
  messageId: 42
}"#;
    let expected = ControlPacket::PubRel(PubRel {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: ReasonCode::Success,
        properties: Default::default(),
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_pubcomp_packets() {
    let name = "Example pubcomp packet";
    let packet = r#"{
  cmd: 'pubcomp',
  messageId: 42,
  reasonCode: 146, // only for MQTT 5.0
  properties: { // properties MQTT 5.0
    reasonString: 'test',
    userProperties: {
       'test': 'test'
    }
  }
}
"#;
    let expected = ControlPacket::PubComp(PubComp {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: ReasonCode::PacketIdentifierNotFound,
        properties: PubCompProperties {
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_lean_pubcomp_packets() {
    let name = "Lean pubcomp packet";
    let packet = r#"{
  cmd: 'pubcomp',
  messageId: 42
}"#;
    let expected = ControlPacket::PubComp(PubComp {
        packet_id: NonZero::new(42).unwrap(),
        reason_code: ReasonCode::Success,
        properties: Default::default(),
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_pingreq_packets() {
    let name = "Example pigreq packet";
    let packet = r#"{
  cmd: 'pingreq'
}"#;
    let expected = ControlPacket::PingReq(PingReq {});
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_example_pingresp_packets() {
    let name = "Example pingresp packet";
    let packet = r#"{
  cmd: 'pingresp'
}"#;
    let expected = ControlPacket::PingResp(PingResp {});
    assert_parser_handles_impl(name, packet, expected);
}
#[test]
fn assert_parser_handles_example_disconnect_packets() {
    let name = "Example disconnect packet";
    let packet = r#"{
  cmd: 'disconnect',
  reasonCode: 0, // MQTT 5.0 code
  properties: { // properties MQTT 5.0
     sessionExpiryInterval: 145,
     reasonString: 'test',
     userProperties: {
       'test': 'test'
     },
     serverReference: 'test'
  }
}"#;
    let expected = ControlPacket::Disconnect(Disconnect {
        reason_code: ReasonCode::NormalDisconnection,
        properties: DisconnectProperties {
            session_expiry_interval: Some(145),
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
            server_reference: MQTTString::new("test"),
        },
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_lean_disconnect_packets() {
    let name = "Lean disconnect packet";
    let packet = r#"{
  cmd: 'disconnect',
}"#;
    let expected = ControlPacket::Disconnect(Disconnect {
        reason_code: ReasonCode::NormalDisconnection,
        properties: Default::default(),
    });
    assert_parser_handles_impl(name, packet, expected);
}
#[test]
fn assert_parser_handles_example_auth_packets() {
    let name = "Example auth packet";
    let packet = r#"{
  cmd: 'auth',
  reasonCode: 0, // MQTT 5.0 code
  properties: { // properties MQTT 5.0
     authenticationMethod: 'test',
     authenticationData: Buffer.from([0, 1, 2, 3]),
     reasonString: 'test',
     userProperties: {
       'test': 'test'
     }
  }
}"#;
    let expected = ControlPacket::Auth(Auth {
        reason_code: ReasonCode::Success,
        properties: AuthProperties {
            authentication: Some(AuthenticationKind::WithData {
                method: MQTTString::new("test").unwrap(),
                data: Cow::from([0u8, 1, 2, 3].as_ref()),
            }),
            reason_string: MQTTString::new("test"),
            user_properties: vec![(
                MQTTString::new("test").unwrap(),
                MQTTString::new("test").unwrap(),
            )],
        },
    });
    assert_parser_handles_impl(name, packet, expected);
}

#[test]
fn assert_parser_handles_lean_auth_packets() {
    let name = "Lean auth packet";
    let packet = r#"{
  cmd: 'auth',
}"#;
    let expected = ControlPacket::Auth(Auth {
        reason_code: ReasonCode::Success,
        properties: Default::default(),
    });
    assert_parser_handles_impl(name, packet, expected);
}

///////////////////////////

fn assert_parser_handles_impl(
    name: impl Display,
    packet: impl Display,
    expected: ControlPacket<'static>,
) {
    let packet = encode_mqtt5_packet(packet)
        .unwrap_or_else(|_| panic!("Failed to generate mqtt-packet: {name}"));

    let parser_settings = Settings::default();
    let got = ControlPacket::parse::<_, ContextError, ContextError>(&parser_settings)
        .parse(&*packet)
        .unwrap_or_else(|e| {
            panic!(
                "Failed to parse generated mqtt-packet: '{name}' at offset {offset}\n{context:?}\n{e}",
                offset = e.offset(),
                context = e.inner().context().collect::<Vec<_>>()
            )
        });
    assert_eq!(
        got, expected,
        "Parsed packet does not match expected: {name}"
    );
}

fn encode_mqtt5_packet(packet: impl Display) -> Result<Vec<u8>, Box<dyn Error>> {
    let program = format!(
        include_str!("mqtt-packet-generate.js.tpl"),
        packet = packet,
        options = "{protocolVersion: 5}"
    );

    let deno = std::env::var("DENO").unwrap_or("deno".into());
    let mut command = Command::new(deno);
    command
        .arg("run")
        // Required for Node compatibility
        .arg("--allow-env")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = command.spawn()?;
    child
        .stdin
        .take()
        .expect("STDIN should be piped")
        .write_all(program.as_bytes())?;
    let output = child.wait_with_output()?;
    assert!(
        output.status.success(),
        "Failed to execute deno: exit_code={:?}",
        output.status
    );
    Ok(output.stdout)
}
