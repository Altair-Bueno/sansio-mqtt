/// Trait for time types that support deadline arithmetic with keep-alive intervals.
///
/// Implementors can compute a future deadline by adding a number of seconds to
/// a current instant.  The protocol crate uses this to schedule keep-alive
/// timeouts without a dependency on any specific time library.
///
/// # Implementing for custom time types
///
/// ```rust
/// use sansio_mqtt_v5_protocol::InstantAdd;
///
/// #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// struct MyTick(u64);
///
/// impl InstantAdd for MyTick {
///     fn add_secs(self, secs: u16) -> Self {
///         MyTick(self.0 + u64::from(secs))
///     }
/// }
/// ```
pub trait InstantAdd: Copy + Ord + 'static {
    /// Returns a new instant that is `secs` seconds after `self`.
    fn add_secs(self, secs: u16) -> Self;
}

impl InstantAdd for u64 {
    #[inline]
    fn add_secs(self, secs: u16) -> Self {
        self + u64::from(secs)
    }
}

#[cfg(feature = "tokio")]
impl InstantAdd for ::tokio::time::Instant {
    #[inline]
    fn add_secs(self, secs: u16) -> Self {
        self + core::time::Duration::from_secs(u64::from(secs))
    }
}

// Reexport types from the sansio-mqtt-v5-types crate for usability
pub use sansio_mqtt_v5_types::Auth as AuthPacket;
pub use sansio_mqtt_v5_types::AuthReasonCode;
pub use sansio_mqtt_v5_types::AuthenticationKind;
pub use sansio_mqtt_v5_types::BinaryData;
pub use sansio_mqtt_v5_types::DisconnectReasonCode;
pub use sansio_mqtt_v5_types::FormatIndicator;
pub use sansio_mqtt_v5_types::Payload;
pub use sansio_mqtt_v5_types::PubAckReasonCode;
pub use sansio_mqtt_v5_types::PubCompReasonCode;
pub use sansio_mqtt_v5_types::PubRecReasonCode;
pub use sansio_mqtt_v5_types::Qos;
pub use sansio_mqtt_v5_types::Subscription;
pub use sansio_mqtt_v5_types::Topic;
pub use sansio_mqtt_v5_types::Utf8String;

use alloc::vec::Vec;
use core::num::NonZero;
use core::time::Duration;
use sansio_mqtt_v5_types::MaximumQoS;
use sansio_mqtt_v5_types::ParserSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSettings {
    pub max_bytes_string: u16,
    pub max_bytes_binary_data: u16,
    pub max_remaining_bytes: u64,
    pub max_subscriptions_len: u32,
    pub max_user_properties_len: usize,
    pub max_subscription_identifiers_len: usize,
    pub max_incoming_receive_maximum: Option<NonZero<u16>>,
    pub max_incoming_packet_size: Option<NonZero<u32>>,
    pub max_incoming_topic_alias_maximum: Option<u16>,
    pub max_outgoing_qos: Option<MaximumQoS>,
    pub allow_retain: bool,
    pub allow_wildcard_subscriptions: bool,
    pub allow_shared_subscriptions: bool,
    pub allow_subscription_identifiers: bool,
    pub default_request_response_information: Option<bool>,
    pub default_request_problem_information: Option<bool>,
    pub default_keep_alive: Option<NonZero<u16>>,
}

impl Default for ClientSettings {
    fn default() -> Self {
        let settings = ParserSettings::default();

        Self {
            max_bytes_string: settings.max_bytes_string,
            max_bytes_binary_data: settings.max_bytes_binary_data,
            max_remaining_bytes: settings.max_remaining_bytes,
            max_subscriptions_len: settings.max_subscriptions_len,
            max_user_properties_len: settings.max_user_properties_len,
            max_subscription_identifiers_len: settings.max_subscription_identifiers_len,
            max_incoming_receive_maximum: None,
            max_incoming_packet_size: None,
            max_incoming_topic_alias_maximum: None,
            max_outgoing_qos: None,
            allow_retain: true,
            allow_wildcard_subscriptions: true,
            allow_shared_subscriptions: true,
            allow_subscription_identifiers: true,
            default_request_response_information: None,
            default_request_problem_information: None,
            default_keep_alive: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("malformed packet")]
    MalformedPacket,
    #[error("protocol error")]
    ProtocolError,
    #[error("invalid state transition")]
    InvalidStateTransition,
    #[error("packet too large")]
    PacketTooLarge,
    #[error("receive maximum exceeded")]
    ReceiveMaximumExceeded,
    #[error("encode failure")]
    EncodeFailure,
    /// [MQTT-3.1.4-5] The connection-establishment timeout elapsed before CONNACK was
    /// received (or before CONNECT was sent in the Start state). The socket has been closed.
    #[error("connect timeout")]
    ConnectTimeout,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConnectionOptions {
    pub clean_start: bool,
    pub client_identifier: Utf8String,
    pub will: Option<Will>,
    pub user_name: Option<Utf8String>,
    pub password: Option<BinaryData>,
    pub keep_alive: Option<NonZero<u16>>,
    pub session_expiry_interval: Option<u32>,
    pub receive_maximum: Option<NonZero<u16>>,
    pub maximum_packet_size: Option<NonZero<u32>>,
    pub topic_alias_maximum: Option<u16>,
    pub request_response_information: Option<bool>,
    pub request_problem_information: Option<bool>,
    pub authentication: Option<AuthenticationKind>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Will {
    pub topic: Topic,
    pub payload: Payload,
    pub qos: Qos,
    pub retain: bool,
    pub will_delay_interval: Option<u32>,
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<Duration>,
    pub content_type: Option<Utf8String>,
    pub response_topic: Option<Topic>,
    pub correlation_data: Option<BinaryData>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientMessage {
    pub qos: Qos,
    pub retain: bool,
    pub payload: Payload,
    pub topic: Topic,
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<Duration>,
    pub topic_alias: Option<NonZero<u16>>,
    pub response_topic: Option<Topic>,
    pub correlation_data: Option<BinaryData>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
    pub content_type: Option<Utf8String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrokerMessage {
    pub qos: Qos,
    pub retain: bool,
    pub payload: Payload,
    pub topic: Topic,
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<Duration>,
    pub topic_alias: Option<NonZero<u16>>,
    pub response_topic: Option<Topic>,
    pub correlation_data: Option<BinaryData>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
    /// Zero or more Subscription Identifiers per [MQTT-3.3.2.3.8]. An empty
    /// `Vec` means no subscription identifier was attached on the wire.
    pub subscription_identifiers: Vec<NonZero<u64>>,
    pub content_type: Option<Utf8String>,
}

#[derive(Debug)]
pub struct InboundMessageId(NonZero<u16>);

impl InboundMessageId {
    pub(crate) fn new(id: NonZero<u16>) -> Self {
        Self(id)
    }

    pub(crate) fn get(self) -> NonZero<u16> {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeOptions {
    pub subscription: Subscription,
    pub extra_subscriptions: Vec<Subscription>,
    pub subscription_identifier: Option<NonZero<u64>>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsubscribeOptions {
    pub filter: Utf8String,
    pub extra_filters: Vec<Utf8String>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

// Things that the protocol can read from the socket (via the driver)
#[derive(Debug)]
pub enum UserWriteOut {
    ReceivedMessage(BrokerMessage),
    ReceivedMessageWithRequiredAcknowledgement(InboundMessageId, BrokerMessage),
    PublishAcknowledged(NonZero<u16>, PubAckReasonCode),
    PublishCompleted(NonZero<u16>, PubCompReasonCode),
    PublishDroppedDueToSessionNotResumed(NonZero<u16>),
    PublishDroppedDueToBrokerRejectedPubRec(NonZero<u16>, PubRecReasonCode),
    Connected,
    /// The connection is now disconnected.
    ///
    /// [MQTT-4.13.0-1] When the payload carries `Some(reason_code)`, the disconnect was
    /// initiated by the server via a DISCONNECT packet with that reason code. When
    /// `None`, the disconnect was client-initiated or the socket was closed without an
    /// explicit DISCONNECT from the server.
    Disconnected(Option<DisconnectReasonCode>),
    /// [MQTT-4.12.0-2] The server has sent an AUTH packet initiating or continuing
    /// re-authentication during an established session. The application must respond
    /// by sending a corresponding AUTH or DISCONNECT.
    Auth(AuthPacket),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomingRejectReason {
    UnspecifiedError,
    ImplementationSpecificError,
    NotAuthorized,
    TopicNameInvalid,
    QuotaExceeded,
    PayloadFormatInvalid,
}

// Things that the client can write to the socket (via the driver)
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum UserWriteIn {
    Connect(ConnectionOptions),
    PublishMessage(ClientMessage),
    AcknowledgeMessage(InboundMessageId),
    RejectMessage(InboundMessageId, IncomingRejectReason),
    Subscribe(SubscribeOptions),
    Unsubscribe(UnsubscribeOptions),
    Disconnect,
}

// Driver events to the protocol
#[derive(Debug)]
pub enum DriverEventIn {
    SocketClosed,
    SocketConnected,
    SocketError,
}

// Actions that the protocol wants to perform on the driver
#[derive(Debug)]
pub enum DriverEventOut {
    OpenSocket,
    CloseSocket,
    Quit,
}
