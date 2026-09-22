//! Conversions between the version-neutral `sansio-mqtt-protocol` types and
//! the MQTT v5 wire types from `sansio-mqtt-v5-types`.
//!
//! String and binary fields are validated here (the version-neutral types
//! carry unvalidated `ByteString`/`Bytes`); a wire `Utf8String`/`Topic` is
//! known-valid, so the reverse direction never fails.

use alloc::vec::Vec;
use bytes::Bytes;
use bytestring::ByteString;
use core::time::Duration;

use crate::client::ClientSettings;

use sansio_mqtt_protocol::Authentication;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Message;
use sansio_mqtt_protocol::PayloadFormat as ProtocolPayloadFormat;
use sansio_mqtt_protocol::Qos as ProtocolQos;
use sansio_mqtt_protocol::ReasonCode;
use sansio_mqtt_protocol::RejectReason;
use sansio_mqtt_protocol::RetainHandling as ProtocolRetainHandling;
use sansio_mqtt_protocol::SubscribeOptions;
use sansio_mqtt_protocol::Subscription as ProtocolSubscription;
use sansio_mqtt_protocol::UnsubscribeOptions;
use sansio_mqtt_protocol::Will as ProtocolWill;

use sansio_mqtt_v5_types::Auth as WireAuth;
use sansio_mqtt_v5_types::AuthReasonCode;
use sansio_mqtt_v5_types::AuthenticationKind;
use sansio_mqtt_v5_types::BinaryData;
use sansio_mqtt_v5_types::Connect as WireConnect;
use sansio_mqtt_v5_types::ConnectProperties;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::FormatIndicator;
use sansio_mqtt_v5_types::GuaranteedQoS;
use sansio_mqtt_v5_types::Payload;
use sansio_mqtt_v5_types::PubAckReasonCode;
use sansio_mqtt_v5_types::PubCompReasonCode;
use sansio_mqtt_v5_types::PubRecReasonCode;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::PublishProperties;
use sansio_mqtt_v5_types::Qos as WireQos;
use sansio_mqtt_v5_types::RetainHandling as WireRetainHandling;
use sansio_mqtt_v5_types::SubscribeProperties;
use sansio_mqtt_v5_types::Subscription as WireSubscription;
use sansio_mqtt_v5_types::Topic;
use sansio_mqtt_v5_types::UnsubscribeProperties;
use sansio_mqtt_v5_types::Utf8String;
use sansio_mqtt_v5_types::Will as WireWill;
use sansio_mqtt_v5_types::WillProperties;

/// Converts an application byte string into a wire [`Utf8String`].
///
/// The [MQTT-1.5.4-1] length limit is checked first and reported as
/// [`Error::StringTooLong`]; any other validation failure (MQTT-disallowed
/// characters) is reported as [`Error::InvalidTopicFilter`]. `ByteString`
/// already guarantees well-formed UTF-8, so the character check is the only
/// other way [`Utf8String::try_new`] can fail here.
pub(crate) fn utf8_to_wire(value: ByteString) -> Result<Utf8String, Error> {
    if value.len() > u16::MAX as usize {
        return Err(Error::StringTooLong);
    }
    Utf8String::try_new(value).map_err(|_| Error::InvalidTopicFilter)
}

/// Converts an application byte string into a wire [`Topic`] (a Topic Name,
/// which additionally MUST NOT contain the `#`/`+` wildcard characters,
/// [MQTT-4.7.1-1], [MQTT-4.7.1-2]).
pub(crate) fn topic_to_wire(value: ByteString) -> Result<Topic, Error> {
    if value.len() > u16::MAX as usize {
        return Err(Error::StringTooLong);
    }
    let utf8 = Utf8String::try_new(value).map_err(|_| Error::InvalidTopicFilter)?;
    Topic::try_from(utf8).map_err(|_| Error::InvalidTopic)
}

/// Converts application bytes into wire [`BinaryData`], mapping the
/// [MQTT-1.5.6-1] length limit to [`Error::BinaryTooLong`].
pub(crate) fn binary_to_wire(value: Bytes) -> Result<BinaryData, Error> {
    BinaryData::try_new(value).map_err(|_| Error::BinaryTooLong)
}

/// Converts application bytes into a wire [`Payload`].
///
/// [`Payload::try_new`] only rejects payloads whose length exceeds
/// `u64::MAX`, which is unreachable for a [`bytes::Bytes`] on any supported
/// platform, so this conversion is infallible.
pub(crate) fn payload_to_wire(value: Bytes) -> Payload {
    Payload::new(value)
}

pub(crate) fn binary_from_wire(value: BinaryData) -> Bytes {
    value.into_inner()
}

pub(crate) fn payload_from_wire(value: Payload) -> Bytes {
    value.into_inner()
}

pub(crate) fn user_properties_to_wire(
    properties: Vec<(ByteString, ByteString)>,
) -> Result<Vec<(Utf8String, Utf8String)>, Error> {
    properties
        .into_iter()
        .map(|(key, value)| Ok((utf8_to_wire(key)?, utf8_to_wire(value)?)))
        .collect()
}

pub(crate) fn user_properties_from_wire(
    properties: Vec<(Utf8String, Utf8String)>,
) -> Vec<(ByteString, ByteString)> {
    properties
        .into_iter()
        .map(|(key, value)| (key.into_inner(), value.into_inner()))
        .collect()
}

pub(crate) fn qos_to_wire(qos: ProtocolQos) -> WireQos {
    match qos {
        ProtocolQos::AtMostOnce => WireQos::AtMostOnce,
        ProtocolQos::AtLeastOnce => WireQos::AtLeastOnce,
        ProtocolQos::ExactlyOnce => WireQos::ExactlyOnce,
    }
}

pub(crate) fn guaranteed_qos_from_wire(qos: GuaranteedQoS) -> ProtocolQos {
    match qos {
        GuaranteedQoS::AtLeastOnce => ProtocolQos::AtLeastOnce,
        GuaranteedQoS::ExactlyOnce => ProtocolQos::ExactlyOnce,
    }
}

fn retain_handling_to_wire(value: ProtocolRetainHandling) -> WireRetainHandling {
    match value {
        ProtocolRetainHandling::SendRetained => WireRetainHandling::SendRetained,
        ProtocolRetainHandling::SendRetainedIfSubscriptionDoesNotExist => {
            WireRetainHandling::SendRetainedIfSubscriptionDoesNotExist
        }
        ProtocolRetainHandling::DoNotSend => WireRetainHandling::DoNotSend,
    }
}

fn payload_format_to_wire(value: ProtocolPayloadFormat) -> FormatIndicator {
    match value {
        ProtocolPayloadFormat::Unspecified => FormatIndicator::Unspecified,
        ProtocolPayloadFormat::Utf8 => FormatIndicator::Utf8,
    }
}

fn payload_format_from_wire(value: FormatIndicator) -> ProtocolPayloadFormat {
    match value {
        FormatIndicator::Unspecified => ProtocolPayloadFormat::Unspecified,
        FormatIndicator::Utf8 => ProtocolPayloadFormat::Utf8,
    }
}

/// Converts a [`core::time::Duration`] into the whole-seconds `u32` MQTT
/// properties use, failing with [`Error::ProtocolError`] on overflow.
pub(crate) fn duration_to_secs_u32(value: Duration) -> Result<u32, Error> {
    u32::try_from(value.as_secs()).map_err(|_| Error::ProtocolError)
}

fn authentication_to_wire(auth: Authentication) -> Result<AuthenticationKind, Error> {
    let method = utf8_to_wire(auth.method)?;
    match auth.data {
        None => Ok(AuthenticationKind::WithoutData { method }),
        Some(data) => Ok(AuthenticationKind::WithData {
            method,
            data: binary_to_wire(data)?,
        }),
    }
}

fn will_to_wire(will: ProtocolWill) -> Result<WireWill, Error> {
    let delay = will.delay.map(duration_to_secs_u32).transpose()?;
    let message_expiry_interval = will.message_expiry.map(duration_to_secs_u32).transpose()?;
    let content_type = will.content_type.map(utf8_to_wire).transpose()?;
    let response_topic = will.response_topic.map(topic_to_wire).transpose()?;
    let correlation_data = will.correlation_data.map(binary_to_wire).transpose()?;
    let user_properties = user_properties_to_wire(will.user_properties)?;
    let payload_format = will.payload_format.map(payload_format_to_wire);
    let qos = qos_to_wire(will.qos);
    let retain = will.retain;
    let topic = topic_to_wire(will.topic)?;
    let payload = binary_to_wire(will.payload)?;

    Ok(WireWill::builder()
        .topic(topic)
        .payload(payload)
        .qos(qos)
        .retain(retain)
        .properties(
            WillProperties::builder()
                .maybe_will_delay_interval(delay)
                .maybe_payload_format_indicator(payload_format)
                .maybe_message_expiry_interval(message_expiry_interval)
                .maybe_content_type(content_type)
                .maybe_response_topic(response_topic)
                .maybe_correlation_data(correlation_data)
                .user_properties(user_properties)
                .build(),
        )
        .build())
}

/// Builds the CONNECT packet from [`ClientSettings`] and
/// [`sansio_mqtt_protocol::ConnectOptions`].
///
/// The per-connect properties that MQTT v5 negotiates once per connection
/// (Receive Maximum, Maximum Packet Size, Topic Alias Maximum, Request
/// Response/Problem Information) come from `settings` alone: they are no
/// longer overridable per `Command::Connect`.
pub(crate) fn connect_options_to_wire(
    settings: &ClientSettings,
    options: &ConnectOptions,
) -> Result<WireConnect, Error> {
    let will = options.will.clone().map(will_to_wire).transpose()?;
    let client_identifier = utf8_to_wire(options.client_id.clone())?;
    let user_name = options.user_name.clone().map(utf8_to_wire).transpose()?;
    let password = options.password.clone().map(binary_to_wire).transpose()?;
    let session_expiry_interval = options
        .session_expiry
        .map(duration_to_secs_u32)
        .transpose()?;
    let authentication = options
        .authentication
        .clone()
        .map(authentication_to_wire)
        .transpose()?;
    let user_properties = user_properties_to_wire(options.user_properties.clone())?;

    Ok(WireConnect::builder()
        .protocol_name(
            Utf8String::try_from("MQTT")
                .expect("MQTT protocol name is always a valid MQTT UTF-8 string"),
        )
        .protocol_version(5)
        .clean_start(options.clean_start)
        .client_identifier(client_identifier)
        .maybe_will(will)
        .maybe_user_name(user_name)
        .maybe_password(password)
        .maybe_keep_alive(options.keep_alive.or(settings.keep_alive))
        .properties(
            ConnectProperties::builder()
                .maybe_session_expiry_interval(session_expiry_interval)
                .maybe_receive_maximum(settings.receive_maximum)
                .maybe_maximum_packet_size(settings.maximum_packet_size)
                .maybe_topic_alias_maximum(settings.topic_alias_maximum)
                .maybe_request_response_information(settings.request_response_information)
                .maybe_request_problem_information(settings.request_problem_information)
                .maybe_authentication(authentication)
                .user_properties(user_properties)
                .build(),
        )
        .build())
}

/// Builds the properties section of an outbound PUBLISH from a [`Message`].
///
/// Outbound PUBLISH never carries a Topic Alias: the client always sends the
/// full Topic Name, so `Message` (unlike the wire `Publish`) has no
/// `topic_alias` field to read here.
pub(crate) fn message_properties_to_wire(msg: &Message) -> Result<PublishProperties, Error> {
    Ok(PublishProperties::builder()
        .maybe_payload_format_indicator(msg.payload_format.map(payload_format_to_wire))
        .maybe_message_expiry_interval(msg.message_expiry.map(duration_to_secs_u32).transpose()?)
        .maybe_response_topic(msg.response_topic.clone().map(topic_to_wire).transpose()?)
        .maybe_correlation_data(
            msg.correlation_data
                .clone()
                .map(binary_to_wire)
                .transpose()?,
        )
        .user_properties(user_properties_to_wire(msg.user_properties.clone())?)
        .maybe_content_type(msg.content_type.clone().map(utf8_to_wire).transpose()?)
        .build())
}

/// Converts an inbound wire [`Publish`] into a [`Message`] delivered to the
/// application.
pub(crate) fn publish_to_message(publish: Publish) -> Message {
    let qos = match &publish.kind {
        PublishKind::FireAndForget => ProtocolQos::AtMostOnce,
        PublishKind::Repetible { qos, .. } => guaranteed_qos_from_wire(*qos),
    };
    let retain = publish.retain;
    let properties = publish.properties;
    let topic = publish.topic.into_inner().into_inner();
    let payload = payload_from_wire(publish.payload);

    Message::builder()
        .topic(topic)
        .payload(payload)
        .qos(qos)
        .retain(retain)
        .maybe_payload_format(
            properties
                .payload_format_indicator
                .map(payload_format_from_wire),
        )
        .maybe_message_expiry(
            properties
                .message_expiry_interval
                .map(|secs| Duration::from_secs(u64::from(secs))),
        )
        .maybe_response_topic(
            properties
                .response_topic
                .map(|topic| topic.into_inner().into_inner()),
        )
        .maybe_correlation_data(properties.correlation_data.map(binary_from_wire))
        .maybe_content_type(properties.content_type.map(Utf8String::into_inner))
        .user_properties(user_properties_from_wire(properties.user_properties))
        .subscription_identifiers(properties.subscription_identifiers)
        .build()
}

fn subscription_to_wire(subscription: ProtocolSubscription) -> Result<WireSubscription, Error> {
    Ok(WireSubscription::builder()
        .topic_filter(utf8_to_wire(subscription.filter)?)
        .qos(qos_to_wire(subscription.qos))
        .no_local(subscription.no_local)
        .retain_as_published(subscription.retain_as_published)
        .retain_handling(retain_handling_to_wire(subscription.retain_handling))
        .build())
}

/// Splits a [`SubscribeOptions`] into the wire `Subscribe` packet's parts:
/// the first subscription, the rest, and the properties.
///
/// [MQTT-3.8.3-3] A SUBSCRIBE MUST carry at least one subscription; an empty
/// list is reported as [`Error::EmptySubscribe`].
pub(crate) fn subscribe_options_to_wire(
    options: SubscribeOptions,
) -> Result<(WireSubscription, Vec<WireSubscription>, SubscribeProperties), Error> {
    let identifier = options.identifier;
    let user_properties = user_properties_to_wire(options.user_properties)?;
    let mut subscriptions = options.subscriptions.into_iter();
    let first = subscription_to_wire(subscriptions.next().ok_or(Error::EmptySubscribe)?)?;
    let extra = subscriptions
        .map(subscription_to_wire)
        .collect::<Result<Vec<_>, _>>()?;

    let properties = SubscribeProperties::builder()
        .maybe_subscription_identifier(identifier)
        .user_properties(user_properties)
        .build();

    Ok((first, extra, properties))
}

/// Splits an [`UnsubscribeOptions`] into the wire `Unsubscribe` packet's
/// parts: the first filter, the rest, and the properties.
///
/// [MQTT-3.10.3-1] An UNSUBSCRIBE MUST carry at least one Topic Filter; an
/// empty list is reported as [`Error::EmptyUnsubscribe`].
pub(crate) fn unsubscribe_options_to_wire(
    options: UnsubscribeOptions,
) -> Result<(Utf8String, Vec<Utf8String>, UnsubscribeProperties), Error> {
    let user_properties = user_properties_to_wire(options.user_properties)?;
    let mut filters = options.filters.into_iter();
    let first = utf8_to_wire(filters.next().ok_or(Error::EmptyUnsubscribe)?)?;
    let extra = filters.map(utf8_to_wire).collect::<Result<Vec<_>, _>>()?;

    let properties = UnsubscribeProperties::builder()
        .user_properties(user_properties)
        .build();

    Ok((first, extra, properties))
}

/// Converts an app-level rejection reason into the Reason Code a PUBACK may
/// carry.
pub(crate) fn reject_reason_to_puback(reason: RejectReason) -> PubAckReasonCode {
    match reason {
        RejectReason::UnspecifiedError => PubAckReasonCode::UnspecifiedError,
        RejectReason::ImplementationSpecificError => PubAckReasonCode::ImplementationSpecificError,
        RejectReason::NotAuthorized => PubAckReasonCode::NotAuthorized,
        RejectReason::TopicNameInvalid => PubAckReasonCode::TopicNameInvalid,
        RejectReason::QuotaExceeded => PubAckReasonCode::QuotaExceeded,
        RejectReason::PayloadFormatInvalid => PubAckReasonCode::PayloadFormatInvalid,
        _ => PubAckReasonCode::UnspecifiedError,
    }
}

/// Converts an app-level rejection reason into the Reason Code a PUBREC may
/// carry.
pub(crate) fn reject_reason_to_pubrec(reason: RejectReason) -> PubRecReasonCode {
    match reason {
        RejectReason::UnspecifiedError => PubRecReasonCode::UnspecifiedError,
        RejectReason::ImplementationSpecificError => PubRecReasonCode::ImplementationSpecificError,
        RejectReason::NotAuthorized => PubRecReasonCode::NotAuthorized,
        RejectReason::TopicNameInvalid => PubRecReasonCode::TopicNameInvalid,
        RejectReason::QuotaExceeded => PubRecReasonCode::QuotaExceeded,
        RejectReason::PayloadFormatInvalid => PubRecReasonCode::PayloadFormatInvalid,
        _ => PubRecReasonCode::UnspecifiedError,
    }
}

pub(crate) fn puback_reason_to_protocol(code: PubAckReasonCode) -> ReasonCode {
    match code {
        PubAckReasonCode::Success => ReasonCode::Success,
        PubAckReasonCode::NoMatchingSubscribers => ReasonCode::NoMatchingSubscribers,
        PubAckReasonCode::UnspecifiedError => ReasonCode::UnspecifiedError,
        PubAckReasonCode::ImplementationSpecificError => ReasonCode::ImplementationSpecificError,
        PubAckReasonCode::NotAuthorized => ReasonCode::NotAuthorized,
        PubAckReasonCode::TopicNameInvalid => ReasonCode::TopicNameInvalid,
        PubAckReasonCode::PacketIdentifierInUse => ReasonCode::PacketIdentifierInUse,
        PubAckReasonCode::QuotaExceeded => ReasonCode::QuotaExceeded,
        PubAckReasonCode::PayloadFormatInvalid => ReasonCode::PayloadFormatInvalid,
        _ => ReasonCode::UnspecifiedError,
    }
}

pub(crate) fn pubrec_reason_to_protocol(code: PubRecReasonCode) -> ReasonCode {
    match code {
        PubRecReasonCode::Success => ReasonCode::Success,
        PubRecReasonCode::NoMatchingSubscribers => ReasonCode::NoMatchingSubscribers,
        PubRecReasonCode::UnspecifiedError => ReasonCode::UnspecifiedError,
        PubRecReasonCode::ImplementationSpecificError => ReasonCode::ImplementationSpecificError,
        PubRecReasonCode::NotAuthorized => ReasonCode::NotAuthorized,
        PubRecReasonCode::TopicNameInvalid => ReasonCode::TopicNameInvalid,
        PubRecReasonCode::PacketIdentifierInUse => ReasonCode::PacketIdentifierInUse,
        PubRecReasonCode::QuotaExceeded => ReasonCode::QuotaExceeded,
        PubRecReasonCode::PayloadFormatInvalid => ReasonCode::PayloadFormatInvalid,
        _ => ReasonCode::UnspecifiedError,
    }
}

pub(crate) fn pubcomp_reason_to_protocol(code: PubCompReasonCode) -> ReasonCode {
    match code {
        PubCompReasonCode::Success => ReasonCode::Success,
        PubCompReasonCode::PacketIdentifierNotFound => ReasonCode::PacketIdentifierNotFound,
        _ => ReasonCode::UnspecifiedError,
    }
}

pub(crate) fn disconnect_reason_to_protocol(code: DisconnectReasonCode) -> ReasonCode {
    match code {
        DisconnectReasonCode::NormalDisconnection => ReasonCode::NormalDisconnection,
        DisconnectReasonCode::DisconnectWithWillMessage => ReasonCode::DisconnectWithWillMessage,
        DisconnectReasonCode::UnspecifiedError => ReasonCode::UnspecifiedError,
        DisconnectReasonCode::MalformedPacket => ReasonCode::MalformedPacket,
        DisconnectReasonCode::ProtocolError => ReasonCode::ProtocolError,
        DisconnectReasonCode::ImplementationSpecificError => {
            ReasonCode::ImplementationSpecificError
        }
        DisconnectReasonCode::UnsupportedProtocolVersion => ReasonCode::UnsupportedProtocolVersion,
        DisconnectReasonCode::ClientIdentifierNotValid => ReasonCode::ClientIdentifierNotValid,
        DisconnectReasonCode::BadUserNameOrPassword => ReasonCode::BadUserNameOrPassword,
        DisconnectReasonCode::NotAuthorized => ReasonCode::NotAuthorized,
        DisconnectReasonCode::ServerUnavailable => ReasonCode::ServerUnavailable,
        DisconnectReasonCode::ServerBusy => ReasonCode::ServerBusy,
        DisconnectReasonCode::Banned => ReasonCode::Banned,
        DisconnectReasonCode::BadAuthenticationMethod => ReasonCode::BadAuthenticationMethod,
        DisconnectReasonCode::ServerShuttingDown => ReasonCode::ServerShuttingDown,
        DisconnectReasonCode::KeepAliveTimeout => ReasonCode::KeepAliveTimeout,
        DisconnectReasonCode::SessionTakenOver => ReasonCode::SessionTakenOver,
        DisconnectReasonCode::TopicFilterInvalid => ReasonCode::TopicFilterInvalid,
        DisconnectReasonCode::PacketIdentifierInUse => ReasonCode::PacketIdentifierInUse,
        DisconnectReasonCode::PacketIdentifierNotFound => ReasonCode::PacketIdentifierNotFound,
        DisconnectReasonCode::ReceiveMaximumExceeded => ReasonCode::ReceiveMaximumExceeded,
        DisconnectReasonCode::TopicAliasInvalid => ReasonCode::TopicAliasInvalid,
        DisconnectReasonCode::PacketTooLarge => ReasonCode::PacketTooLarge,
        DisconnectReasonCode::MessageRateTooHigh => ReasonCode::MessageRateTooHigh,
        DisconnectReasonCode::AdministrativeAction => ReasonCode::AdministrativeAction,
        DisconnectReasonCode::PayloadFormatInvalid => ReasonCode::PayloadFormatInvalid,
        DisconnectReasonCode::RetainNotSupported => ReasonCode::RetainNotSupported,
        DisconnectReasonCode::QoSNotSupported => ReasonCode::QoSNotSupported,
        DisconnectReasonCode::UseAnotherServer => ReasonCode::UseAnotherServer,
        DisconnectReasonCode::ServerMoved => ReasonCode::ServerMoved,
        DisconnectReasonCode::SharedSubscriptionsNotSupported => {
            ReasonCode::SharedSubscriptionsNotSupported
        }
        DisconnectReasonCode::ConnectionRateExceeded => ReasonCode::ConnectionRateExceeded,
        DisconnectReasonCode::MaximumConnectTime => ReasonCode::MaximumConnectTime,
        DisconnectReasonCode::SubscriptionIdentifiersNotSupported => {
            ReasonCode::SubscriptionIdentifiersNotSupported
        }
        DisconnectReasonCode::WildcardSubscriptionsNotSupported => {
            ReasonCode::WildcardSubscriptionsNotSupported
        }
        _ => ReasonCode::UnspecifiedError,
    }
}

fn auth_reason_code_to_protocol(code: AuthReasonCode) -> ReasonCode {
    match code {
        AuthReasonCode::Success => ReasonCode::Success,
        AuthReasonCode::ContinueAuthentication => ReasonCode::ContinueAuthentication,
        AuthReasonCode::ReAuthenticate => ReasonCode::ReAuthenticate,
        _ => ReasonCode::UnspecifiedError,
    }
}

/// Converts an inbound AUTH packet into `Event::Auth`.
///
/// [MQTT-3.15.2.2.2] Authentication Method is normally present, but the wire
/// type allows it to be absent; when it is, `method` is the empty string
/// rather than requiring the caller to track the method negotiated earlier.
pub(crate) fn auth_packet_to_event(auth: WireAuth) -> Event {
    let reason = auth_reason_code_to_protocol(auth.reason_code);
    let (method, data) = match auth.properties.authentication {
        Some(AuthenticationKind::WithoutData { method }) => (method.into_inner(), None),
        Some(AuthenticationKind::WithData { method, data }) => {
            (method.into_inner(), Some(binary_from_wire(data)))
        }
        None | Some(_) => (ByteString::new(), None),
    };

    Event::Auth {
        reason,
        method,
        data,
        reason_string: auth.properties.reason_string.map(Utf8String::into_inner),
        user_properties: user_properties_from_wire(auth.properties.user_properties),
    }
}
