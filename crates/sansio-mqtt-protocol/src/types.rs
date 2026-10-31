// TODO: Remove once this is solved upstream:
// - https://github.com/elastio/bon/issues/367
// - https://github.com/rust-lang/rust-clippy/issues/13238#issuecomment-2807794976
#![allow(clippy::duplicated_attributes)]

use alloc::vec::Vec;
use core::num::NonZero;
use core::ops::Add;
use core::time::Duration;

pub use bytes::Bytes;
pub use bytestring::ByteString;

pub trait Time: Ord + Add<Duration, Output = Self> + Copy {}

impl<T> Time for T where T: Ord + Add<Duration, Output = T> + Copy {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingData<'bytes, Time> {
    pub bytes: &'bytes [u8],
    pub received_at: Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Qos {
    #[default]
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RetainHandling {
    #[default]
    SendRetained,
    SendRetainedIfSubscriptionDoesNotExist,
    DoNotSend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadFormat {
    Unspecified,
    Utf8,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct Message {
    pub topic: ByteString,
    pub payload: Bytes,
    #[builder(default)]
    pub qos: Qos,
    #[builder(default)]
    pub retain: bool,
    pub payload_format: Option<PayloadFormat>,
    pub message_expiry: Option<Duration>,
    pub response_topic: Option<ByteString>,
    pub correlation_data: Option<Bytes>,
    pub content_type: Option<ByteString>,
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
    #[builder(default, with = FromIterator::from_iter)]
    pub subscription_identifiers: Vec<NonZero<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct Will {
    pub topic: ByteString,
    #[builder(default)]
    pub payload: Bytes,
    #[builder(default)]
    pub qos: Qos,
    #[builder(default)]
    pub retain: bool,
    pub delay: Option<Duration>,
    pub payload_format: Option<PayloadFormat>,
    pub message_expiry: Option<Duration>,
    pub response_topic: Option<ByteString>,
    pub correlation_data: Option<Bytes>,
    pub content_type: Option<ByteString>,
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct Subscription {
    pub filter: ByteString,
    #[builder(default)]
    pub qos: Qos,
    #[builder(default)]
    pub no_local: bool,
    #[builder(default)]
    pub retain_as_published: bool,
    #[builder(default)]
    pub retain_handling: RetainHandling,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct Authentication {
    pub method: ByteString,
    pub data: Option<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct ConnectOptions {
    #[builder(default)]
    pub client_id: ByteString,
    #[builder(default)]
    pub clean_start: bool,
    pub keep_alive: Option<NonZero<u16>>,
    pub session_expiry: Option<Duration>,
    pub user_name: Option<ByteString>,
    pub password: Option<Bytes>,
    pub will: Option<Will>,
    pub authentication: Option<Authentication>,
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct SubscribeOptions {
    #[builder(default, with = FromIterator::from_iter)]
    pub subscriptions: Vec<Subscription>,
    pub identifier: Option<NonZero<u64>>,
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(ByteString, into), on(Bytes, into))]
#[non_exhaustive]
pub struct UnsubscribeOptions {
    #[builder(default, with = |filters: impl IntoIterator<Item: Into<ByteString>>| filters.into_iter().map(Into::into).collect())]
    pub filters: Vec<ByteString>,
    #[builder(default, with = FromIterator::from_iter)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageId(NonZero<u16>);

impl MessageId {
    pub const fn new(id: NonZero<u16>) -> Self {
        Self(id)
    }

    pub const fn get(self) -> NonZero<u16> {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RejectReason {
    UnspecifiedError,
    ImplementationSpecificError,
    NotAuthorized,
    TopicNameInvalid,
    QuotaExceeded,
    PayloadFormatInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropReason {
    SessionNotResumed,
    BrokerRejected(ReasonCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReasonCode {
    Success,
    UnspecifiedError,
    MalformedPacket,
    ProtocolError,
    ImplementationSpecificError,
    UnsupportedProtocolVersion,
    ClientIdentifierNotValid,
    BadUserNameOrPassword,
    NotAuthorized,
    ServerUnavailable,
    ServerBusy,
    Banned,
    BadAuthenticationMethod,
    TopicNameInvalid,
    PacketTooLarge,
    QuotaExceeded,
    PayloadFormatInvalid,
    RetainNotSupported,
    QoSNotSupported,
    UseAnotherServer,
    ServerMoved,
    ConnectionRateExceeded,
    NoMatchingSubscribers,
    PacketIdentifierInUse,
    PacketIdentifierNotFound,
    SuccessQoS0,
    SuccessQoS1,
    SuccessQoS2,
    NoSubscriptionExisted,
    TopicFilterInvalid,
    SubscriptionIdentifiersNotSupported,
    WildcardSubscriptionsNotSupported,
    NormalDisconnection,
    DisconnectWithWillMessage,
    ServerShuttingDown,
    KeepAliveTimeout,
    SessionTakenOver,
    ReceiveMaximumExceeded,
    TopicAliasInvalid,
    MessageRateTooHigh,
    AdministrativeAction,
    SharedSubscriptionsNotSupported,
    MaximumConnectTime,
    ContinueAuthentication,
    ReAuthenticate,
}

#[derive(Debug, Clone, PartialEq, Eq, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::Display))]
#[non_exhaustive]
pub enum Command {
    Connect(ConnectOptions),
    Publish { token: u64, message: Message },
    Acknowledge(MessageId),
    Reject(MessageId, RejectReason),
    Subscribe(SubscribeOptions),
    Unsubscribe(UnsubscribeOptions),
    Disconnect,
}

#[derive(Debug, Clone, PartialEq, Eq, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::Display))]
#[non_exhaustive]
pub enum Event {
    Connected,
    Disconnected(Option<ReasonCode>),
    Message(Message),
    MessageRequiresAcknowledgement(MessageId, Message),
    PublishAcknowledged {
        token: u64,
        reason: ReasonCode,
    },
    PublishCompleted {
        token: u64,
        reason: ReasonCode,
    },
    PublishDropped {
        token: u64,
        reason: DropReason,
    },
    Auth {
        reason: ReasonCode,
        method: ByteString,
        data: Option<Bytes>,
        reason_string: Option<ByteString>,
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
    /// Publishing at the requested QoS (Maximum QoS) [MQTT-3.2.2-11].
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
/// CONNACK, a socket error, a connect timeout or a keep-alive timeout returns
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
    #[error("topic name is not valid for PUBLISH (contains a wildcard or is otherwise malformed)")]
    InvalidTopic,
    #[error("topic filter is not a valid MQTT UTF-8 string")]
    InvalidTopicFilter,
    #[error("a UTF-8 string field exceeds the MQTT 65535-byte wire limit")]
    StringTooLong,
    #[error("a binary data field exceeds the MQTT 65535-byte wire limit")]
    BinaryTooLong,
    #[error("SUBSCRIBE must carry at least one subscription")]
    EmptySubscribe,
    #[error("UNSUBSCRIBE must carry at least one topic filter")]
    EmptyUnsubscribe,
}

pub trait MqttProtocol<'bytes, T: Time>:
    sansio::Protocol<
        IncomingData<'bytes, T>,
        Command,
        DriverEvent,
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
