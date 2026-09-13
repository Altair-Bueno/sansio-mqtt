use alloc::vec::Vec;
use core::num::NonZero;
use core::ops::Add;
use core::time::Duration;

pub use bytes::Bytes;
pub use bytestring::ByteString;

pub trait Time: Ord + Add<Duration, Output = Self> + Copy {}

impl<T> Time for T where T: Ord + Add<Duration, Output = T> + Copy {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingData<Time> {
    pub bytes: Bytes,
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
    #[builder(default)]
    pub user_properties: Vec<(ByteString, ByteString)>,
    #[builder(default)]
    pub subscription_identifiers: Vec<NonZero<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct Will {
    pub topic: ByteString,
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
    #[builder(default)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
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
#[non_exhaustive]
pub struct Authentication {
    pub method: ByteString,
    pub data: Option<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct ConnectOptions {
    pub client_id: ByteString,
    #[builder(default)]
    pub clean_start: bool,
    pub keep_alive: Option<NonZero<u16>>,
    pub session_expiry: Option<Duration>,
    pub user_name: Option<ByteString>,
    pub password: Option<Bytes>,
    pub will: Option<Will>,
    pub authentication: Option<Authentication>,
    #[builder(default)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct SubscribeOptions {
    #[builder(default)]
    pub subscriptions: Vec<Subscription>,
    pub identifier: Option<NonZero<u64>>,
    #[builder(default)]
    pub user_properties: Vec<(ByteString, ByteString)>,
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct UnsubscribeOptions {
    #[builder(default)]
    pub filters: Vec<ByteString>,
    #[builder(default)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DriverEvent {
    SocketConnected,
    SocketClosed,
    SocketError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DriverAction {
    OpenSocket,
    CloseSocket,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("malformed MQTT control packet received from the peer")]
    MalformedPacket,
    #[error("protocol error: packet violated MQTT sequencing or state rules")]
    ProtocolError,
    #[error("command or event is not valid in the client's current connection state")]
    InvalidStateTransition,
    #[error("encoded control packet exceeds the negotiated Maximum Packet Size")]
    PacketTooLarge,
    #[error("in-flight QoS 1/QoS 2 PUBLISH count would exceed the peer's Receive Maximum")]
    ReceiveMaximumExceeded,
    #[error("failed to encode an outbound MQTT control packet")]
    EncodeFailure,
    #[error("CONNACK was not received before the connect timeout elapsed")]
    ConnectTimeout,
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

pub trait MqttProtocol<T: Time>:
    sansio::Protocol<
        IncomingData<T>,
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
