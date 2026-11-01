// TODO: Remove once this is solved upstream:
// - https://github.com/elastio/bon/issues/367
// - https://github.com/rust-lang/rust-clippy/issues/13238#issuecomment-2807794976
#![expect(
    clippy::duplicated_attributes,
    reason = "bon::Builder repeats attributes in generated code (elastio/bon#367)"
)]

use alloc::vec::Vec;
use core::num::NonZero;
use core::ops::Add;
use core::time::Duration;

pub use bytes::Bytes;
pub use bytestring::ByteString;

/// An instant on the driver's clock.
///
/// The protocol only needs to order instants and to add a [`Duration`] to one
/// to compute a deadline (e.g. the next keep-alive PINGREQ or a connect
/// timeout). Any `Ord + Add<Duration> + Copy` type qualifies through the
/// blanket impl, such as [`Duration`] measured from an arbitrary epoch or
/// `std::time::Instant`.
pub trait Time: Ord + Add<Duration, Output = Self> + Copy {}

impl<T> Time for T where T: Ord + Add<Duration, Output = T> + Copy {}

/// Bytes read from the socket, paired with the instant they arrived.
///
/// The bytes need not hold a whole control packet: the protocol buffers
/// partial input until a complete packet can be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingData<'bytes, Time> {
    /// The bytes read from the socket.
    pub bytes: &'bytes [u8],
    /// When the bytes were read, on the driver's clock.
    pub received_at: Time,
}

/// An input to the protocol paired with the instant it happened.
///
/// A sans-IO protocol has no clock: it only learns the time from its inputs.
/// Every input it reacts to therefore carries the instant it happened, read
/// from the driver's clock: bytes arrive as [`IncomingData`], and
/// [`Command`]s and [`DriverEvent`]s arrive wrapped in a `Timestamped`. This
/// lets the protocol arm deadlines from any input, e.g. a connect timeout
/// from the [`Command::Connect`] that starts the attempt, and report them
/// through `poll_timeout`.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use sansio_mqtt_protocol::Command;
/// use sansio_mqtt_protocol::Timestamped;
///
/// let input = Timestamped {
///     value: Command::Disconnect,
///     at: Duration::from_secs(5),
/// };
/// assert_eq!(input.at, Duration::from_secs(5));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timestamped<Value, Time> {
    /// The input itself.
    pub value: Value,
    /// When the input happened, on the driver's clock.
    pub at: Time,
}

/// The delivery guarantee of an application message (MQTT v5.0 section 4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Qos {
    /// `QoS` 0: delivered at most once, with no acknowledgement
    /// (section 4.3.1).
    #[default]
    AtMostOnce,
    /// `QoS` 1: delivered at least once, acknowledged with PUBACK
    /// (section 4.3.2).
    AtLeastOnce,
    /// `QoS` 2: delivered exactly once, through the PUBREC, PUBREL and PUBCOMP
    /// exchange (section 4.3.3).
    ExactlyOnce,
}

/// Whether the server sends retained messages when a subscription is
/// established (Retain Handling subscription option, MQTT v5.0 section
/// 3.8.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RetainHandling {
    /// Send retained messages at the time of the subscribe (value 0).
    #[default]
    SendRetained,
    /// Send retained messages only if the subscription does not already
    /// exist (value 1).
    SendRetainedIfSubscriptionDoesNotExist,
    /// Do not send retained messages at the time of the subscribe (value 2).
    DoNotSend,
}

/// How the payload of a message is encoded (Payload Format Indicator
/// property, MQTT v5.0 section 3.3.2.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadFormat {
    /// The payload is unspecified bytes (value 0).
    Unspecified,
    /// The payload is UTF-8 encoded character data (value 1).
    Utf8,
}

/// An application message, either published by the application through
/// [`Command::Publish`] or received from the server in [`Event::Message`] or
/// [`Event::MessageRequiresAcknowledgement`].
///
/// The fields mirror the PUBLISH packet (MQTT v5.0 section 3.3). Strings and
/// binary data are validated when the message is encoded, so an invalid value
/// makes the [`Command::Publish`] fail with an [`Error`] rather than this type
/// being unconstructible.
///
/// # Examples
///
/// ```
/// use sansio_mqtt_protocol::Message;
/// use sansio_mqtt_protocol::Qos;
///
/// let message = Message::builder()
///     .topic("sensors/temperature")
///     .payload("21.5")
///     .qos(Qos::AtLeastOnce)
///     .build();
/// assert_eq!(message.qos, Qos::AtLeastOnce);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct Message {
    /// The Topic Name the message is published to (section 3.3.2.1). It must
    /// not contain wildcard characters [MQTT-3.3.2-2].
    pub topic: ByteString,
    /// The application payload (section 3.3.3).
    pub payload: Bytes,
    /// The delivery guarantee of the message.
    #[builder(default)]
    pub qos: Qos,
    /// The RETAIN flag (section 3.3.1.3): on an outbound message, asks the
    /// server to retain it for future subscribers; on an inbound message,
    /// reports whether the server delivered it as a retained message.
    #[builder(default)]
    pub retain: bool,
    /// The Payload Format Indicator property, if present (section 3.3.2.3.2).
    pub payload_format: Option<PayloadFormat>,
    /// The Message Expiry Interval property, if present (section 3.3.2.3.3).
    /// Encoded in whole seconds, so it must not exceed `u32::MAX` seconds.
    pub message_expiry: Option<Duration>,
    /// The Response Topic property, if present (section 3.3.2.3.5): the Topic
    /// Name a response to this message should be published to.
    pub response_topic: Option<ByteString>,
    /// The Correlation Data property, if present (section 3.3.2.3.6), used by
    /// the requester to match a response to its request.
    pub correlation_data: Option<Bytes>,
    /// The Content Type property, if present (section 3.3.2.3.9), describing
    /// the payload, e.g. a MIME type.
    pub content_type: Option<ByteString>,
    /// The User Property name/value pairs, in order (section 3.3.2.3.7).
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
    /// The Subscription Identifiers of the subscriptions that matched an
    /// inbound message (section 3.3.2.3.8). Not sent on outbound messages: a
    /// client must not send this property in a PUBLISH [MQTT-3.3.4-6].
    #[builder(default, with = FromIterator::from_iter)]
    pub subscription_identifiers: Vec<NonZero<u64>>,
}

/// The Will Message the server publishes when the connection ends without a
/// normal DISCONNECT (MQTT v5.0 section 3.1.2.5).
///
/// Set through [`ConnectOptions::will`].
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct Will {
    /// The Will Topic (section 3.1.3.3).
    pub topic: ByteString,
    /// The Will Payload (section 3.1.3.4). Empty by default.
    #[builder(default)]
    pub payload: Bytes,
    /// The Will `QoS` (section 3.1.2.6).
    #[builder(default)]
    pub qos: Qos,
    /// The Will Retain flag (section 3.1.2.7).
    #[builder(default)]
    pub retain: bool,
    /// The Will Delay Interval property, if present (section 3.1.3.2.2): how
    /// long the server waits after the connection ends before publishing the
    /// Will Message. Encoded in whole seconds.
    pub delay: Option<Duration>,
    /// The Payload Format Indicator property, if present (section
    /// 3.1.3.2.3).
    pub payload_format: Option<PayloadFormat>,
    /// The Message Expiry Interval property, if present (section 3.1.3.2.4).
    /// Encoded in whole seconds.
    pub message_expiry: Option<Duration>,
    /// The Response Topic property, if present (section 3.1.3.2.6).
    pub response_topic: Option<ByteString>,
    /// The Correlation Data property, if present (section 3.1.3.2.7).
    pub correlation_data: Option<Bytes>,
    /// The Content Type property, if present (section 3.1.3.2.5).
    pub content_type: Option<ByteString>,
    /// The User Property name/value pairs, in order (section 3.1.3.2.8).
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

/// One Topic Filter and its Subscription Options in a SUBSCRIBE (MQTT v5.0
/// section 3.8.3).
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct Subscription {
    /// The Topic Filter, which may contain the `+` and `#` wildcards
    /// (section 4.7) or name a Shared Subscription (`$share/...`,
    /// section 4.8.2).
    pub filter: ByteString,
    /// The Maximum `QoS` the server may deliver matching messages with.
    #[builder(default)]
    pub qos: Qos,
    /// The No Local option: if set, the server does not forward messages this
    /// client published itself [MQTT-3.8.3-3]. Must not be set on a Shared
    /// Subscription [MQTT-3.8.3-4].
    #[builder(default)]
    pub no_local: bool,
    /// The Retain As Published option: if set, forwarded messages keep the
    /// RETAIN flag they were published with.
    #[builder(default)]
    pub retain_as_published: bool,
    /// The Retain Handling option.
    #[builder(default)]
    pub retain_handling: RetainHandling,
}

/// Enhanced authentication sent in CONNECT (MQTT v5.0 section 4.12).
///
/// Set through [`ConnectOptions::authentication`].
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct Authentication {
    /// The Authentication Method property (section 3.1.2.11.9) naming the
    /// authentication scheme, e.g. `SCRAM-SHA-1`.
    pub method: ByteString,
    /// The Authentication Data property, if present (section 3.1.2.11.10),
    /// whose content is defined by the method.
    pub data: Option<Bytes>,
}

/// The per-connection parameters of a CONNECT (MQTT v5.0 section 3.1),
/// carried by [`Command::Connect`].
///
/// Properties the client negotiates once for all connections (Keep Alive,
/// Receive Maximum, Maximum Packet Size, ...) belong to the version-specific
/// client settings instead.
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct ConnectOptions {
    /// The Client Identifier (section 3.1.3.1). Empty by default, which asks
    /// the server to assign one [MQTT-3.1.3-6].
    #[builder(default)]
    pub client_id: ByteString,
    /// The Clean Start flag (section 3.1.2.4): if set, the server discards any
    /// existing session [MQTT-3.1.2-4].
    #[builder(default)]
    pub clean_start: bool,
    /// The Session Expiry Interval property, if present (section 3.1.2.11.2).
    /// `None` means the session ends when the Network Connection closes.
    /// Encoded in whole seconds.
    pub session_expiry: Option<Duration>,
    /// The User Name (section 3.1.3.5), if any.
    pub user_name: Option<ByteString>,
    /// The Password (section 3.1.3.6), if any.
    pub password: Option<Bytes>,
    /// The Will Message (section 3.1.2.5), if any.
    pub will: Option<Will>,
    /// Enhanced authentication (section 4.12), if any.
    pub authentication: Option<Authentication>,
    /// The User Property name/value pairs, in order (section 3.1.2.11.8).
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

/// The content of a SUBSCRIBE (MQTT v5.0 section 3.8), carried by
/// [`Command::Subscribe`].
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct SubscribeOptions {
    /// The subscriptions to request. A SUBSCRIBE must carry at least one
    /// [MQTT-3.8.3-2]; an empty list is rejected with
    /// [`Error::EmptySubscribe`].
    #[builder(default, with = FromIterator::from_iter)]
    pub subscriptions: Vec<Subscription>,
    /// The Subscription Identifier property, if present (section
    /// 3.8.2.1.2). The server echoes it in
    /// [`Message::subscription_identifiers`] of matching messages.
    pub identifier: Option<NonZero<u64>>,
    /// The User Property name/value pairs, in order (section 3.8.2.1.3).
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

/// The content of an UNSUBSCRIBE (MQTT v5.0 section 3.10), carried by
/// [`Command::Unsubscribe`].
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct UnsubscribeOptions {
    /// The Topic Filters to unsubscribe from. An UNSUBSCRIBE must carry at
    /// least one [MQTT-3.10.3-2]; an empty list is rejected with
    /// [`Error::EmptyUnsubscribe`].
    #[builder(default, with = |filters: impl IntoIterator<Item: Into<ByteString>>| filters.into_iter().map(Into::into).collect())]
    pub filters: Vec<ByteString>,
    /// The User Property name/value pairs, in order (section 3.10.2.1.2).
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

/// Identifies an inbound `QoS` 1 or `QoS` 2 message awaiting the
/// application's decision.
///
/// Delivered in [`Event::MessageRequiresAcknowledgement`] and passed back in
/// [`Command::Acknowledge`] or [`Command::Reject`]. It wraps the PUBLISH
/// Packet Identifier, which is never zero [MQTT-2.2.1-3].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId(NonZero<u16>);

impl MessageId {
    /// Wraps a Packet Identifier.
    #[must_use]
    pub const fn new(id: NonZero<u16>) -> Self {
        Self(id)
    }

    /// Returns the wrapped Packet Identifier.
    #[must_use]
    pub const fn get(self) -> NonZero<u16> {
        self.0
    }
}

/// Why the application rejects an inbound message in [`Command::Reject`].
///
/// Each variant is a Reason Code that both PUBACK (MQTT v5.0 section
/// 3.4.2.1) and PUBREC (section 3.5.2.1) may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RejectReason {
    /// The message is not accepted and no more specific reason applies
    /// (0x80).
    UnspecifiedError,
    /// The message is valid but not accepted by this implementation (0x83).
    ImplementationSpecificError,
    /// The publisher is not authorized to publish the message (0x87).
    NotAuthorized,
    /// The Topic Name is valid but not accepted (0x90).
    TopicNameInvalid,
    /// An implementation or administrative quota has been exceeded (0x97).
    QuotaExceeded,
    /// The payload does not match the Payload Format Indicator (0x99).
    PayloadFormatInvalid,
}

/// Why an outbound `QoS` 1 or `QoS` 2 message was dropped without being
/// delivered, reported in [`Event::PublishDropped`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropReason {
    /// The server did not resume the session (CONNACK Session Present was 0),
    /// so the message still in flight from the previous connection is lost.
    SessionNotResumed,
    /// The server answered the PUBLISH with this failure Reason Code (0x80 or
    /// above).
    BrokerRejected(ReasonCode),
}

/// A version-neutral MQTT Reason Code (MQTT v5.0 section 2.4).
///
/// Unlike the wire format, which reuses some numeric values across packets,
/// each variant names one meaning. A code received from the server that does
/// not map onto a known variant is reported as
/// [`UnspecifiedError`](Self::UnspecifiedError).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReasonCode {
    /// The operation succeeded (0x00).
    Success,
    /// The operation failed and no more specific reason applies (0x80).
    UnspecifiedError,
    /// A packet could not be parsed according to the specification (0x81).
    MalformedPacket,
    /// A packet violated the protocol (0x82).
    ProtocolError,
    /// The packet is valid but not accepted by this implementation (0x83).
    ImplementationSpecificError,
    /// The server does not support the requested protocol version (0x84).
    UnsupportedProtocolVersion,
    /// The Client Identifier is valid but not allowed by the server (0x85).
    ClientIdentifierNotValid,
    /// The User Name or Password was rejected (0x86).
    BadUserNameOrPassword,
    /// The client is not authorized for the operation (0x87).
    NotAuthorized,
    /// The server is not available (0x88).
    ServerUnavailable,
    /// The server is busy; try again later (0x89).
    ServerBusy,
    /// The client has been banned by administrative action (0x8A).
    Banned,
    /// The Authentication Method is not supported or does not match the one
    /// in use (0x8C).
    BadAuthenticationMethod,
    /// The Topic Name is well formed but not accepted (0x90).
    TopicNameInvalid,
    /// A packet exceeded the maximum permissible size (0x95).
    PacketTooLarge,
    /// An implementation or administrative quota has been exceeded (0x97).
    QuotaExceeded,
    /// The payload does not match the Payload Format Indicator (0x99).
    PayloadFormatInvalid,
    /// The server does not support retained messages (0x9A).
    RetainNotSupported,
    /// The server does not support the requested `QoS` (0x9B).
    QoSNotSupported,
    /// The client should temporarily use another server (0x9C).
    UseAnotherServer,
    /// The client should permanently use another server (0x9D).
    ServerMoved,
    /// The connection rate limit has been exceeded (0x9F).
    ConnectionRateExceeded,
    /// The message was accepted but no subscriber matched it (0x10).
    NoMatchingSubscribers,
    /// The Packet Identifier is already in use (0x91).
    PacketIdentifierInUse,
    /// The Packet Identifier is not known (0x92).
    PacketIdentifierNotFound,
    /// The subscription was granted with Maximum `QoS` 0 (0x00 in SUBACK).
    SuccessQoS0,
    /// The subscription was granted with Maximum `QoS` 1 (0x01).
    SuccessQoS1,
    /// The subscription was granted with Maximum `QoS` 2 (0x02).
    SuccessQoS2,
    /// No matching subscription existed to unsubscribe from (0x11).
    NoSubscriptionExisted,
    /// The Topic Filter is well formed but not accepted (0x8F).
    TopicFilterInvalid,
    /// The server does not support Subscription Identifiers (0xA1).
    SubscriptionIdentifiersNotSupported,
    /// The server does not support Wildcard Subscriptions (0xA2).
    WildcardSubscriptionsNotSupported,
    /// The connection was closed normally (0x00 in DISCONNECT).
    NormalDisconnection,
    /// The client disconnects but wants the Will Message published (0x04).
    DisconnectWithWillMessage,
    /// The server is shutting down (0x8B).
    ServerShuttingDown,
    /// No packet was received within 1.5 times the Keep Alive time (0x8D).
    KeepAliveTimeout,
    /// Another connection using the same Client Identifier took over the
    /// session (0x8E).
    SessionTakenOver,
    /// More `QoS` 1/2 PUBLISH packets were in flight than Receive Maximum
    /// allows (0x93).
    ReceiveMaximumExceeded,
    /// The Topic Alias was invalid (0x94).
    TopicAliasInvalid,
    /// The message rate limit has been exceeded (0x96).
    MessageRateTooHigh,
    /// The connection was closed by administrative action (0x98).
    AdministrativeAction,
    /// The server does not support Shared Subscriptions (0x9E).
    SharedSubscriptionsNotSupported,
    /// The maximum connection time authorized has been exceeded (0xA0).
    MaximumConnectTime,
    /// Enhanced authentication continues with another step (0x18).
    ContinueAuthentication,
    /// Re-authentication is being initiated (0x19).
    ReAuthenticate,
}

/// An instruction from the application to the protocol.
///
/// Passed to the protocol wrapped in [`Timestamped`]. A command the protocol
/// cannot accept is rejected with an [`Error`] and has no effect.
#[derive(Debug, Clone, PartialEq, Eq, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::Display))]
#[non_exhaustive]
pub enum Command {
    /// Start a connection: ask the driver to open a socket, then send CONNECT
    /// with these options. Answered by [`Event::Connected`] or
    /// [`Event::Disconnected`]. Rejected with
    /// [`Error::InvalidStateTransition`] while a connection or connection
    /// attempt exists.
    Connect(ConnectOptions),
    /// Publish a message while connected.
    ///
    /// `token` is an opaque value chosen by the application and echoed back
    /// in the [`Event::PublishAcknowledged`] (`QoS` 1),
    /// [`Event::PublishCompleted`] (`QoS` 2) or [`Event::PublishDropped`] that
    /// ends the exchange. A `QoS` 0 publish produces no such event, so its
    /// token is unused.
    Publish {
        /// The application's correlation token for this publish.
        token: u64,
        /// The message to publish.
        message: Message,
    },
    /// Accept an inbound message delivered in
    /// [`Event::MessageRequiresAcknowledgement`]: answer with a successful
    /// PUBACK (`QoS` 1) or PUBREC (`QoS` 2).
    Acknowledge(MessageId),
    /// Refuse an inbound message delivered in
    /// [`Event::MessageRequiresAcknowledgement`]: answer with a PUBACK
    /// (`QoS` 1) or PUBREC (`QoS` 2) carrying the given failure Reason Code.
    Reject(MessageId, RejectReason),
    /// Send a SUBSCRIBE while connected.
    Subscribe(SubscribeOptions),
    /// Send an UNSUBSCRIBE while connected.
    Unsubscribe(UnsubscribeOptions),
    /// Close the connection normally, sending DISCONNECT if a session is
    /// established, or abandon a connection attempt. Answered by
    /// [`Event::Disconnected`].
    Disconnect,
}

/// A notification from the protocol to the application.
///
/// Read through `poll_read` after feeding the protocol an input.
#[derive(Debug, Clone, PartialEq, Eq, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::Display))]
#[non_exhaustive]
pub enum Event {
    /// The server accepted the connection with a successful CONNACK.
    Connected,
    /// The connection or connection attempt ended.
    ///
    /// Carries the Reason Code that ended it when one applies: the one in the
    /// server's DISCONNECT or failed CONNACK, or the one the client sent when
    /// it closed the connection because of a protocol failure or a timeout.
    /// `None` for a client-initiated [`Command::Disconnect`], a lost socket,
    /// or an abandoned connection attempt.
    Disconnected(Option<ReasonCode>),
    /// An inbound `QoS` 0 message. It needs no acknowledgement.
    Message(Message),
    /// An inbound `QoS` 1 or `QoS` 2 message. The application must answer
    /// with [`Command::Acknowledge`] or [`Command::Reject`] carrying this
    /// [`MessageId`]. A redelivery of a message still awaiting that answer is
    /// not reported again.
    MessageRequiresAcknowledgement(MessageId, Message),
    /// The server answered a `QoS` 1 [`Command::Publish`] with PUBACK.
    PublishAcknowledged {
        /// The token of the [`Command::Publish`].
        token: u64,
        /// The PUBACK Reason Code, e.g. [`ReasonCode::Success`] or a failure
        /// such as [`ReasonCode::NotAuthorized`].
        reason: ReasonCode,
    },
    /// The `QoS` 2 exchange of a [`Command::Publish`] completed with PUBCOMP.
    PublishCompleted {
        /// The token of the [`Command::Publish`].
        token: u64,
        /// The PUBCOMP Reason Code: [`ReasonCode::Success`] or
        /// [`ReasonCode::PacketIdentifierNotFound`].
        reason: ReasonCode,
    },
    /// A `QoS` 1 or `QoS` 2 [`Command::Publish`] ended without delivery.
    PublishDropped {
        /// The token of the [`Command::Publish`].
        token: u64,
        /// Why the message was dropped.
        reason: DropReason,
    },
    /// The server sent an AUTH packet (MQTT v5.0 section 3.15) while
    /// connected, e.g. to start re-authentication. The application is
    /// responsible for responding [MQTT-4.12.0-4].
    Auth {
        /// The AUTH Reason Code: [`ReasonCode::Success`],
        /// [`ReasonCode::ContinueAuthentication`] or
        /// [`ReasonCode::ReAuthenticate`].
        reason: ReasonCode,
        /// The Authentication Method property, or the empty string if the
        /// packet omitted it.
        method: ByteString,
        /// The Authentication Data property, if present.
        data: Option<Bytes>,
        /// The Reason String property, if present: a human-readable diagnostic.
        reason_string: Option<ByteString>,
        /// The User Property name/value pairs, in order.
        user_properties: Vec<(ByteString, ByteString)>,
    },
}

/// A socket lifecycle notification from the driver to the protocol.
///
/// The driver reports only what the protocol did not ask for: the outcome of
/// a [`DriverAction::OpenSocket`], and the unsolicited loss of an open socket.
/// [`DriverAction::CloseSocket`] is never answered.
///
/// # Contract
///
/// - Report [`SocketConnected`](Self::SocketConnected) or
///   [`SocketError`](Self::SocketError) exactly once per performed
///   [`DriverAction::OpenSocket`], and never otherwise. An unrequested
///   `SocketConnected` is rejected with [`Error::InvalidStateTransition`].
/// - On peer EOF or an I/O error on an open socket, drop the socket first and
///   then report [`SocketClosed`](Self::SocketClosed) or
///   [`SocketError`](Self::SocketError). The protocol does not answer with
///   [`DriverAction::CloseSocket`]: the socket is already gone.
/// - Do not report anything after performing a [`DriverAction::CloseSocket`].
///
/// Reporting a loss is always accepted (`Ok(())`) and always discards the
/// bytes still queued in `poll_write`: they belonged to the lost socket.
///
/// - If a [`DriverAction::CloseSocket`] is still pending in the action queue,
///   the loss necessarily concerns the socket that action closes (e.g. writing
///   the DISCONNECT queued before it failed). The protocol has already torn
///   that connection down, so it emits nothing and leaves any connection
///   attempt queued behind the `CloseSocket` untouched. The driver still
///   performs the pending `CloseSocket`, which then has no socket to close.
/// - Otherwise, if the application believed a connection or connection attempt
///   existed, the protocol emits
///   [`Event::Disconnected(None)`](Event::Disconnected).
/// - Otherwise the report is a no-op, so no duplicate `Disconnected` is ever
///   emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DriverEvent {
    /// The socket requested by [`DriverAction::OpenSocket`] is connected.
    SocketConnected,
    /// The peer closed the open socket (EOF). The driver has already dropped
    /// it.
    SocketClosed,
    /// Opening the socket requested by [`DriverAction::OpenSocket`] failed, or
    /// an I/O error occurred on the open socket. The driver has already
    /// dropped it.
    SocketError,
}

/// A request from the protocol for the driver to act on the socket.
///
/// # Contract
///
/// - **Write ordering.** Before performing any action, the driver MUST write
///   every pending `poll_write` buffer to the current socket. Bytes queued
///   before a [`CloseSocket`](Self::CloseSocket), such as the DISCONNECT that
///   precedes it, belong to the socket being closed.
/// - **No bytes without a socket.** After the protocol queues `CloseSocket`, or
///   after the driver reports [`DriverEvent::SocketClosed`] or
///   [`DriverEvent::SocketError`], `poll_write` yields no bytes until the next
///   [`DriverEvent::SocketConnected`]. Reporting a loss discards the bytes
///   still queued for the lost socket, even those queued before a pending
///   `CloseSocket`.
/// - **One socket at a time.** The protocol never queues
///   [`OpenSocket`](Self::OpenSocket) while a socket is open, i.e. between
///   `SocketConnected` and the next `CloseSocket`, `SocketClosed` or
///   `SocketError`. In the action queue, a `CloseSocket` always precedes the
///   next `OpenSocket`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DriverAction {
    /// Open a socket to the server and answer with
    /// [`DriverEvent::SocketConnected`] or [`DriverEvent::SocketError`].
    OpenSocket,
    /// Close the current socket, or abandon an `OpenSocket` still in progress.
    ///
    /// Not answered: the protocol has already moved to a disconnected state
    /// and emitted whatever [`Event`] the application needs. A `CloseSocket`
    /// with no socket open is a no-op for the driver.
    CloseSocket,
}

/// A feature the server can disable in its CONNACK, and the client can
/// disable through its own settings.
///
/// Reported by [`Error::CapabilityNotAvailable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display)]
#[non_exhaustive]
pub enum Capability {
    /// Publishing at the requested `QoS` (Maximum `QoS`) [MQTT-3.2.2-11].
    #[strum(to_string = "the requested QoS")]
    MaximumQos,
    /// Retained messages (Retain Available).
    #[strum(to_string = "retained messages")]
    Retain,
    /// Wildcard topic filters (Wildcard Subscription Available)
    /// [MQTT-3.2.2-12].
    #[strum(to_string = "wildcard subscriptions")]
    WildcardSubscriptions,
    /// `$share/` topic filters (Shared Subscription Available)
    /// [MQTT-3.2.2-13].
    #[strum(to_string = "shared subscriptions")]
    SharedSubscriptions,
    /// Subscription Identifiers (Subscription Identifiers Available)
    /// [MQTT-3.2.2-14].
    #[strum(to_string = "subscription identifiers")]
    SubscriptionIdentifiers,
}

/// An input the protocol rejected.
///
/// `Err` from any `sansio::Protocol` method means only "this input was
/// rejected and nothing changed": the protocol state, its queues and the
/// session are exactly as they were before the call, so the caller may log
/// the error and carry on.
///
/// Connection-level failures that the protocol handles itself are not errors.
/// A malformed or protocol-violating packet from the server, a rejected
/// CONNACK, a socket error, a connect timeout (no CONNACK before the deadline
/// armed by [`Command::Connect`]) or a keep-alive timeout returns
/// `Ok(())`: the protocol queues any DISCONNECT and
/// [`DriverAction::CloseSocket`] it needs and reports the failure as
/// [`Event::Disconnected`] carrying the most fitting [`ReasonCode`] (for
/// example [`ReasonCode::MalformedPacket`], [`ReasonCode::ProtocolError`] or
/// [`ReasonCode::KeepAliveTimeout`]), or `None` when no reason code applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The command needs a capability that is not available on this
    /// connection, because the server's CONNACK or the client's own settings
    /// disabled it.
    #[error(
        "{0} is not available on this connection (disabled by the server or the client settings)"
    )]
    CapabilityNotAvailable(Capability),
    /// A SUBSCRIBE sets No Local on a Shared Subscription, which is a
    /// Protocol Error [MQTT-3.8.3-4].
    #[error("SUBSCRIBE sets No Local on a Shared Subscription [MQTT-3.8.3-4]")]
    NoLocalOnSharedSubscription,
    /// A duration (e.g. a Message Expiry or Will Delay Interval) does not fit
    /// the Four Byte Integer number of seconds MQTT encodes it as.
    #[error("a duration exceeds the MQTT interval limit of 4294967295 seconds")]
    IntervalTooLong,
    /// The command or driver event is not valid in the client's current
    /// connection state, e.g. `Publish` while disconnected, `Connect` while a
    /// connection exists, an unrequested `SocketConnected`, or inbound bytes
    /// with no socket open.
    #[error("command or event is not valid in the client's current connection state")]
    InvalidStateTransition,
    /// The encoded packet exceeds the Maximum Packet Size negotiated with the
    /// server.
    #[error("encoded control packet exceeds the negotiated Maximum Packet Size")]
    PacketTooLarge,
    /// Sending would exceed the server's Receive Maximum, or no Packet
    /// Identifier is free.
    #[error("in-flight QoS 1/QoS 2 PUBLISH count would exceed the peer's Receive Maximum")]
    ReceiveMaximumExceeded,
    /// The outbound packet could not be encoded.
    #[error("failed to encode an outbound MQTT control packet")]
    EncodeFailure,
    /// A Topic Name (the topic of a [`Message`] or [`Will`], or a Response
    /// Topic) is not valid, e.g. it contains a `+` or `#` wildcard
    /// [MQTT-3.3.2-2].
    #[error("topic name is not valid for PUBLISH (contains a wildcard or is otherwise malformed)")]
    InvalidTopic,
    /// A string field (a Topic Filter, Client Identifier, User Property, ...)
    /// contains characters MQTT forbids in a UTF-8 Encoded String, such as
    /// U+0000 [MQTT-1.5.4-2].
    #[error("topic filter is not a valid MQTT UTF-8 string")]
    InvalidTopicFilter,
    /// A string field exceeds the 65,535-byte limit of a UTF-8 Encoded String
    /// [MQTT-1.5.4-1].
    #[error("a UTF-8 string field exceeds the MQTT 65535-byte wire limit")]
    StringTooLong,
    /// A binary field (a Password, Correlation Data, Authentication Data or
    /// Will Payload) exceeds the 65,535-byte limit of Binary Data
    /// (section 1.5.6).
    #[error("a binary data field exceeds the MQTT 65535-byte wire limit")]
    BinaryTooLong,
    /// A [`Command::Subscribe`] carries no subscription; a SUBSCRIBE must
    /// carry at least one [MQTT-3.8.3-2].
    #[error("SUBSCRIBE must carry at least one subscription")]
    EmptySubscribe,
    /// A [`Command::Unsubscribe`] carries no Topic Filter; an UNSUBSCRIBE must
    /// carry at least one [MQTT-3.10.3-2].
    #[error("UNSUBSCRIBE must carry at least one topic filter")]
    EmptyUnsubscribe,
}

/// An MQTT client state machine for one protocol version, driven through
/// [`sansio::Protocol`].
///
/// Every input carries the instant it happened on the driver's clock `T`:
/// bytes as [`IncomingData`], [`Command`]s and [`DriverEvent`]s as
/// [`Timestamped`]. The protocol never reads a clock itself; it reports its
/// next deadline through `poll_timeout` and expects `handle_timeout` once that
/// deadline has passed.
pub trait MqttProtocol<'bytes, T: Time>:
    sansio::Protocol<
        IncomingData<'bytes, T>,
        Timestamped<Command, T>,
        Timestamped<DriverEvent, T>,
        Rout = Event,
        Wout = Bytes,
        Eout = DriverAction,
        Error = Error,
        Time = T,
    >
{
    /// Reports the MQTT protocol version supported by this implementation.
    fn version(&self) -> semver::Version;
}
