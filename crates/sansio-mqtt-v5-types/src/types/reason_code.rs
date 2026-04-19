//! MQTT v5.0 Reason Codes
//! ([§2.4 — Reason Code](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901031)).
//!
//! A Reason Code is a one-byte unsigned value that indicates the
//! outcome of an operation. Each acknowledgement and control packet
//! restricts the set of Reason Codes it may carry; the enums below
//! model those per-packet subsets.
use super::*;

/// Reason Code sent by a Server in `CONNACK` to summarise a `CONNECT`
/// attempt ([§3.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901079)).
///
/// Semantically equivalent to [`ConnackReasonCode`] and preserved as a
/// distinct type so clients can match on the precise packet they
/// originate from. Conformance: `[MQTT-3.2.2-7]`, `[MQTT-3.2.2-8]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum ConnectReasonCode {
    /// `0x00` — The Connection is accepted.
    #[default]
    Success = 0x00,
    /// `0x80` — The Server does not wish to reveal the reason for the
    /// failure, or none of the other Reason Codes apply.
    UnspecifiedError = 0x80,
    /// `0x81` — Data within the `CONNECT` packet could not be correctly
    /// parsed.
    MalformedPacket = 0x81,
    /// `0x82` — Data in the `CONNECT` packet does not conform to this
    /// specification.
    ProtocolError = 0x82,
    /// `0x83` — The `CONNECT` is valid but is not accepted by this
    /// Server.
    ImplementationSpecificError = 0x83,
    /// `0x84` — The Server does not support the protocol version
    /// requested by the Client.
    UnsupportedProtocolVersion = 0x84,
    /// `0x85` — The Client Identifier is a valid string but is not
    /// allowed by the Server.
    ClientIdentifierNotValid = 0x85,
    /// `0x86` — The Server does not accept the User Name or Password
    /// specified by the Client.
    BadUserNameOrPassword = 0x86,
    /// `0x87` — The Client is not authorized to connect.
    NotAuthorized = 0x87,
    /// `0x88` — The MQTT Server is not available.
    ServerUnavailable = 0x88,
    /// `0x89` — The Server is busy. Try again later.
    ServerBusy = 0x89,
    /// `0x8A` — This Client has been banned by administrative action.
    Banned = 0x8A,
    /// `0x8C` — The authentication method is not supported or does
    /// not match the authentication method currently in use.
    BadAuthenticationMethod = 0x8C,
    /// `0x90` — The Will Topic Name is not malformed, but is not
    /// accepted by this Server.
    TopicNameInvalid = 0x90,
    /// `0x95` — The `CONNECT` packet exceeded the maximum permissible
    /// size.
    PacketTooLarge = 0x95,
    /// `0x97` — An implementation or administrative-imposed limit has
    /// been exceeded.
    QuotaExceeded = 0x97,
    /// `0x99` — The Will Payload does not match the specified Payload
    /// Format Indicator.
    PayloadFormatInvalid = 0x99,
    /// `0x9A` — The Server does not support retained messages, and
    /// Will Retain was set to 1.
    RetainNotSupported = 0x9A,
    /// `0x9B` — The Server does not support the QoS set in Will QoS.
    QoSNotSupported = 0x9B,
    /// `0x9C` — The Client should temporarily use another Server.
    UseAnotherServer = 0x9C,
    /// `0x9D` — The Client should permanently use another Server.
    ServerMoved = 0x9D,
    /// `0x9F` — The connection rate limit has been exceeded.
    ConnectionRateExceeded = 0x9F,
}

/// Reason Code sent by the Server in `CONNACK`
/// ([§3.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901079)).
///
/// Same value space as [`ConnectReasonCode`] but associated with the
/// server's `CONNACK` response rather than the client's `CONNECT`.
/// Conformance: `[MQTT-3.2.2-7]`, `[MQTT-3.2.2-8]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum ConnackReasonCode {
    /// `0x00` — The Connection is accepted.
    #[default]
    Success = 0x00,
    /// `0x80` — The Server does not wish to reveal the reason for the
    /// failure, or none of the other Reason Codes apply.
    UnspecifiedError = 0x80,
    /// `0x81` — Data within the `CONNECT` packet could not be correctly
    /// parsed.
    MalformedPacket = 0x81,
    /// `0x82` — Data in the `CONNECT` packet does not conform to this
    /// specification.
    ProtocolError = 0x82,
    /// `0x83` — The `CONNECT` is valid but is not accepted by this
    /// Server.
    ImplementationSpecificError = 0x83,
    /// `0x84` — The Server does not support the protocol version
    /// requested by the Client.
    UnsupportedProtocolVersion = 0x84,
    /// `0x85` — The Client Identifier is a valid string but is not
    /// allowed by the Server.
    ClientIdentifierNotValid = 0x85,
    /// `0x86` — The Server does not accept the User Name or Password
    /// specified by the Client.
    BadUserNameOrPassword = 0x86,
    /// `0x87` — The Client is not authorized to connect.
    NotAuthorized = 0x87,
    /// `0x88` — The MQTT Server is not available.
    ServerUnavailable = 0x88,
    /// `0x89` — The Server is busy. Try again later.
    ServerBusy = 0x89,
    /// `0x8A` — This Client has been banned by administrative action.
    Banned = 0x8A,
    /// `0x8C` — The authentication method is not supported or does
    /// not match the authentication method currently in use.
    BadAuthenticationMethod = 0x8C,
    /// `0x90` — The Will Topic Name is not malformed, but is not
    /// accepted by this Server.
    TopicNameInvalid = 0x90,
    /// `0x95` — The `CONNECT` packet exceeded the maximum permissible
    /// size.
    PacketTooLarge = 0x95,
    /// `0x97` — An implementation or administrative-imposed limit has
    /// been exceeded.
    QuotaExceeded = 0x97,
    /// `0x99` — The Will Payload does not match the specified Payload
    /// Format Indicator.
    PayloadFormatInvalid = 0x99,
    /// `0x9A` — The Server does not support retained messages, and
    /// Will Retain was set to 1.
    RetainNotSupported = 0x9A,
    /// `0x9B` — The Server does not support the QoS set in Will QoS.
    QoSNotSupported = 0x9B,
    /// `0x9C` — The Client should temporarily use another Server.
    UseAnotherServer = 0x9C,
    /// `0x9D` — The Client should permanently use another Server.
    ServerMoved = 0x9D,
    /// `0x9F` — The connection rate limit has been exceeded.
    ConnectionRateExceeded = 0x9F,
}

/// Reason Code carried by a `PUBLISH` packet's acknowledgement flow
/// when delivery was rejected — shared value space across the
/// PUBACK/PUBREC path
/// ([§3.4.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901124)).
///
/// Retained for symmetry with the per-packet Reason Code types; most
/// callers will interact with [`PubAckReasonCode`] or
/// [`PubRecReasonCode`] instead.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum PublishReasonCode {
    /// `0x00` — The message is accepted. Publication of the QoS 1
    /// message proceeds.
    #[default]
    Success = 0x00,
    /// `0x10` — The message is accepted but there are no subscribers.
    NoMatchingSubscribers = 0x10,
    /// `0x80` — The receiver does not accept the publish but either
    /// does not want to reveal the reason, or it does not match one of
    /// the other values.
    UnspecifiedError = 0x80,
    /// `0x83` — The `PUBLISH` is valid but the receiver is not willing
    /// to accept it.
    ImplementationSpecificError = 0x83,
    /// `0x87` — The `PUBLISH` is not authorized.
    NotAuthorized = 0x87,
    /// `0x90` — The Topic Name is not malformed, but is not accepted.
    TopicNameInvalid = 0x90,
    /// `0x91` — The Packet Identifier is already in use.
    PacketIdentifierInUse = 0x91,
    /// `0x97` — An implementation or administrative imposed limit has
    /// been exceeded.
    QuotaExceeded = 0x97,
    /// `0x99` — The payload format does not match the specified Payload
    /// Format Indicator.
    PayloadFormatInvalid = 0x99,
}

/// Reason Code carried by a `PUBACK` packet
/// ([§3.4.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901124)).
///
/// Sent by the receiver of a QoS 1 `PUBLISH` to acknowledge receipt
/// (`Success`) or reject it with one of the error codes. Conformance:
/// `[MQTT-3.4.2-1]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum PubAckReasonCode {
    /// `0x00` — The message is accepted. Publication of the QoS 1
    /// message proceeds.
    #[default]
    Success = 0x00,
    /// `0x10` — The message is accepted but there are no subscribers.
    NoMatchingSubscribers = 0x10,
    /// `0x80` — The receiver does not accept the publish but either
    /// does not want to reveal the reason, or none of the other values
    /// apply.
    UnspecifiedError = 0x80,
    /// `0x83` — The `PUBLISH` is valid but the receiver is not willing
    /// to accept it.
    ImplementationSpecificError = 0x83,
    /// `0x87` — The `PUBLISH` is not authorized.
    NotAuthorized = 0x87,
    /// `0x90` — The Topic Name is not malformed, but is not accepted.
    TopicNameInvalid = 0x90,
    /// `0x91` — The Packet Identifier is already in use.
    PacketIdentifierInUse = 0x91,
    /// `0x97` — An implementation or administrative imposed limit has
    /// been exceeded.
    QuotaExceeded = 0x97,
    /// `0x99` — The payload format does not match the specified Payload
    /// Format Indicator.
    PayloadFormatInvalid = 0x99,
}

/// Reason Code carried by a `PUBREC` packet
/// ([§3.5.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901134)).
///
/// Sent by the receiver of a QoS 2 `PUBLISH` as the first
/// acknowledgement of the four-packet flow. Conformance:
/// `[MQTT-3.5.2-1]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum PubRecReasonCode {
    /// `0x00` — The message is accepted. Publication of the QoS 2
    /// message proceeds.
    #[default]
    Success = 0x00,
    /// `0x10` — The message is accepted but there are no subscribers.
    NoMatchingSubscribers = 0x10,
    /// `0x80` — The receiver does not accept the publish but either
    /// does not want to reveal the reason, or none of the other values
    /// apply.
    UnspecifiedError = 0x80,
    /// `0x83` — The `PUBLISH` is valid but the receiver is not willing
    /// to accept it.
    ImplementationSpecificError = 0x83,
    /// `0x87` — The `PUBLISH` is not authorized.
    NotAuthorized = 0x87,
    /// `0x90` — The Topic Name is not malformed, but is not accepted.
    TopicNameInvalid = 0x90,
    /// `0x91` — The Packet Identifier is already in use.
    PacketIdentifierInUse = 0x91,
    /// `0x97` — An implementation or administrative imposed limit has
    /// been exceeded.
    QuotaExceeded = 0x97,
    /// `0x99` — The payload format does not match the specified Payload
    /// Format Indicator.
    PayloadFormatInvalid = 0x99,
}

/// Reason Code carried by a `PUBREL` packet
/// ([§3.6.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901144)).
///
/// Third packet of the QoS 2 flow. The sender can only indicate
/// `Success` or that the acknowledged Packet Identifier was not
/// recognised. Conformance: `[MQTT-3.6.2-1]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum PubRelReasonCode {
    /// `0x00` — The message is released.
    #[default]
    Success = 0x00,
    /// `0x92` — The Packet Identifier is not known. This is not an
    /// error during recovery, but at other times indicates a mismatch
    /// between the Session State on the Client and Server.
    PacketIdentifierNotFound = 0x92,
}

/// Reason Code carried by a `PUBCOMP` packet
/// ([§3.7.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901154)).
///
/// Fourth and final packet of the QoS 2 flow. Conformance:
/// `[MQTT-3.7.2-1]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum PubCompReasonCode {
    /// `0x00` — The packet identifier is released; the flow is
    /// complete.
    #[default]
    Success = 0x00,
    /// `0x92` — The Packet Identifier is not known.
    PacketIdentifierNotFound = 0x92,
}

/// Reason Code carried by a `SUBACK` packet, one per subscription
/// ([§3.9.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901174)).
///
/// Each `SUBACK` payload byte corresponds to one Topic Filter in the
/// matching `SUBSCRIBE`, and indicates either the maximum QoS granted
/// or a failure reason. Conformance: `[MQTT-3.9.3-1]`,
/// `[MQTT-3.9.3-2]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, EnumIter, Display)]
pub enum SubAckReasonCode {
    /// `0x00` — Subscription accepted; maximum granted QoS is 0.
    SuccessQoS0 = 0x00,
    /// `0x01` — Subscription accepted; maximum granted QoS is 1.
    SuccessQoS1 = 0x01,
    /// `0x02` — Subscription accepted; maximum granted QoS is 2.
    SuccessQoS2 = 0x02,
    /// `0x11` — No matching existing subscription to update.
    NoSubscriptionExisted = 0x11,
    /// `0x80` — The subscription is not accepted and the Server either
    /// does not wish to reveal the reason or none of the other Reason
    /// Codes apply.
    UnspecifiedError = 0x80,
    /// `0x83` — The `SUBSCRIBE` is valid but the Server does not
    /// accept it.
    ImplementationSpecificError = 0x83,
    /// `0x87` — The Client is not authorized to make this subscription.
    NotAuthorized = 0x87,
    /// `0x8F` — The Topic Filter is correctly formed but is not allowed
    /// for this Client.
    TopicFilterInvalid = 0x8F,
    /// `0x91` — The specified Packet Identifier is already in use.
    PacketIdentifierInUse = 0x91,
    /// `0x97` — An implementation or administrative imposed limit has
    /// been exceeded.
    QuotaExceeded = 0x97,
    /// `0x99` — The payload format does not match the specified Payload
    /// Format Indicator.
    PayloadFormatInvalid = 0x99,
    /// `0x9A` — The Server does not support retained messages.
    RetainNotSupported = 0x9A,
    /// `0x9B` — The Server does not support the QoS.
    QoSNotSupported = 0x9B,
    /// `0x9C` — The Client should temporarily use another Server.
    UseAnotherServer = 0x9C,
    /// `0x9D` — The Client should permanently use another Server.
    ServerMoved = 0x9D,
    /// `0x9F` — The connection rate limit has been exceeded.
    ConnectionRateExceeded = 0x9F,
    /// `0xA1` — The Server does not support Subscription Identifiers.
    SubscriptionIdentifiersNotSupported = 0xA1,
    /// `0xA2` — The Server does not support Wildcard Subscriptions.
    WildcardSubscriptionsNotSupported = 0xA2,
}

/// Reason Code carried by an `UNSUBACK` packet, one per unsubscribed
/// Topic Filter
/// ([§3.11.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901194)).
///
/// Conformance: `[MQTT-3.11.3-1]`, `[MQTT-3.11.3-2]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum UnsubAckReasonCode {
    /// `0x00` — The subscription is deleted.
    #[default]
    Success = 0x00,
    /// `0x11` — No matching Topic Filter is being used by the Client.
    NoSubscriptionExisted = 0x11,
    /// `0x80` — The `UNSUBSCRIBE` could not be completed and the
    /// Server either does not wish to reveal the reason or none of the
    /// other Reason Codes apply.
    UnspecifiedError = 0x80,
    /// `0x83` — The `UNSUBSCRIBE` is valid but the Server does not
    /// accept it.
    ImplementationSpecificError = 0x83,
    /// `0x87` — The Client is not authorized to unsubscribe.
    NotAuthorized = 0x87,
    /// `0x8F` — The Topic Filter is correctly formed but is not allowed
    /// for this Client.
    TopicFilterInvalid = 0x8F,
    /// `0x91` — The specified Packet Identifier is already in use.
    PacketIdentifierInUse = 0x91,
    /// `0x97` — An implementation or administrative imposed limit has
    /// been exceeded.
    QuotaExceeded = 0x97,
    /// `0x99` — The payload format does not match the specified Payload
    /// Format Indicator.
    PayloadFormatInvalid = 0x99,
}

/// Reason Code carried by a `DISCONNECT` packet
/// ([§3.14.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901208)).
///
/// Sent by either Client or Server to indicate the reason for closing
/// the Network Connection. Conformance: `[MQTT-3.14.2-1]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum DisconnectReasonCode {
    /// `0x00` — Close the connection normally. Do not send the Will
    /// Message.
    #[default]
    NormalDisconnection = 0x00,
    /// `0x04` — The Client wishes to disconnect but requires that the
    /// Server also publish its Will Message.
    DisconnectWithWillMessage = 0x04,
    /// `0x80` — The Connection is closed but the sender does not wish
    /// to reveal the reason, or none of the other Reason Codes apply.
    UnspecifiedError = 0x80,
    /// `0x81` — The received packet does not conform to this
    /// specification.
    MalformedPacket = 0x81,
    /// `0x82` — An unexpected or out-of-order packet was received.
    ProtocolError = 0x82,
    /// `0x83` — The packet received is valid but cannot be processed
    /// by this implementation.
    ImplementationSpecificError = 0x83,
    /// `0x84` — The Server does not support the protocol version
    /// used by the Client.
    UnsupportedProtocolVersion = 0x84,
    /// `0x85` — The Client Identifier is not valid.
    ClientIdentifierNotValid = 0x85,
    /// `0x86` — The Server does not accept the User Name or Password
    /// specified by the Client.
    BadUserNameOrPassword = 0x86,
    /// `0x87` — The request is not authorized.
    NotAuthorized = 0x87,
    /// `0x88` — The Server is not available.
    ServerUnavailable = 0x88,
    /// `0x89` — The Server is busy and cannot continue processing
    /// requests from this Client.
    ServerBusy = 0x89,
    /// `0x8A` — This Client has been banned by administrative action.
    Banned = 0x8A,
    /// `0x8C` — The authentication method is not supported or does
    /// not match the authentication method currently in use.
    BadAuthenticationMethod = 0x8C,
    /// `0x8B` — The Server is shutting down.
    ServerShuttingDown = 0x8B,
    /// `0x8D` — The Connection is closed because no packet has been
    /// received for 1.5 times the Keep Alive time.
    KeepAliveTimeout = 0x8D,
    /// `0x8E` — Another Connection using the same Client Identifier
    /// has connected and taken over this session.
    SessionTakenOver = 0x8E,
    /// `0x8F` — The Topic Filter is correctly formed, but is not
    /// accepted by this Server.
    TopicFilterInvalid = 0x8F,
    /// `0x91` — The Packet Identifier is already in use.
    PacketIdentifierInUse = 0x91,
    /// `0x92` — The Packet Identifier is not known.
    PacketIdentifierNotFound = 0x92,
    /// `0x93` — The Client or Server has received more than Receive
    /// Maximum publications for which it has not sent PUBACK or
    /// PUBCOMP.
    ReceiveMaximumExceeded = 0x93,
    /// `0x94` — The Client or Server has received a PUBLISH with a
    /// Topic Alias greater than the maximum Topic Alias it has sent.
    TopicAliasInvalid = 0x94,
    /// `0x95` — The packet size is greater than Maximum Packet Size
    /// advertised by the receiver.
    PacketTooLarge = 0x95,
    /// `0x96` — The received data rate is too high.
    MessageRateTooHigh = 0x96,
    /// `0x98` — An administrative action has terminated the Connection.
    AdministrativeAction = 0x98,
    /// `0x99` — The payload format does not match the Payload Format
    /// Indicator.
    PayloadFormatInvalid = 0x99,
    /// `0x9A` — The Server does not support retained messages and a
    /// retained message was received.
    RetainNotSupported = 0x9A,
    /// `0x9B` — The Server does not support the QoS requested.
    QoSNotSupported = 0x9B,
    /// `0x9C` — The Client should temporarily change its Server.
    UseAnotherServer = 0x9C,
    /// `0x9D` — The Client should permanently change its Server.
    ServerMoved = 0x9D,
    /// `0x9E` — The Server does not support Shared Subscriptions.
    SharedSubscriptionsNotSupported = 0x9E,
    /// `0x9F` — The connection rate limit has been exceeded.
    ConnectionRateExceeded = 0x9F,
    /// `0xA0` — The maximum connection time authorized for this
    /// connection has been exceeded.
    MaximumConnectTime = 0xA0,
    /// `0xA1` — The Server does not support Subscription Identifiers.
    SubscriptionIdentifiersNotSupported = 0xA1,
    /// `0xA2` — The Server does not support Wildcard Subscriptions.
    WildcardSubscriptionsNotSupported = 0xA2,
}

/// Reason Code carried by an `AUTH` packet
/// ([§3.15.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901220)).
///
/// Used to drive the enhanced authentication exchange introduced in
/// MQTT v5.0. Conformance: `[MQTT-3.15.2-1]`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default, EnumIter, Display)]
pub enum AuthReasonCode {
    /// `0x00` — Authentication is successful.
    #[default]
    Success = 0x00,
    /// `0x18` — Continue the authentication with another step.
    ContinueAuthentication = 0x18,
    /// `0x19` — Initiate a re-authentication.
    ReAuthenticate = 0x19,
}

/// Error returned when attempting to convert a byte into a Reason Code
/// variant that the spec does not define for the target packet type
/// ([§2.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901031)).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, thiserror::Error)]
#[error("Invalid reason code {value}")]
pub struct InvalidReasonCode {
    value: u8,
}

macro_rules! impl_reason_code {
    ($name:ty) => {
        impl From<$name> for u8 {
            #[inline]
            fn from(value: $name) -> Self {
                value as u8
            }
        }

        impl TryFrom<u8> for $name {
            type Error = InvalidReasonCode;

            #[inline]
            fn try_from(value: u8) -> Result<Self, Self::Error> {
                Self::iter()
                    .find(|v| *v as u8 == value)
                    .ok_or(InvalidReasonCode { value })
            }
        }
    };
}

impl_reason_code!(ConnectReasonCode);
impl_reason_code!(ConnackReasonCode);
impl_reason_code!(PublishReasonCode);
impl_reason_code!(PubAckReasonCode);
impl_reason_code!(PubRecReasonCode);
impl_reason_code!(PubRelReasonCode);
impl_reason_code!(PubCompReasonCode);
impl_reason_code!(SubAckReasonCode);
impl_reason_code!(UnsubAckReasonCode);
impl_reason_code!(DisconnectReasonCode);
impl_reason_code!(AuthReasonCode);
