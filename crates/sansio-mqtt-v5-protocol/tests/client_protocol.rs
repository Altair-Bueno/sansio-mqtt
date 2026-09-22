use bytes::Bytes;
use bytestring::ByteString;
use core::num::NonZero;
use core::time::Duration;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_protocol::Authentication;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::DropReason;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::Qos as ProtoQos;
use sansio_mqtt_protocol::ReasonCode;
use sansio_mqtt_protocol::RejectReason;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription as ProtoSubscription;
use sansio_mqtt_protocol::UnsubscribeOptions;
use sansio_mqtt_v5_protocol::Client;
use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_types::Auth;
use sansio_mqtt_v5_types::AuthProperties;
use sansio_mqtt_v5_types::AuthReasonCode;
use sansio_mqtt_v5_types::AuthenticationKind;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnAckProperties;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::Disconnect;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::GuaranteedQoS;
use sansio_mqtt_v5_types::MaximumQoS;
use sansio_mqtt_v5_types::ParserSettings;
use sansio_mqtt_v5_types::Payload;
use sansio_mqtt_v5_types::PubAck;
use sansio_mqtt_v5_types::PubAckReasonCode;
use sansio_mqtt_v5_types::PubComp;
use sansio_mqtt_v5_types::PubCompReasonCode;
use sansio_mqtt_v5_types::PubRec;
use sansio_mqtt_v5_types::PubRecReasonCode;
use sansio_mqtt_v5_types::PubRel;
use sansio_mqtt_v5_types::PubRelReasonCode;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::PublishProperties;
use sansio_mqtt_v5_types::SubAck;
use sansio_mqtt_v5_types::SubAckReasonCode;
use sansio_mqtt_v5_types::Topic;
use sansio_mqtt_v5_types::UnsubAck;
use sansio_mqtt_v5_types::UnsubAckReasonCode;
use sansio_mqtt_v5_types::Utf8String;
use winnow::Parser;
use winnow::error::ContextError;

// ── Wire-level helpers (build/decode raw packets with `sansio-mqtt-v5-types`)
// ─

fn encode_packet(packet: &ControlPacket) -> Bytes {
    let mut out = Vec::new();
    packet.encode(&mut out).expect("packet should encode");
    Bytes::from(out)
}

fn wire_topic(name: &str) -> Topic {
    Topic::try_from(Utf8String::try_from(name).expect("valid utf8")).expect("valid topic")
}

fn read(client: &mut Client<Duration>, packet: &ControlPacket) -> Result<(), Error> {
    client.handle_read(IncomingData {
        bytes: &encode_packet(packet),
        received_at: Duration::ZERO,
    })
}

fn read_at(
    client: &mut Client<Duration>,
    packet: &ControlPacket,
    received_at: Duration,
) -> Result<(), Error> {
    client.handle_read(IncomingData {
        bytes: &encode_packet(packet),
        received_at,
    })
}

fn success_connack() -> ControlPacket {
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .build(),
    )
}

fn connack_with_properties(properties: ConnAckProperties) -> ControlPacket {
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::Success,
            })
            .properties(properties)
            .build(),
    )
}

fn resume_connack() -> ControlPacket {
    ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::ResumePreviousSession)
            .build(),
    )
}

// ── App-facing helpers (build values with `sansio-mqtt-protocol`) ───────────

/// Minimal valid `ConnectOptions`: only `client_id` is mandatory.
fn connect_options() -> ConnectOptions {
    ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .build()
}

fn message(topic: &str, qos: ProtoQos, payload: &[u8]) -> Message {
    Message::builder()
        .topic(ByteString::from(topic))
        .payload(Bytes::copy_from_slice(payload))
        .qos(qos)
        .build()
}

fn app_subscription(filter: &str) -> ProtoSubscription {
    ProtoSubscription::builder()
        .filter(ByteString::from(filter))
        .build()
}

fn subscribe_one(sub: ProtoSubscription) -> Command {
    Command::Subscribe(SubscribeOptions::builder().subscriptions(vec![sub]).build())
}

fn unsubscribe_one(filter: &str) -> Command {
    Command::Unsubscribe(
        UnsubscribeOptions::builder()
            .filters(vec![ByteString::from(filter)])
            .build(),
    )
}

/// `ClientSettings` field overrides, mirroring `ClientSettings::default()`
/// except for the three mandatory counters (always 32, as in the library
/// default). `ClientSettings` is `#[non_exhaustive]`, so struct-update syntax
/// is unavailable to this integration test crate; this struct plus [`settings`]
/// stands in for it.
struct SettingsOverrides {
    receive_maximum: Option<NonZero<u16>>,
    maximum_packet_size: Option<NonZero<u32>>,
    topic_alias_maximum: Option<u16>,
    request_response_information: Option<bool>,
    request_problem_information: Option<bool>,
    keep_alive: Option<NonZero<u16>>,
    max_outgoing_qos: Option<ProtoQos>,
    allow_retain: bool,
    allow_wildcard_subscriptions: bool,
    allow_shared_subscriptions: bool,
    allow_subscription_identifiers: bool,
}

impl Default for SettingsOverrides {
    fn default() -> Self {
        Self {
            receive_maximum: None,
            maximum_packet_size: None,
            topic_alias_maximum: None,
            request_response_information: None,
            request_problem_information: None,
            keep_alive: None,
            max_outgoing_qos: None,
            allow_retain: true,
            allow_wildcard_subscriptions: true,
            allow_shared_subscriptions: true,
            allow_subscription_identifiers: true,
        }
    }
}

fn settings(overrides: SettingsOverrides) -> ClientSettings {
    ClientSettings::builder()
        .maybe_receive_maximum(overrides.receive_maximum)
        .maybe_maximum_packet_size(overrides.maximum_packet_size)
        .maybe_topic_alias_maximum(overrides.topic_alias_maximum)
        .maybe_request_response_information(overrides.request_response_information)
        .maybe_request_problem_information(overrides.request_problem_information)
        .maybe_keep_alive(overrides.keep_alive)
        .maybe_max_outgoing_qos(overrides.max_outgoing_qos)
        .allow_retain(overrides.allow_retain)
        .allow_wildcard_subscriptions(overrides.allow_wildcard_subscriptions)
        .allow_shared_subscriptions(overrides.allow_shared_subscriptions)
        .allow_subscription_identifiers(overrides.allow_subscription_identifiers)
        .max_user_properties(32)
        .max_subscription_identifiers(32)
        .max_subscriptions(32)
        .build()
}

/// Sends `Command::Connect(options)`, asserts `Ok(())` and
/// `DriverAction::OpenSocket`, then delivers `DriverEvent::SocketConnected` and
/// returns the encoded CONNECT bytes.
fn open_connecting(client: &mut Client<Duration>, options: ConnectOptions) -> Bytes {
    assert_eq!(client.handle_write(Command::Connect(options)), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    client.poll_write().expect("connect frame expected")
}

/// [`open_connecting`] followed by a successful CONNACK; asserts
/// `Event::Connected`.
fn connect_client(client: &mut Client<Duration>, options: ConnectOptions) -> Bytes {
    let connect_bytes = open_connecting(client, options);
    assert_eq!(read(client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
    connect_bytes
}

fn connect_default(client: &mut Client<Duration>) -> Bytes {
    connect_client(client, connect_options())
}

#[test]
fn message_builder_default_qos_is_at_most_once() {
    let message = Message::builder()
        .topic(ByteString::from_static("t"))
        .payload(Bytes::new())
        .build();
    let _: ProtoQos = message.qos;

    assert_eq!(message.qos, ProtoQos::AtMostOnce);
}

#[test]
fn event_exposes_qos_delivery_variants_with_token() {
    let no_ack = Event::Message(message("t", ProtoQos::AtMostOnce, b"x"));
    assert!(matches!(no_ack, Event::Message(_)));
    assert!(matches!(Event::Connected, Event::Connected));

    let acknowledged = Event::PublishAcknowledged {
        token: 7,
        reason: ReasonCode::Success,
    };
    let completed = Event::PublishCompleted {
        token: 7,
        reason: ReasonCode::Success,
    };
    let dropped = Event::PublishDropped {
        token: 7,
        reason: DropReason::SessionNotResumed,
    };
    let dropped_by_broker = Event::PublishDropped {
        token: 7,
        reason: DropReason::BrokerRejected(ReasonCode::NotAuthorized),
    };

    match acknowledged {
        Event::PublishAcknowledged { token, reason } => {
            assert_eq!(token, 7);
            assert_eq!(reason, ReasonCode::Success);
        }
        other => panic!("expected PublishAcknowledged, got {other:?}"),
    }
    match completed {
        Event::PublishCompleted { token, reason } => {
            assert_eq!(token, 7);
            assert_eq!(reason, ReasonCode::Success);
        }
        other => panic!("expected PublishCompleted, got {other:?}"),
    }
    match dropped {
        Event::PublishDropped {
            token,
            reason: DropReason::SessionNotResumed,
        } => assert_eq!(token, 7),
        other => panic!("expected PublishDropped(SessionNotResumed), got {other:?}"),
    }
    match dropped_by_broker {
        Event::PublishDropped {
            token,
            reason: DropReason::BrokerRejected(ReasonCode::NotAuthorized),
        } => assert_eq!(token, 7),
        other => panic!("expected PublishDropped(BrokerRejected), got {other:?}"),
    }
}

#[test]
fn driver_events_are_pattern_matchable_without_equality() {
    let incoming: DriverEvent = DriverEvent::SocketConnected;
    let outgoing = DriverAction::OpenSocket;

    assert!(matches!(incoming, DriverEvent::SocketConnected));
    assert!(matches!(outgoing, DriverAction::OpenSocket));
}

#[test]
fn client_new_uses_default_state_and_blank_scratchpad() {
    let _client = Client::<Duration>::new(ClientSettings::default());
}

#[test]
fn error_variants_are_instantiable_and_matchable() {
    let classify = |error: Error| -> &'static str {
        match error {
            Error::MalformedPacket => "malformed packet",
            Error::ProtocolError => "protocol error",
            Error::InvalidStateTransition => "invalid state transition",
            Error::PacketTooLarge => "packet too large",
            Error::ReceiveMaximumExceeded => "receive maximum exceeded",
            Error::EncodeFailure => "encode failure",
            Error::ConnectTimeout => "connect timeout",
            _ => "other",
        }
    };

    assert_eq!(classify(Error::MalformedPacket), "malformed packet");
    assert_eq!(classify(Error::ProtocolError), "protocol error");
    assert_eq!(
        classify(Error::InvalidStateTransition),
        "invalid state transition"
    );
    assert_eq!(classify(Error::PacketTooLarge), "packet too large");
    assert_eq!(
        classify(Error::ReceiveMaximumExceeded),
        "receive maximum exceeded"
    );
    assert_eq!(classify(Error::EncodeFailure), "encode failure");
    assert_eq!(classify(Error::ConnectTimeout), "connect timeout");
}

#[test]
fn client_settings_default_includes_permissive_negotiation_policy() {
    let settings = ClientSettings::default();

    assert!(settings.receive_maximum.is_none());
    assert!(settings.maximum_packet_size.is_none());
    assert!(settings.topic_alias_maximum.is_none());
    assert!(settings.max_outgoing_qos.is_none());
    assert!(settings.allow_retain);
    assert!(settings.allow_wildcard_subscriptions);
    assert!(settings.allow_shared_subscriptions);
    assert!(settings.allow_subscription_identifiers);
    assert!(settings.request_response_information.is_none());
    assert!(settings.request_problem_information.is_none());
    assert!(settings.keep_alive.is_none());
}

/// [Behaviour change] `DriverEvent::SocketConnected` before any
/// `Command::Connect` is now rejected instead of sending an empty CONNECT.
#[test]
fn socket_connected_before_connect_is_invalid_state_transition() {
    let mut client = Client::<Duration>::default();

    let result = client.handle_event(DriverEvent::SocketConnected);

    assert_eq!(result, Err(Error::InvalidStateTransition));
    assert_eq!(client.poll_write(), None);
}

/// `ClientSettings` is the single source of the CONNECT-advertised limits;
/// there is no more per-`Command::Connect` override for any of them.
#[test]
fn connect_settings_populate_connect_properties() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        keep_alive: NonZero::new(30),
        request_response_information: Some(true),
        request_problem_information: Some(false),
        receive_maximum: NonZero::new(7),
        maximum_packet_size: NonZero::new(1024),
        topic_alias_maximum: Some(3),
        ..Default::default()
    }));

    let connect_bytes = connect_default(&mut client);

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

/// A small `maximum_packet_size` in `ClientSettings` bounds the parser even
/// before CONNACK arrives, since it is now the single source of the
/// advertised (and locally-enforced) Maximum Packet Size.
#[test]
fn parser_uses_effective_client_limits_after_connect_policy_applied() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        maximum_packet_size: NonZero::new(2),
        ..Default::default()
    }));
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        read(&mut client, &success_connack()),
        Err(Error::MalformedPacket)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x81, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn socket_closed_emits_disconnected_event() {
    let mut client = Client::<Duration>::default();

    let result = client.handle_event(DriverEvent::SocketClosed);

    assert_eq!(result, Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
}

#[test]
fn socket_connected_in_connecting_state_returns_invalid_transition() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    let result = client.handle_event(DriverEvent::SocketConnected);

    assert_eq!(result, Err(Error::InvalidStateTransition));
    assert_eq!(client.poll_write(), None);
}

#[test]
fn fragmented_packet_is_buffered_until_complete() {
    let mut client = Client::<Duration>::default();

    let first_fragment = client.handle_read(IncomingData {
        bytes: (&[0xD0]),
        received_at: Duration::ZERO,
    });

    assert_eq!(first_fragment, Ok(()));
    assert_eq!(client.poll_write(), None);
    assert!(client.poll_event().is_none());

    let second_fragment = client.handle_read(IncomingData {
        bytes: (&[0x00]),
        received_at: Duration::ZERO,
    });

    assert_eq!(second_fragment, Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn malformed_packet_triggers_close_action() {
    let mut client = Client::<Duration>::default();

    let result = client.handle_read(IncomingData {
        bytes: (&[0xD0, 0x01, 0x00]),
        received_at: Duration::ZERO,
    });

    assert_eq!(result, Err(Error::MalformedPacket));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x81, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn protocol_error_emits_disconnect_bytes_before_close_action_polling() {
    let mut client = Client::<Duration>::default();

    let result = client.handle_read(IncomingData {
        bytes: (&[0xD0, 0x00]),
        received_at: Duration::ZERO,
    });

    assert_eq!(result, Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert_eq!(client.poll_write(), None);
    assert!(client.poll_event().is_none());
}

#[test]
fn connack_transitions_to_connected_and_emits_connected() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("test/topic", ProtoQos::AtMostOnce, b"qos0"),
        }),
        Ok(())
    );
    assert!(
        client.poll_write().is_some(),
        "PUBLISH should be queued once Connected"
    );
    assert!(client.poll_read().is_none());
}

#[test]
fn connack_rejected_reason_closes_without_connected_event() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    let connack = ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::Other {
                reason_code: ConnackReasonCode::NotAuthorized,
            })
            .build(),
    );

    assert_eq!(read(&mut client, &connack), Err(Error::ProtocolError));
    assert!(client.poll_read().is_none());
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn inbound_publish_qos0_is_forwarded_to_user_queue() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"27.5".as_slice()))
            .topic(wire_topic("sensors/temp"))
            .build(),
    );
    assert_eq!(read(&mut client, &publish), Ok(()));

    match client.poll_read() {
        Some(Event::Message(message)) => {
            assert_eq!(message.topic, ByteString::from_static("sensors/temp"));
            assert_eq!(message.payload, Bytes::from_static(b"27.5"));
            assert_eq!(message.payload_format, None);
            assert_eq!(message.message_expiry, None);
            assert_eq!(message.response_topic, None);
            assert_eq!(message.correlation_data, None);
            assert!(message.subscription_identifiers.is_empty());
            assert_eq!(message.content_type, None);
            assert!(message.user_properties.is_empty());
        }
        other => panic!("expected received message, got {other:?}"),
    }
}

#[test]
fn inbound_publish_multiple_subscription_identifiers_surface_to_user() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"hello".as_slice()))
            .topic(wire_topic("t/multi"))
            .properties(
                PublishProperties::builder()
                    .subscription_identifiers(vec![
                        NonZero::new(7).unwrap(),
                        NonZero::new(42).unwrap(),
                    ])
                    .build(),
            )
            .build(),
    );
    assert_eq!(read(&mut client, &publish), Ok(()));

    match client.poll_read() {
        Some(Event::Message(message)) => {
            assert_eq!(message.topic, ByteString::from_static("t/multi"));
            assert_eq!(message.payload, Bytes::from_static(b"hello"));
            assert_eq!(
                message.subscription_identifiers,
                vec![NonZero::new(7).unwrap(), NonZero::new(42).unwrap()]
            );
        }
        other => panic!("expected received message, got {other:?}"),
    }
}

#[test]
fn inbound_publish_registers_topic_alias_then_resolves_alias_only_publish() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        topic_alias_maximum: Some(10),
        ..Default::default()
    }));
    let _ = connect_default(&mut client);

    let alias = NonZero::new(1).expect("non-zero alias");

    let register_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"first".as_slice()))
            .topic(wire_topic("alias/topic"))
            .properties(PublishProperties::builder().topic_alias(alias).build())
            .build(),
    );
    assert_eq!(read(&mut client, &register_publish), Ok(()));
    match client.poll_read() {
        Some(Event::Message(message)) => {
            assert_eq!(message.topic, ByteString::from_static("alias/topic"));
            assert_eq!(message.payload, Bytes::from_static(b"first"));
        }
        other => panic!("expected received message, got {other:?}"),
    }

    let alias_only_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"second".as_slice()))
            .topic(wire_topic(""))
            .properties(PublishProperties::builder().topic_alias(alias).build())
            .build(),
    );
    assert_eq!(read(&mut client, &alias_only_publish), Ok(()));

    match client.poll_read() {
        Some(Event::Message(message)) => {
            assert_eq!(message.topic, ByteString::from_static("alias/topic"));
            assert_eq!(message.payload, Bytes::from_static(b"second"));
        }
        other => panic!("expected received message, got {other:?}"),
    }
}

#[test]
fn inbound_publish_alias_only_unknown_alias_is_protocol_error() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        topic_alias_maximum: Some(10),
        ..Default::default()
    }));
    let _ = connect_default(&mut client);

    let unknown_alias_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"unknown".as_slice()))
            .topic(wire_topic(""))
            .properties(
                PublishProperties::builder()
                    .topic_alias(NonZero::new(1).expect("non-zero alias"))
                    .build(),
            )
            .build(),
    );

    assert_eq!(
        read(&mut client, &unknown_alias_publish),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(client.poll_read().is_none());
}

#[test]
fn inbound_publish_empty_topic_without_alias_is_protocol_error() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let invalid_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"invalid".as_slice()))
            .topic(wire_topic(""))
            .build(),
    );

    assert_eq!(
        read(&mut client, &invalid_publish),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(client.poll_read().is_none());
}

/// [Behaviour change] `topic_alias_maximum` is settings-only: there is no
/// per-connect override any more, so `Some(0)` (or the default `None`) is the
/// only way to disable inbound Topic Aliases.
#[test]
fn inbound_publish_alias_rejected_when_topic_alias_maximum_is_zero() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let publish_with_alias = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"with-alias".as_slice()))
            .topic(wire_topic("alias/topic"))
            .properties(
                PublishProperties::builder()
                    .topic_alias(NonZero::new(1).expect("non-zero alias"))
                    .build(),
            )
            .build(),
    );

    assert_eq!(
        read(&mut client, &publish_with_alias),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(client.poll_read().is_none());
}

#[test]
fn inbound_publish_alias_exceeding_topic_alias_maximum_is_protocol_error() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        topic_alias_maximum: Some(1),
        ..Default::default()
    }));
    let _ = connect_default(&mut client);

    let alias_too_large_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"value".as_slice()))
            .topic(wire_topic("alias/topic"))
            .properties(
                PublishProperties::builder()
                    .topic_alias(NonZero::new(2).expect("non-zero alias"))
                    .build(),
            )
            .build(),
    );

    assert_eq!(
        read(&mut client, &alias_too_large_publish),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(client.poll_read().is_none());
}

/// The boundary case: an alias exactly equal to `topic_alias_maximum` is
/// still valid ([MQTT-3.3.2-13] allows Topic Alias values `1..=Topic Alias
/// Maximum`); only a value strictly greater is a protocol error.
#[test]
fn inbound_publish_alias_equal_to_topic_alias_maximum_is_accepted() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        topic_alias_maximum: Some(2),
        ..Default::default()
    }));
    let _ = connect_default(&mut client);

    let alias = NonZero::new(2).expect("non-zero alias");

    let register_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"first".as_slice()))
            .topic(wire_topic("t"))
            .properties(PublishProperties::builder().topic_alias(alias).build())
            .build(),
    );
    assert_eq!(read(&mut client, &register_publish), Ok(()));
    match client.poll_read() {
        Some(Event::Message(message)) => {
            assert_eq!(message.topic, ByteString::from_static("t"));
            assert_eq!(message.payload, Bytes::from_static(b"first"));
        }
        other => panic!("expected received message, got {other:?}"),
    }

    let alias_only_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"second".as_slice()))
            .topic(wire_topic(""))
            .properties(PublishProperties::builder().topic_alias(alias).build())
            .build(),
    );
    assert_eq!(read(&mut client, &alias_only_publish), Ok(()));
    match client.poll_read() {
        Some(Event::Message(message)) => {
            assert_eq!(message.topic, ByteString::from_static("t"));
            assert_eq!(message.payload, Bytes::from_static(b"second"));
        }
        other => panic!("expected received message, got {other:?}"),
    }

    let alias_over_limit_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"third".as_slice()))
            .topic(wire_topic("t"))
            .properties(
                PublishProperties::builder()
                    .topic_alias(NonZero::new(3).expect("non-zero alias"))
                    .build(),
            )
            .build(),
    );
    assert_eq!(
        read(&mut client, &alias_over_limit_publish),
        Err(Error::ProtocolError)
    );
}

#[test]
fn inbound_qos1_publish_waits_for_app_ack_then_sends_puback() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let packet_id = NonZero::new(7).expect("non-zero packet id");
    let publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            })
            .payload(Payload::new(b"27.5".as_slice()))
            .topic(wire_topic("sensors/temp"))
            .build(),
    );
    assert_eq!(read(&mut client, &publish), Ok(()));

    let inbound_message_id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, message)) => {
            assert_eq!(message.topic, ByteString::from_static("sensors/temp"));
            assert_eq!(message.payload, Bytes::from_static(b"27.5"));
            id
        }
        other => panic!("expected received message, got {other:?}"),
    };
    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(Command::Acknowledge(inbound_message_id)),
        Ok(())
    );

    let expected_puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(PubAckReasonCode::Success)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));
    assert!(client.poll_read().is_none());
}

#[test]
fn inbound_qos1_publish_reject_sends_puback_failure_reason() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let packet_id = NonZero::new(11).expect("non-zero packet id");
    let publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            })
            .payload(Payload::new(b"42".as_slice()))
            .topic(wire_topic("sensors/humidity"))
            .build(),
    );
    assert_eq!(read(&mut client, &publish), Ok(()));

    let inbound_message_id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected received message, got {other:?}"),
    };
    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(Command::Reject(
            inbound_message_id,
            RejectReason::NotAuthorized
        )),
        Ok(())
    );

    let expected_puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(PubAckReasonCode::NotAuthorized)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));
    assert!(client.poll_read().is_none());
}

#[test]
fn inbound_qos2_publish_waits_for_app_ack_then_sends_pubrec_and_completes_on_pubrel() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let packet_id = NonZero::new(13).expect("non-zero packet id");
    let publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(b"qos2".as_slice()))
            .topic(wire_topic("sensors/pressure"))
            .build(),
    );
    assert_eq!(read(&mut client, &publish), Ok(()));
    let inbound_message_id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected received message with acknowledgement, got {other:?}"),
    };
    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(Command::Acknowledge(inbound_message_id)),
        Ok(())
    );

    let expected_pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::Success)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));

    let pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(packet_id)
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrel), Ok(()));

    let expected_pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(PubCompReasonCode::Success)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubcomp)));
    assert!(client.poll_event().is_none());
}

#[test]
fn inbound_qos2_publish_reject_sends_pubrec_failure_and_clears_state() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let packet_id = NonZero::new(13).expect("non-zero packet id");
    let publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(b"qos2".as_slice()))
            .topic(wire_topic("sensors/pressure"))
            .build(),
    );
    assert_eq!(read(&mut client, &publish), Ok(()));
    let inbound_message_id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected received message with acknowledgement, got {other:?}"),
    };
    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(Command::Reject(
            inbound_message_id,
            RejectReason::QuotaExceeded
        )),
        Ok(())
    );

    let expected_pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::QuotaExceeded)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));

    let pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(packet_id)
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrel), Ok(()));

    let expected_pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(PubCompReasonCode::PacketIdentifierNotFound)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubcomp)));
    assert!(client.poll_event().is_none());
}

#[test]
fn inbound_packet_id_reuse_conflict_causes_protocol_error() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let packet_id = NonZero::new(19).expect("non-zero packet id");
    let qos1_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            })
            .payload(Payload::new(b"qos1".as_slice()))
            .topic(wire_topic("state/conflict"))
            .build(),
    );
    assert_eq!(read(&mut client, &qos1_publish), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(Event::MessageRequiresAcknowledgement(_, _))
    ));

    let qos2_same_packet_id = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(b"qos2".as_slice()))
            .topic(wire_topic("state/conflict"))
            .build(),
    );

    assert_eq!(
        read(&mut client, &qos2_same_packet_id),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn duplicate_qos2_publish_after_reject_resends_same_failure_pubrec_without_redelivery() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let packet_id = NonZero::new(23).expect("non-zero packet id");
    let first_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(b"first".as_slice()))
            .topic(wire_topic("state/reject"))
            .build(),
    );

    assert_eq!(read(&mut client, &first_publish), Ok(()));
    let inbound_message_id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected received message with acknowledgement, got {other:?}"),
    };
    assert_eq!(
        client.handle_write(Command::Reject(
            inbound_message_id,
            RejectReason::NotAuthorized
        )),
        Ok(())
    );

    let expected_pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::NotAuthorized)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));

    let duplicate_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: true,
            })
            .payload(Payload::new(b"duplicate".as_slice()))
            .topic(wire_topic("state/reject"))
            .build(),
    );

    assert_eq!(read(&mut client, &duplicate_publish), Ok(()));
    assert!(client.poll_read().is_none());
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));
}

#[test]
fn manual_ack_sends_puback_success_for_pending_message_id() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let packet_id = NonZero::new(77).expect("non-zero packet id");
    let publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            })
            .payload(Payload::new(b"unknown".as_slice()))
            .topic(wire_topic("unknown/id"))
            .build(),
    );
    assert_eq!(read(&mut client, &publish), Ok(()));

    let inbound_message_id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
        other => panic!("expected received message with acknowledgement, got {other:?}"),
    };

    assert_eq!(
        client.handle_write(Command::Acknowledge(inbound_message_id)),
        Ok(())
    );

    let expected_puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(PubAckReasonCode::Success)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));
    assert!(client.poll_write().is_none());
}

#[test]
fn reject_reason_maps_to_puback_failure_codes_for_qos1() {
    let cases = [
        (
            RejectReason::UnspecifiedError,
            PubAckReasonCode::UnspecifiedError,
        ),
        (
            RejectReason::ImplementationSpecificError,
            PubAckReasonCode::ImplementationSpecificError,
        ),
        (RejectReason::NotAuthorized, PubAckReasonCode::NotAuthorized),
        (
            RejectReason::TopicNameInvalid,
            PubAckReasonCode::TopicNameInvalid,
        ),
        (RejectReason::QuotaExceeded, PubAckReasonCode::QuotaExceeded),
        (
            RejectReason::PayloadFormatInvalid,
            PubAckReasonCode::PayloadFormatInvalid,
        ),
    ];

    for (offset, (reject_reason, expected_reason_code)) in cases.iter().enumerate() {
        let mut client = Client::<Duration>::default();
        let _ = connect_default(&mut client);

        let packet_id = NonZero::new((offset + 1) as u16).expect("non-zero packet id");
        let publish = ControlPacket::Publish(
            Publish::builder()
                .kind(PublishKind::Repetible {
                    packet_id,
                    qos: GuaranteedQoS::AtLeastOnce,
                    dup: false,
                })
                .payload(Payload::new(b"mapping".as_slice()))
                .topic(wire_topic("reject/reason/qos1"))
                .build(),
        );

        assert_eq!(read(&mut client, &publish), Ok(()));
        let inbound_message_id = match client.poll_read() {
            Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
            other => panic!("expected received message with acknowledgement, got {other:?}"),
        };

        assert_eq!(
            client.handle_write(Command::Reject(inbound_message_id, *reject_reason)),
            Ok(())
        );

        let expected_puback = ControlPacket::PubAck(
            PubAck::builder()
                .packet_id(packet_id)
                .reason_code(*expected_reason_code)
                .build(),
        );
        assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));
    }
}

#[test]
fn reject_reason_maps_to_pubrec_failure_codes_for_qos2() {
    let cases = [
        (
            RejectReason::UnspecifiedError,
            PubRecReasonCode::UnspecifiedError,
        ),
        (
            RejectReason::ImplementationSpecificError,
            PubRecReasonCode::ImplementationSpecificError,
        ),
        (RejectReason::NotAuthorized, PubRecReasonCode::NotAuthorized),
        (
            RejectReason::TopicNameInvalid,
            PubRecReasonCode::TopicNameInvalid,
        ),
        (RejectReason::QuotaExceeded, PubRecReasonCode::QuotaExceeded),
        (
            RejectReason::PayloadFormatInvalid,
            PubRecReasonCode::PayloadFormatInvalid,
        ),
    ];

    for (offset, (reject_reason, expected_reason_code)) in cases.iter().enumerate() {
        let mut client = Client::<Duration>::default();
        let _ = connect_default(&mut client);

        let packet_id = NonZero::new((offset + 1) as u16).expect("non-zero packet id");
        let publish = ControlPacket::Publish(
            Publish::builder()
                .kind(PublishKind::Repetible {
                    packet_id,
                    qos: GuaranteedQoS::ExactlyOnce,
                    dup: false,
                })
                .payload(Payload::new(b"mapping".as_slice()))
                .topic(wire_topic("reject/reason/qos2"))
                .build(),
        );

        assert_eq!(read(&mut client, &publish), Ok(()));
        let inbound_message_id = match client.poll_read() {
            Some(Event::MessageRequiresAcknowledgement(id, _)) => id,
            other => panic!("expected received message with acknowledgement, got {other:?}"),
        };

        assert_eq!(
            client.handle_write(Command::Reject(inbound_message_id, *reject_reason)),
            Ok(())
        );

        let expected_pubrec = ControlPacket::PubRec(
            PubRec::builder()
                .packet_id(packet_id)
                .reason_code(*expected_reason_code)
                .build(),
        );
        assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrec)));
    }
}

#[test]
fn inbound_qos2_unknown_pubrel_sends_pubcomp_packet_identifier_not_found() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let unknown_packet_id = NonZero::new(21).expect("non-zero packet id");
    let pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(unknown_packet_id)
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );

    assert_eq!(read(&mut client, &pubrel), Ok(()));

    let expected_pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(unknown_packet_id)
            .reason_code(PubCompReasonCode::PacketIdentifierNotFound)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubcomp)));
    assert!(client.poll_event().is_none());
}

#[test]
fn socket_closed_after_disconnect_does_not_duplicate_disconnected_event() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let disconnect = ControlPacket::Disconnect(
        Disconnect::builder()
            .reason_code(DisconnectReasonCode::NormalDisconnection)
            .build(),
    );
    assert_eq!(read(&mut client, &disconnect), Ok(()));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert!(client.poll_read().is_none());
}

#[test]
fn outbound_qos1_publish_emits_acknowledged_event_on_puback() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let qos1_message = message("test/topic", ProtoQos::AtLeastOnce, b"qos1");

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 42,
            message: qos1_message.clone(),
        }),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            })
            .payload(Payload::new(qos1_message.payload.clone()))
            .topic(wire_topic("test/topic"))
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_publish)));

    let puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(PubAckReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &puback), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishAcknowledged {
            token: 42,
            reason: ReasonCode::Success,
        })
    );
    assert!(client.poll_read().is_none());
}

#[test]
fn unexpected_puback_without_matching_qos1_transaction_triggers_protocol_error_close() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let packet_id = NonZero::new(42).expect("non-zero packet id");
    let puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(PubAckReasonCode::Success)
            .build(),
    );

    assert_eq!(read(&mut client, &puback), Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn qos2_inflight_receiving_puback_triggers_protocol_error_close() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let qos2_message = message("test/topic", ProtoQos::ExactlyOnce, b"qos2");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: qos2_message,
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(PubAckReasonCode::Success)
            .build(),
    );

    assert_eq!(read(&mut client, &puback), Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(client.poll_read().is_none());
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn qos1_inflight_receiving_pubrec_triggers_protocol_error_close() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let qos1_message = message("test/topic", ProtoQos::AtLeastOnce, b"qos1");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: qos1_message,
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::Success)
            .build(),
    );

    assert_eq!(read(&mut client, &pubrec), Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(client.poll_read().is_none());
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn connack_receive_maximum_only_limits_broker_facing_publish_flow() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_receive_maximum(NonZero::new(1))
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let first_message = message("test/topic", ProtoQos::AtLeastOnce, b"qos1-first");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: first_message.clone(),
        }),
        Ok(())
    );

    let first_packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_first_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id: first_packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            })
            .payload(Payload::new(first_message.payload.clone()))
            .topic(wire_topic("test/topic"))
            .build(),
    );
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&expected_first_publish))
    );

    let inbound_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"inbound-ok".as_slice()))
            .topic(wire_topic("inbound/unchanged"))
            .build(),
    );
    assert_eq!(read(&mut client, &inbound_publish), Ok(()));
    assert!(matches!(
        client.poll_read(),
        Some(Event::Message(message)) if message.payload == Bytes::from_static(b"inbound-ok")
    ));

    let inbound_qos1_packet_id = NonZero::new(41).expect("non-zero packet id");
    let inbound_qos1_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id: inbound_qos1_packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            })
            .payload(Payload::new(b"inbound-qos1-ok".as_slice()))
            .topic(wire_topic("inbound/qos1"))
            .build(),
    );
    assert_eq!(read(&mut client, &inbound_qos1_publish), Ok(()));
    let inbound_message_id = match client.poll_read() {
        Some(Event::MessageRequiresAcknowledgement(id, message)) => {
            assert_eq!(message.payload, Bytes::from_static(b"inbound-qos1-ok"));
            id
        }
        other => panic!("expected qos1 inbound message with ack id, got {other:?}"),
    };
    assert_eq!(
        client.handle_write(Command::Acknowledge(inbound_message_id)),
        Ok(())
    );
    let expected_puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(inbound_qos1_packet_id)
            .reason_code(PubAckReasonCode::Success)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_puback)));

    let second_message = message("test/second", ProtoQos::AtLeastOnce, b"qos1-second");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 2,
            message: second_message,
        }),
        Err(Error::ReceiveMaximumExceeded)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn effective_limits_recompute_on_connect_socketconnected_connack_and_socketclosed() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        max_outgoing_qos: Some(ProtoQos::AtMostOnce),
        ..Default::default()
    }));

    let qos1_message = message("test/topic", ProtoQos::AtLeastOnce, b"qos1");

    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maximum_qos(MaximumQoS::AtLeastOnce)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: qos1_message.clone(),
        }),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maximum_qos(MaximumQoS::AtLeastOnce)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 2,
            message: qos1_message,
        }),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn effective_limits_recompute_on_connect_applies_pending_connect_options() {
    let mut client = Client::<Duration>::new(ClientSettings::default());
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maximum_qos(MaximumQoS::AtLeastOnce)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let qos2_message = message("test/qos2", ProtoQos::ExactlyOnce, b"qos2");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: qos2_message,
        }),
        Err(Error::ProtocolError)
    );
}

#[test]
fn effective_limits_recompute_on_connack_applies_broker_receive_maximum() {
    let mut client = Client::<Duration>::new(ClientSettings::default());
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_receive_maximum(NonZero::new(1))
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let first = message("test/first", ProtoQos::AtLeastOnce, b"1");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: first,
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let second = message("test/second", ProtoQos::AtLeastOnce, b"2");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 2,
            message: second,
        }),
        Err(Error::ReceiveMaximumExceeded)
    );
}

#[test]
fn app_retain_policy_false_blocks_retain_publish_even_if_broker_allows() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        allow_retain: false,
        ..Default::default()
    }));
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(ConnAckProperties::builder().retain_available(true).build()),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let mut retained_message = message("retain/topic", ProtoQos::AtMostOnce, b"retained");
    retained_message.retain = true;
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: retained_message,
        }),
        Err(Error::ProtocolError)
    );
}

#[test]
fn app_subscription_policy_flags_override_broker_allowances() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        allow_subscription_identifiers: false,
        allow_wildcard_subscriptions: false,
        allow_shared_subscriptions: false,
        ..Default::default()
    }));
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .wildcard_subscription_available(true)
                    .shared_subscription_available(true)
                    .subscription_identifiers_available(true)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(Command::Subscribe(
            SubscribeOptions::builder()
                .subscriptions(vec![app_subscription("topic/+")])
                .identifier(NonZero::new(1u64).unwrap())
                .build(),
        )),
        Err(Error::ProtocolError)
    );

    assert_eq!(
        client.handle_write(subscribe_one(app_subscription("$share/g/topic"))),
        Err(Error::ProtocolError)
    );
}

#[test]
fn outbound_qos2_publish_emits_completed_event_on_pubcomp() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let qos2_message = message("test/topic", ProtoQos::ExactlyOnce, b"qos2");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 9,
            message: qos2_message.clone(),
        }),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(qos2_message.payload.clone()))
            .topic(wire_topic("test/topic"))
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_publish)));

    let pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrec), Ok(()));

    let expected_pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(packet_id)
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrel)));

    let pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(PubCompReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubcomp), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishCompleted {
            token: 9,
            reason: ReasonCode::Success,
        })
    );
    assert!(client.poll_read().is_none());
}

#[test]
fn unexpected_pubcomp_before_pubrec_transition_triggers_protocol_error_close() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let qos2_message = message("test/topic", ProtoQos::ExactlyOnce, b"qos2");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: qos2_message,
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(PubCompReasonCode::Success)
            .build(),
    );

    assert_eq!(read(&mut client, &pubcomp), Err(Error::ProtocolError));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x82, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn receive_maximum_full_returns_immediate_error_for_new_qos2_publish() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_receive_maximum(NonZero::new(1))
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let first_message = message("test/topic", ProtoQos::ExactlyOnce, b"qos2-first");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: first_message.clone(),
        }),
        Ok(())
    );

    let first_packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_first_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id: first_packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(first_message.payload.clone()))
            .topic(wire_topic("test/topic"))
            .build(),
    );
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&expected_first_publish))
    );

    let second_message = message("test/topic", ProtoQos::ExactlyOnce, b"qos2-second");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 2,
            message: second_message,
        }),
        Err(Error::ReceiveMaximumExceeded)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn duplicate_pubrec_in_qos2_await_pubcomp_resends_pubrel_without_disconnect() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let qos2_message = message("test/topic", ProtoQos::ExactlyOnce, b"qos2");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: qos2_message.clone(),
        }),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let expected_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(qos2_message.payload.clone()))
            .topic(wire_topic("test/topic"))
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_publish)));

    let pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrec), Ok(()));

    let expected_pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(packet_id)
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrel)));

    assert_eq!(read(&mut client, &pubrec), Ok(()));
    assert_eq!(client.poll_write(), Some(encode_packet(&expected_pubrel)));
    assert!(client.poll_event().is_none());

    let pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(PubCompReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubcomp), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishCompleted {
            token: 1,
            reason: ReasonCode::Success,
        })
    );
}

#[test]
fn qos2_pubrec_failure_reason_drops_inflight_without_pubrel() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let qos2_message = message("test/topic", ProtoQos::ExactlyOnce, b"qos2");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 5,
            message: qos2_message,
        }),
        Ok(())
    );
    let packet_id = NonZero::new(1).expect("non-zero packet id");
    assert!(client.poll_write().is_some());

    let pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::NotAuthorized)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrec), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishDropped {
            token: 5,
            reason: DropReason::BrokerRejected(ReasonCode::NotAuthorized),
        })
    );

    let pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(PubCompReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubcomp), Err(Error::ProtocolError));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn publish_rejects_packet_exceeding_connack_maximum_packet_size() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_maximum_packet_size(NonZero::new(16))
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let message = message("test/topic", ProtoQos::AtMostOnce, &[0; 64]);
    assert_eq!(
        client.handle_write(Command::Publish { token: 1, message }),
        Err(Error::PacketTooLarge)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_rejects_packet_exceeding_connack_maximum_packet_size() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_maximum_packet_size(NonZero::new(20))
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(subscribe_one(app_subscription("a/very/long/topic/filter"))),
        Err(Error::PacketTooLarge)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn repeated_connect_does_not_duplicate_open_socket_event() {
    let mut client = Client::<Duration>::default();

    assert_eq!(
        client.handle_write(Command::Connect(connect_options())),
        Ok(())
    );
    assert_eq!(
        client.handle_write(Command::Connect(connect_options())),
        Ok(())
    );

    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::OpenSocket)
    ));
    assert!(client.poll_event().is_none());
}

#[test]
fn rejected_connect_does_not_mutate_pending_connect_options() {
    let mut client = Client::<Duration>::default();

    let initial_options = ConnectOptions::builder()
        .client_id(ByteString::from_static("initial-client"))
        .build();
    let rejected_options = ConnectOptions::builder()
        .client_id(ByteString::from_static("rejected-client"))
        .build();

    let mut rejected_connect_reference = Client::<Duration>::default();
    let rejected_connect_bytes =
        open_connecting(&mut rejected_connect_reference, rejected_options.clone());

    let first_connect_bytes = open_connecting(&mut client, initial_options);
    assert_ne!(first_connect_bytes, rejected_connect_bytes);

    assert_eq!(
        client.handle_write(Command::Connect(rejected_options)),
        Err(Error::InvalidStateTransition)
    );
    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    let reconnect_bytes = client.poll_write().expect("connect bytes are queued");

    assert_eq!(reconnect_bytes, first_connect_bytes);
}

#[test]
fn reconnect_ignores_previous_connack_maximum_packet_size_for_connect() {
    let mut client = Client::<Duration>::default();
    let first_connect = open_connecting(&mut client, connect_options());

    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_maximum_packet_size(NonZero::new(8))
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    let reconnect_connect = client.poll_write().expect("connect bytes are queued");

    assert_eq!(reconnect_connect, first_connect);
}

#[test]
fn connack_resume_with_clean_start_is_protocol_error() {
    let mut client = Client::<Duration>::default();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .clean_start(true)
        .build();
    let _ = open_connecting(&mut client, options);

    assert_eq!(
        read(&mut client, &resume_connack()),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(client.poll_read().is_none());
}

#[test]
fn connack_resume_without_local_state_is_accepted_when_clean_start_false() {
    let mut client = Client::<Duration>::default();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .clean_start(false)
        .build();
    let _ = open_connecting(&mut client, options);

    assert_eq!(read(&mut client, &resume_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
    assert!(client.poll_event().is_none());
}

fn session_expiry_options(secs: u64) -> ConnectOptions {
    ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .session_expiry(Duration::from_secs(secs))
        .build()
}

#[test]
fn resumed_session_replays_outbound_qos_publish_with_dup_set() {
    let mut client = Client::<Duration>::default();
    let _ = connect_client(&mut client, session_expiry_options(30));

    let outbound = message("replay/topic", ProtoQos::AtLeastOnce, b"replay");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: outbound.clone(),
        }),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    let first_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            })
            .payload(Payload::new(outbound.payload.clone()))
            .topic(wire_topic("replay/topic"))
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&first_publish)));

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &resume_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let replay_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: true,
            })
            .payload(Payload::new(outbound.payload))
            .topic(wire_topic("replay/topic"))
            .build(),
    );
    assert_eq!(client.poll_write(), Some(encode_packet(&replay_publish)));

    let puback = ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(PubAckReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &puback), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishAcknowledged {
            token: 1,
            reason: ReasonCode::Success,
        })
    );
}

#[test]
fn resumed_session_replay_failure_does_not_emit_connected_and_closes() {
    let mut client = Client::<Duration>::default();
    let _ = connect_client(&mut client, session_expiry_options(30));

    let outbound = message("replay/failure", ProtoQos::AtLeastOnce, &[0; 64]);
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: outbound.clone(),
        }),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::Publish(
            Publish::builder()
                .kind(PublishKind::Repetible {
                    packet_id,
                    qos: GuaranteedQoS::AtLeastOnce,
                    dup: false,
                })
                .payload(Payload::new(outbound.payload))
                .topic(wire_topic("replay/failure"))
                .build()
        )))
    );

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());

    let resumed_connack = ControlPacket::ConnAck(
        ConnAck::builder()
            .kind(ConnAckKind::ResumePreviousSession)
            .properties(
                ConnAckProperties::builder()
                    .maybe_maximum_packet_size(NonZero::new(16))
                    .build(),
            )
            .build(),
    );
    assert_eq!(
        read(&mut client, &resumed_connack),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(client.poll_read().is_none());
}

#[test]
fn resumed_session_replays_unacknowledged_pubrel() {
    let mut client = Client::<Duration>::default();
    let _ = connect_client(&mut client, session_expiry_options(30));

    let outbound = message("resume/qos2", ProtoQos::ExactlyOnce, b"qos2");
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: outbound.clone(),
        }),
        Ok(())
    );

    let packet_id = NonZero::new(1).expect("non-zero packet id");
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::Publish(
            Publish::builder()
                .kind(PublishKind::Repetible {
                    packet_id,
                    qos: GuaranteedQoS::ExactlyOnce,
                    dup: false,
                })
                .payload(Payload::new(outbound.payload))
                .topic(wire_topic("resume/qos2"))
                .build()
        )))
    );

    let pubrec = ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(PubRecReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrec), Ok(()));
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubRel(
            PubRel::builder()
                .packet_id(packet_id)
                .reason_code(PubRelReasonCode::Success)
                .build()
        )))
    );

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &resume_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubRel(
            PubRel::builder()
                .packet_id(packet_id)
                .reason_code(PubRelReasonCode::Success)
                .build()
        )))
    );

    let pubcomp = ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(PubCompReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubcomp), Ok(()));
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishCompleted {
            token: 1,
            reason: ReasonCode::Success,
        })
    );
}

#[test]
fn non_resumed_session_drops_inflight_and_emits_publish_dropped_events() {
    let mut client = Client::<Duration>::default();
    let _ = connect_client(&mut client, session_expiry_options(30));

    let qos1_topic = "drop/topic";
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message(qos1_topic, ProtoQos::AtLeastOnce, b"qos1"),
        }),
        Ok(())
    );
    let qos1_packet_id = NonZero::new(1).expect("non-zero packet id");
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::Publish(
            Publish::builder()
                .kind(PublishKind::Repetible {
                    packet_id: qos1_packet_id,
                    qos: GuaranteedQoS::AtLeastOnce,
                    dup: false,
                })
                .payload(Payload::new(b"qos1".as_slice()))
                .topic(wire_topic(qos1_topic))
                .build()
        )))
    );

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 2,
            message: message(qos1_topic, ProtoQos::ExactlyOnce, b"qos2"),
        }),
        Ok(())
    );
    let qos2_packet_id = NonZero::new(2).expect("non-zero packet id");
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::Publish(
            Publish::builder()
                .kind(PublishKind::Repetible {
                    packet_id: qos2_packet_id,
                    qos: GuaranteedQoS::ExactlyOnce,
                    dup: false,
                })
                .payload(Payload::new(b"qos2".as_slice()))
                .topic(wire_topic(qos1_topic))
                .build()
        )))
    );

    let inbound_packet_id = NonZero::new(33).expect("non-zero packet id");
    let inbound_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id: inbound_packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(b"inbound".as_slice()))
            .topic(wire_topic("inbound/topic"))
            .build(),
    );
    assert_eq!(read(&mut client, &inbound_publish), Ok(()));
    let _ = client.poll_read();
    assert_eq!(client.poll_write(), None);

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishDropped {
            token: 1,
            reason: DropReason::SessionNotResumed,
        })
    );
    assert_eq!(
        client.poll_read(),
        Some(Event::PublishDropped {
            token: 2,
            reason: DropReason::SessionNotResumed,
        })
    );
    assert!(client.poll_read().is_none());
    assert_eq!(client.poll_write(), None);

    let pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(inbound_packet_id)
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrel), Ok(()));
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubComp(
            PubComp::builder()
                .packet_id(inbound_packet_id)
                .reason_code(PubCompReasonCode::PacketIdentifierNotFound)
                .build()
        )))
    );
}

#[test]
fn non_resumed_connack_discards_all_local_session_state() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("state/topic", ProtoQos::AtLeastOnce, b"qos1"),
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    let inbound_packet_id = NonZero::new(55).expect("non-zero packet id");
    let inbound_publish = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::Repetible {
                packet_id: inbound_packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            })
            .payload(Payload::new(b"inbound".as_slice()))
            .topic(wire_topic("state/inbound"))
            .build(),
    );
    assert_eq!(read(&mut client, &inbound_publish), Ok(()));
    let _ = client.poll_read();
    assert_eq!(client.poll_write(), None);

    assert_eq!(
        client.handle_write(subscribe_one(app_subscription("state/sub"))),
        Ok(())
    );
    let _ = client.poll_write().expect("subscribe frame expected");

    assert_eq!(client.handle_write(unsubscribe_one("state/sub")), Ok(()));
    let _ = client.poll_write().expect("unsubscribe frame expected");

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let pubrel = ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(inbound_packet_id)
            .reason_code(PubRelReasonCode::Success)
            .build(),
    );
    assert_eq!(read(&mut client, &pubrel), Ok(()));
    assert_eq!(
        client.poll_write(),
        Some(encode_packet(&ControlPacket::PubComp(
            PubComp::builder()
                .packet_id(inbound_packet_id)
                .reason_code(PubCompReasonCode::PacketIdentifierNotFound)
                .build()
        )))
    );

    let stale_suback = ControlPacket::SubAck(
        SubAck::builder()
            .packet_id(NonZero::new(2).expect("non-zero"))
            .reason_codes(vec![SubAckReasonCode::SuccessQoS0])
            .build(),
    );
    assert_eq!(read(&mut client, &stale_suback), Err(Error::ProtocolError));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));

    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    assert_eq!(client.handle_write(unsubscribe_one("state/unsub")), Ok(()));
    let _ = client.poll_write().expect("unsubscribe frame expected");

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let stale_unsuback = ControlPacket::UnsubAck(
        UnsubAck::builder()
            .packet_id(NonZero::new(1).expect("non-zero"))
            .reason_codes(vec![UnsubAckReasonCode::Success])
            .build(),
    );
    assert_eq!(
        read(&mut client, &stale_unsuback),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn stale_read_buffer_is_cleared_on_socket_closed() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: (&[0x20]),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

#[test]
fn stale_read_buffer_is_cleared_on_socket_error() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: (&[0x20]),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );

    assert_eq!(
        client.handle_event(DriverEvent::SocketError),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

#[test]
fn stale_read_buffer_is_cleared_on_user_disconnect() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: (&[0x20]),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );

    assert_eq!(client.handle_write(Command::Disconnect), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

#[test]
fn stale_read_buffer_is_cleared_on_close() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        client.handle_read(IncomingData {
            bytes: (&[0x20]),
            received_at: Duration::ZERO,
        }),
        Ok(())
    );

    assert_eq!(client.close(), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

#[test]
fn timeout_in_connected_state_enqueues_pingreq() {
    let mut client = Client::<Duration>::default();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .keep_alive(NonZero::new(10).unwrap())
        .build();
    let _ = connect_client(&mut client, options);

    assert_eq!(client.handle_timeout(Duration::from_secs(42)), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xC0, 0x00])));
    // [MQTT-3.1.2-24] After PINGREQ the next deadline is now + interval/2 (= 42
    // + 5 = 47) so total elapsed from last packet is 1.5× the keep-alive
    // interval.
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(47)));
}

#[test]
fn close_enqueues_disconnect_and_close_socket() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    assert_eq!(client.close(), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert!(client.poll_read().is_none());

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(client.poll_read().is_none());
}

#[test]
fn close_succeeds_even_when_disconnect_packet_exceeds_maximum_packet_size() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_maximum_packet_size(NonZero::new(1))
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(client.close(), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert!(client.poll_read().is_none());

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(client.poll_read().is_none());
}

#[test]
fn user_disconnect_succeeds_even_when_disconnect_packet_exceeds_maximum_packet_size() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_maximum_packet_size(NonZero::new(1))
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(client.handle_write(Command::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert!(client.poll_read().is_none());

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(client.poll_read().is_none());
}

#[test]
fn timeout_is_cleared_on_close() {
    let mut close_client = Client::<Duration>::default();
    let options = || {
        ConnectOptions::builder()
            .client_id(ByteString::from_static("test-client"))
            .keep_alive(NonZero::new(10).unwrap())
            .build()
    };
    let _ = connect_client(&mut close_client, options());

    assert_eq!(close_client.handle_timeout(Duration::from_secs(42)), Ok(()));
    // [MQTT-3.1.2-24] After PINGREQ the next deadline is now + interval/2 (= 42
    // + 5 = 47).
    assert_eq!(close_client.poll_timeout(), Some(Duration::from_secs(47)));

    assert_eq!(close_client.close(), Ok(()));
    assert_eq!(close_client.poll_timeout(), None);

    let mut socket_closed_client = Client::<Duration>::default();
    let _ = connect_client(&mut socket_closed_client, options());

    assert_eq!(
        socket_closed_client.handle_timeout(Duration::from_secs(99)),
        Ok(())
    );
    // [MQTT-3.1.2-24] After PINGREQ the next deadline is now + interval/2 (= 99
    // + 5 = 104).
    assert_eq!(
        socket_closed_client.poll_timeout(),
        Some(Duration::from_secs(104))
    );

    assert_eq!(
        socket_closed_client.handle_event(DriverEvent::SocketClosed),
        Ok(())
    );
    assert_eq!(socket_closed_client.poll_timeout(), None);
}

fn scram_authentication() -> Authentication {
    Authentication::builder()
        .method(ByteString::from_static("SCRAM"))
        .build()
}

fn scram_auth_packet(reason_code: AuthReasonCode) -> ControlPacket {
    ControlPacket::Auth(
        Auth::builder()
            .reason_code(reason_code)
            .properties(
                AuthProperties::builder()
                    .authentication(AuthenticationKind::WithoutData {
                        method: Utf8String::try_from("SCRAM").expect("valid utf8"),
                    })
                    .build(),
            )
            .build(),
    )
}

fn connect_options_with_auth() -> ConnectOptions {
    ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .authentication(scram_authentication())
        .build()
}

#[test]
fn connecting_accepts_auth_and_stays_open() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options_with_auth());

    assert_eq!(
        read(
            &mut client,
            &scram_auth_packet(AuthReasonCode::ContinueAuthentication)
        ),
        Ok(())
    );
    assert!(client.poll_event().is_none());
}

#[test]
fn connecting_auth_then_connack_success_transitions_connected() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options_with_auth());

    assert_eq!(
        read(
            &mut client,
            &scram_auth_packet(AuthReasonCode::ContinueAuthentication)
        ),
        Ok(())
    );
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

#[test]
fn connecting_auth_without_configured_authentication_is_protocol_error() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        read(
            &mut client,
            &scram_auth_packet(AuthReasonCode::ContinueAuthentication)
        ),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn connecting_auth_with_reason_other_than_continue_is_protocol_error() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options_with_auth());

    assert_eq!(
        read(&mut client, &scram_auth_packet(AuthReasonCode::Success)),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

#[test]
fn publish_qos_above_server_maximum_qos_is_rejected() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maximum_qos(MaximumQoS::AtMostOnce)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("qos/guard", ProtoQos::AtLeastOnce, b"payload"),
        }),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn publish_retain_when_server_retain_not_available_is_rejected() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(ConnAckProperties::builder().retain_available(false).build()),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let mut retained_message = message("retain/guard", ProtoQos::AtMostOnce, b"payload");
    retained_message.retain = true;
    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: retained_message,
        }),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_shared_with_no_local_is_rejected() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let mut shared = app_subscription("$share/group/topic");
    shared.no_local = true;
    assert_eq!(
        client.handle_write(subscribe_one(shared)),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_wildcard_when_server_disallows_is_rejected() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .wildcard_subscription_available(false)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(subscribe_one(app_subscription("topic/#"))),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_shared_when_server_disallows_is_rejected() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .shared_subscription_available(false)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(subscribe_one(app_subscription("$share/g/topic"))),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn subscribe_identifier_when_server_disallows_is_rejected() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .subscription_identifiers_available(false)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(Command::Subscribe(
            SubscribeOptions::builder()
                .subscriptions(vec![app_subscription("topic/a")])
                .identifier(NonZero::new(1u64).unwrap())
                .build(),
        )),
        Err(Error::ProtocolError)
    );
    assert_eq!(client.poll_write(), None);
}

#[test]
fn connecting_auth_continue_then_connack_success_connects() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options_with_auth());

    assert_eq!(
        read(
            &mut client,
            &scram_auth_packet(AuthReasonCode::ContinueAuthentication)
        ),
        Ok(())
    );
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
}

/// [MQTT-4.12.0-2] AUTH in the Connected state must be forwarded to the
/// application, not treated as a protocol error. The application decides how
/// to respond.
#[test]
fn auth_in_connected_state_is_forwarded_not_protocol_error() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let auth = ControlPacket::Auth(
        Auth::builder()
            .reason_code(AuthReasonCode::ContinueAuthentication)
            .build(),
    );
    assert_eq!(
        read(&mut client, &auth),
        Ok(()),
        "AUTH in Connected state must be forwarded to the application, not treated as an error"
    );
    assert!(
        matches!(client.poll_read(), Some(Event::Auth { .. })),
        "AUTH event must be emitted to the application"
    );
    assert!(client.poll_event().is_none(), "no CloseSocket expected");
}

#[test]
fn keepalive_disabled_without_interval_no_pingreq() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    assert_eq!(client.handle_timeout(Duration::from_secs(1)), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn connack_server_keep_alive_zero_disables_keepalive_without_panic() {
    let mut client = Client::<Duration>::default();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .keep_alive(NonZero::new(10).unwrap())
        .build();
    let _ = open_connecting(&mut client, options);
    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(ConnAckProperties::builder().server_keep_alive(0).build()),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(client.handle_timeout(Duration::from_secs(1)), Ok(()));
    assert_eq!(client.poll_write(), None);
    assert_eq!(client.poll_timeout(), None);
}

#[test]
fn keepalive_timeout_without_pingresp_closes_connection() {
    let mut client = Client::<Duration>::default();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .keep_alive(NonZero::new(10).unwrap())
        .build();
    let _ = connect_client(&mut client, options);

    assert_eq!(client.handle_timeout(Duration::from_secs(1)), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xC0, 0x00])));

    assert_eq!(
        client.handle_timeout(Duration::from_secs(2)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x8D, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

/// [MQTT-3.1.4-5] If a Client does not receive a CONNACK packet from the
/// Server within a reasonable amount of time, the Client SHOULD close the
/// Network Connection. When `handle_timeout` fires in the Start state (before
/// CONNECT is sent), it should treat the timeout as a connection-establishment
/// timeout and close the socket.
#[test]
fn timeout_in_start_state_closes_connection_with_connect_timeout_error() {
    let mut client = Client::<Duration>::default();

    assert_eq!(
        client.handle_timeout(Duration::from_secs(100)),
        Err(Error::ConnectTimeout),
        "timeout in Start state must return ConnectTimeout"
    );
    assert!(
        matches!(client.poll_event(), Some(DriverAction::CloseSocket)),
        "CloseSocket must be emitted on Start timeout"
    );
    assert!(client.poll_read().is_none());
}

/// [MQTT-3.1.4-5] Same as above, but the timeout fires in the Connecting state
/// (after CONNECT, before CONNACK).
#[test]
fn timeout_in_connecting_state_closes_connection_with_connect_timeout_error() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        client.handle_timeout(Duration::from_secs(100)),
        Err(Error::ConnectTimeout),
        "timeout in Connecting state must return ConnectTimeout"
    );
    assert!(
        matches!(client.poll_event(), Some(DriverAction::CloseSocket)),
        "CloseSocket must be emitted on Connecting timeout"
    );
    assert!(client.poll_read().is_none());
}

/// [MQTT-3.1.2-4] The server may override the session expiry interval in
/// CONNACK properties. When the server returns `session_expiry_interval=0`,
/// `session_should_persist` must be false, so inflight messages are discarded
/// on disconnect and NOT re-queued on the next reconnect.
#[test]
fn connack_session_expiry_zero_overrides_client_session_should_persist() {
    let mut client = Client::<Duration>::default();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .clean_start(false)
        .session_expiry(Duration::from_secs(60))
        .build();
    let _ = open_connecting(&mut client, options);

    let connack_no_persist = connack_with_properties(
        ConnAckProperties::builder()
            .session_expiry_interval(0)
            .build(),
    );
    assert_eq!(read(&mut client, &connack_no_persist), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("test/topic", ProtoQos::AtLeastOnce, b"data"),
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some(), "PUBLISH must be sent");

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert!(client.poll_read().is_none(), "no drop event at disconnect");

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    let _ = client
        .poll_write()
        .expect("reconnect CONNECT frame expected");
    assert_eq!(read(&mut client, &connack_no_persist), Ok(()));
    let first = client.poll_read();
    assert!(
        matches!(first, Some(Event::Connected)),
        "expected Connected, got {first:?}"
    );
    assert!(
        client.poll_read().is_none(),
        "no PublishDropped events: inflight was already cleared at disconnect"
    );
}

/// [MQTT-3.1.2-4] When the server sets `session_expiry_interval > 0` in
/// CONNACK, `session_should_persist` must be true: inflight messages survive
/// the disconnect and are reported as dropped on the next reconnect (when the
/// session is not resumed).
#[test]
fn connack_session_expiry_nonzero_sets_session_should_persist() {
    let mut client = Client::<Duration>::default();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .clean_start(false)
        .session_expiry(Duration::ZERO)
        .build();
    let _ = open_connecting(&mut client, options);

    let connack_persist = connack_with_properties(
        ConnAckProperties::builder()
            .session_expiry_interval(120)
            .build(),
    );
    assert_eq!(read(&mut client, &connack_persist), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("test/topic", ProtoQos::AtLeastOnce, b"data"),
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some(), "PUBLISH must be sent");

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));
    assert!(client.poll_read().is_none(), "no extra events");

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    let _ = client
        .poll_write()
        .expect("reconnect CONNECT frame expected");
    assert_eq!(read(&mut client, &connack_persist), Ok(()));

    let first = client.poll_read();
    let second = client.poll_read();
    assert!(
        matches!(first, Some(Event::Connected)),
        "expected Connected, got {first:?}"
    );
    assert!(
        matches!(
            second,
            Some(Event::PublishDropped {
                reason: DropReason::SessionNotResumed,
                ..
            })
        ),
        "expected PublishDropped(SessionNotResumed), got {second:?}"
    );
}

#[test]
fn clean_start_true_clears_local_session_before_connect() {
    let mut client = Client::<Duration>::default();
    let first_options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .session_expiry(Duration::from_secs(30))
        .keep_alive(NonZero::new(10).unwrap())
        .build();
    let _ = connect_client(&mut client, first_options);

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("clean/start", ProtoQos::AtLeastOnce, b"qos1"),
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_write(Command::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    let clean_start_options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .clean_start(true)
        .session_expiry(Duration::from_secs(30))
        .build();
    let _ = open_connecting(&mut client, clean_start_options);

    assert_eq!(
        read(&mut client, &resume_connack()),
        Err(Error::ProtocolError)
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(client.poll_read().is_none());
}

#[test]
fn session_with_expiry_keeps_inflight_across_graceful_disconnect() {
    let mut client = Client::<Duration>::default();
    let _ = connect_client(&mut client, session_expiry_options(30));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("session/persist", ProtoQos::AtLeastOnce, b"persist"),
        }),
        Ok(())
    );
    let publish = client.poll_write().expect("publish expected");

    assert_eq!(client.handle_write(Command::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &resume_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let replay_publish = client.poll_write().expect("replayed publish expected");
    assert_eq!(replay_publish.len(), publish.len());
    assert_eq!(replay_publish[0], publish[0] | 0b0000_1000);
    assert_eq!(&replay_publish[1..], &publish[1..]);
}

#[test]
fn zero_session_expiry_clears_inflight_on_disconnect() {
    let mut client = Client::<Duration>::default();
    let _ = connect_client(&mut client, session_expiry_options(0));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("session/clear", ProtoQos::AtLeastOnce, b"clear"),
        }),
        Ok(())
    );
    assert!(client.poll_write().is_some());

    assert_eq!(client.handle_write(Command::Disconnect), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xE0, 0x00])));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &resume_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
    assert_eq!(client.poll_write(), None);
    assert!(client.poll_event().is_none());
}

#[test]
fn zero_session_expiry_clears_inflight_on_socket_closed() {
    let mut client = Client::<Duration>::default();
    let _ = connect_client(&mut client, session_expiry_options(0));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("session/close-clear", ProtoQos::AtLeastOnce, b"clear"),
        }),
        Ok(())
    );
    let first_publish = client.poll_write().expect("publish expected");

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &resume_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let replay = client.poll_write();
    assert_eq!(replay, None);
    assert_ne!(replay, Some(first_publish));
}

#[test]
fn keepalive_timeout_with_session_expiry_preserves_inflight_for_resume() {
    let mut client = Client::<Duration>::default();
    let options = ConnectOptions::builder()
        .client_id(ByteString::from_static("test-client"))
        .keep_alive(NonZero::new(10).unwrap())
        .session_expiry(Duration::from_secs(30))
        .build();
    let _ = connect_client(&mut client, options);

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("session/timeout-persist", ProtoQos::AtLeastOnce, b"persist"),
        }),
        Ok(())
    );
    let first_publish = client.poll_write().expect("publish expected");

    assert_eq!(client.handle_timeout(Duration::from_secs(1)), Ok(()));
    assert_eq!(client.poll_write(), None);

    assert_eq!(client.handle_timeout(Duration::from_secs(2)), Ok(()));
    assert_eq!(client.poll_write(), Some(Bytes::from_static(&[0xC0, 0x00])));

    assert_eq!(
        client.handle_timeout(Duration::from_secs(3)),
        Err(Error::ProtocolError)
    );
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xE0, 0x02, 0x8D, 0x00]))
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));

    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    assert!(client.poll_write().is_some());
    assert_eq!(read(&mut client, &resume_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let replay_publish = client.poll_write().expect("replayed publish expected");
    assert_eq!(replay_publish.len(), first_publish.len());
    assert_eq!(replay_publish[0], first_publish[0] | 0b0000_1000);
    assert_eq!(&replay_publish[1..], &first_publish[1..]);
}

#[test]
fn subscribe_tracks_packet_id_until_suback() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    assert_eq!(
        client.handle_write(subscribe_one(app_subscription("topic/a"))),
        Ok(())
    );
    let subscribe_frame = client.poll_write().expect("subscribe frame expected");

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("topic/pub", ProtoQos::AtLeastOnce, b"payload"),
        }),
        Ok(())
    );
    let publish_frame = client.poll_write().expect("publish frame expected");
    assert_ne!(publish_frame, subscribe_frame);

    let suback = ControlPacket::SubAck(
        SubAck::builder()
            .packet_id(NonZero::new(1).expect("non-zero"))
            .reason_codes(vec![SubAckReasonCode::SuccessQoS0])
            .build(),
    );
    assert_eq!(read(&mut client, &suback), Ok(()));
}

#[test]
fn unsubscribe_tracks_packet_id_until_unsuback() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    assert_eq!(client.handle_write(unsubscribe_one("topic/a")), Ok(()));
    let unsub_frame = client.poll_write().expect("unsubscribe frame expected");

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("topic/pub", ProtoQos::AtLeastOnce, b"payload"),
        }),
        Ok(())
    );
    let publish_frame = client.poll_write().expect("publish frame expected");
    assert_ne!(publish_frame, unsub_frame);

    let unsuback = ControlPacket::UnsubAck(
        UnsubAck::builder()
            .packet_id(NonZero::new(1).expect("non-zero"))
            .reason_codes(vec![UnsubAckReasonCode::Success])
            .build(),
    );
    assert_eq!(read(&mut client, &unsuback), Ok(()));
}

#[test]
fn unknown_suback_or_unsuback_is_protocol_error() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let suback = ControlPacket::SubAck(
        SubAck::builder()
            .packet_id(NonZero::new(123).expect("non-zero"))
            .reason_codes(vec![SubAckReasonCode::SuccessQoS0])
            .build(),
    );
    assert_eq!(read(&mut client, &suback), Err(Error::ProtocolError));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));

    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let unsuback = ControlPacket::UnsubAck(
        UnsubAck::builder()
            .packet_id(NonZero::new(123).expect("non-zero"))
            .reason_codes(vec![UnsubAckReasonCode::Success])
            .build(),
    );
    assert_eq!(read(&mut client, &unsuback), Err(Error::ProtocolError));
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::CloseSocket)
    ));
}

/// [MQTT-4.13.0-1] When the server sends a DISCONNECT packet, the reason code
/// must be forwarded to the application via
/// `Event::Disconnected(Some(reason))`.
#[test]
fn server_disconnect_with_reason_code_forwarded_to_application() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let server_disconnect = ControlPacket::Disconnect(
        Disconnect::builder()
            .reason_code(DisconnectReasonCode::ServerBusy)
            .build(),
    );
    assert_eq!(read(&mut client, &server_disconnect), Ok(()));

    let event = client.poll_read();
    match event {
        Some(Event::Disconnected(Some(rc))) => {
            assert_eq!(
                rc,
                ReasonCode::ServerBusy,
                "reason code must match the server's DISCONNECT"
            );
        }
        other => panic!("expected Disconnected(Some(ServerBusy)), got {other:?}"),
    }
    assert!(
        matches!(client.poll_event(), Some(DriverAction::CloseSocket)),
        "CloseSocket must be emitted after server DISCONNECT"
    );
}

/// Normal server DISCONNECT (NormalDisconnection) is also forwarded.
#[test]
fn server_normal_disconnect_reason_code_forwarded() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let server_disconnect = ControlPacket::Disconnect(
        Disconnect::builder()
            .reason_code(DisconnectReasonCode::NormalDisconnection)
            .build(),
    );
    assert_eq!(read(&mut client, &server_disconnect), Ok(()));

    let event = client.poll_read();
    match event {
        Some(Event::Disconnected(Some(rc))) => {
            assert_eq!(rc, ReasonCode::NormalDisconnection);
        }
        other => panic!("expected Disconnected(Some(NormalDisconnection)), got {other:?}"),
    }
}

/// Client-initiated disconnect emits `Disconnected(None)` (no server reason
/// code).
#[test]
fn client_initiated_disconnect_emits_disconnected_none() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    assert_eq!(client.handle_write(Command::Disconnect), Ok(()));
    let event = client.poll_read();
    assert!(
        matches!(event, Some(Event::Disconnected(None))),
        "client-initiated disconnect must emit Disconnected(None), got {event:?}"
    );
}

/// [MQTT-4.12.0-2] [MQTT-4.12.0-4] When the server sends AUTH during an
/// established Connected session, it must be forwarded to the application as
/// `Event::Auth` rather than triggering a protocol error.
#[test]
fn auth_packet_in_connected_state_forwarded_to_application() {
    let mut client = Client::<Duration>::default();
    let _ = connect_default(&mut client);

    let auth_packet = ControlPacket::Auth(
        Auth::builder()
            .reason_code(AuthReasonCode::ReAuthenticate)
            .build(),
    );
    assert_eq!(
        read(&mut client, &auth_packet),
        Ok(()),
        "AUTH in Connected state must not return an error"
    );

    let event = client.poll_read();
    assert!(
        matches!(event, Some(Event::Auth { .. })),
        "AUTH packet must be forwarded as Event::Auth, got {event:?}"
    );
    assert!(client.poll_event().is_none(), "no action events expected");
    assert!(client.poll_read().is_none(), "no further read events");
}

/// Brings a `Client<Duration>` to the Connected state with the given
/// keep-alive (in seconds). The CONNACK is delivered with `received_at =
/// Duration::ZERO`, so when keep-alive is configured the timer is armed at
/// `Duration::from_secs(keep_alive)`.
fn make_connected_client_with_keep_alive(keep_alive_secs: Option<u16>) -> Client<Duration> {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        read(
            &mut client,
            &connack_with_properties(
                ConnAckProperties::builder()
                    .maybe_server_keep_alive(keep_alive_secs)
                    .build(),
            ),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
    client
}

#[test]
fn keep_alive_timer_armed_after_connack_when_keep_alive_configured() {
    // CONNACK received at t=0 with interval=30 → timer = 0 + 30 = 30.
    let mut client = make_connected_client_with_keep_alive(Some(30));
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(30)));
}

#[test]
fn keep_alive_timer_armed_after_connack_uses_received_at_timestamp() {
    let mut client = Client::<Duration>::default();
    let _ = open_connecting(&mut client, connect_options());

    assert_eq!(
        read_at(
            &mut client,
            &connack_with_properties(ConnAckProperties::builder().server_keep_alive(30).build(),),
            Duration::from_secs(100),
        ),
        Ok(())
    );
    assert!(matches!(client.poll_read(), Some(Event::Connected)));
    // Tr = received_at + interval = 100 + 30 = 130.
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(130)));
}

#[test]
fn keep_alive_timer_not_armed_when_no_keep_alive_configured() {
    let mut client = make_connected_client_with_keep_alive(None);
    assert_eq!(client.poll_timeout(), None);
}

/// An outgoing packet (any successful `handle_write`) sets the keep-alive
/// activity flag so that `handle_timeout` reschedules the deadline without
/// sending a PINGREQ.
#[test]
fn handle_timeout_reschedules_deadline_to_now_plus_interval_when_outgoing_packet_sent() {
    let interval_secs: u64 = 30;
    let mut client = make_connected_client_with_keep_alive(Some(interval_secs as u16));
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(30)));

    assert_eq!(
        client.handle_write(Command::Publish {
            token: 1,
            message: message("t", ProtoQos::AtMostOnce, b""),
        }),
        Ok(())
    );
    let _ = client.poll_write();

    assert_eq!(client.handle_timeout(Duration::from_secs(30)), Ok(()));
    assert_eq!(
        client.poll_write(),
        None,
        "no PINGREQ when outgoing packet was sent"
    );
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(60)));
}

/// [MQTT-3.1.2-24] The server MUST close the connection if it receives no
/// packet within 1.5× the keep-alive interval.
#[test]
fn handle_timeout_sends_pingreq_and_reschedules_at_half_interval_per_mqtt_3_1_2_24() {
    let interval_secs: u64 = 10;
    let mut client = make_connected_client_with_keep_alive(Some(interval_secs as u16));
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(10)));

    assert_eq!(client.handle_timeout(Duration::from_secs(10)), Ok(()));
    assert_eq!(
        client.poll_write(),
        Some(Bytes::from_static(&[0xC0, 0x00])),
        "PINGREQ must be sent"
    );
    assert_eq!(
        client.poll_timeout(),
        Some(Duration::from_secs(15)),
        "next deadline must be interval/2 after PINGREQ so total is 1.5× interval"
    );

    assert_eq!(
        client.handle_timeout(Duration::from_secs(15)),
        Err(Error::ProtocolError),
        "connection must be closed after 1.5× keep-alive interval without PINGRESP"
    );
}

/// PINGRESP received between PINGREQ and the half-interval deadline must clear
/// `ping_outstanding` so the connection is NOT closed at the half-interval
/// deadline.
#[test]
fn handle_timeout_pingresp_before_half_interval_deadline_resets_ping_outstanding() {
    let interval_secs: u64 = 10;
    let mut client = make_connected_client_with_keep_alive(Some(interval_secs as u16));
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(10)));

    assert_eq!(client.handle_timeout(Duration::from_secs(10)), Ok(()));
    assert!(client.poll_write().is_some(), "PINGREQ must be sent");
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(15)));

    let pingresp = ControlPacket::PingResp(sansio_mqtt_v5_types::PingResp {});
    assert_eq!(
        read_at(&mut client, &pingresp, Duration::from_secs(12)),
        Ok(())
    );
    assert_eq!(
        client.poll_timeout(),
        Some(Duration::from_secs(15)),
        "timer stays at half-interval deadline; incoming packets do not move it"
    );

    assert_eq!(
        client.handle_timeout(Duration::from_secs(15)),
        Ok(()),
        "connection must NOT close after PINGRESP was received"
    );
    assert!(
        client.poll_write().is_some(),
        "PINGREQ sent — no outgoing traffic"
    );
    assert_eq!(client.poll_timeout(), Some(Duration::from_secs(20)));
}

/// Regression test: `handle_event(SocketConnected)` must preserve
/// `pending_connect_options` in the scratchpad so that
/// `reset_negotiated_limits` → `recompute_effective_limits` reads the
/// user-supplied values rather than defaults.
#[test]
fn socket_connected_preserves_connect_options_for_effective_limit_recomputation() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        topic_alias_maximum: Some(10),
        ..Default::default()
    }));
    let _ = connect_default(&mut client);

    let alias = NonZero::new(5).expect("non-zero alias");
    let publish_with_alias = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"hello".as_slice()))
            .topic(wire_topic("test/topic"))
            .properties(PublishProperties::builder().topic_alias(alias).build())
            .build(),
    );

    assert_eq!(
        read(&mut client, &publish_with_alias),
        Ok(()),
        "PUBLISH with topic alias within configured limit must be accepted"
    );
    assert!(matches!(client.poll_read(), Some(Event::Message(_))));
}

/// Companion to
/// `socket_connected_preserves_connect_options_for_effective_limit_recomputation`:
/// covers the `Disconnected → SocketConnected` reconnect path.
#[test]
fn reconnect_from_disconnected_preserves_connect_options_for_effective_limit_recomputation() {
    let mut client = Client::<Duration>::new(settings(SettingsOverrides {
        topic_alias_maximum: Some(10),
        ..Default::default()
    }));
    let _ = connect_default(&mut client);

    assert_eq!(client.handle_event(DriverEvent::SocketClosed), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Disconnected(_))));

    assert_eq!(
        client.handle_write(Command::Connect(connect_options())),
        Ok(())
    );
    assert!(matches!(
        client.poll_event(),
        Some(DriverAction::OpenSocket)
    ));
    assert_eq!(client.handle_event(DriverEvent::SocketConnected), Ok(()));
    let _ = client
        .poll_write()
        .expect("reconnect CONNECT frame expected");
    assert_eq!(read(&mut client, &success_connack()), Ok(()));
    assert!(matches!(client.poll_read(), Some(Event::Connected)));

    let alias = NonZero::new(5).expect("non-zero alias");
    let publish_with_alias = ControlPacket::Publish(
        Publish::builder()
            .kind(PublishKind::FireAndForget)
            .payload(Payload::new(b"hello".as_slice()))
            .topic(wire_topic("test/topic"))
            .properties(PublishProperties::builder().topic_alias(alias).build())
            .build(),
    );

    assert_eq!(
        read(&mut client, &publish_with_alias),
        Ok(()),
        "PUBLISH with topic alias within configured limit must be accepted after reconnect"
    );
    assert!(matches!(client.poll_read(), Some(Event::Message(_))));
}
