// Reexport types from the sansio-mqtt-v5-types crate for usability
pub use sansio_mqtt_v5_types::AuthenticationKind;
pub use sansio_mqtt_v5_types::BinaryData;
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
use sansio_mqtt_v5_types::ParserSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSettings {
    pub parser_max_bytes_string: u16,
    pub parser_max_bytes_binary_data: u16,
    pub parser_max_remaining_bytes: u64,
    pub parser_max_subscriptions_len: u32,
    pub parser_max_user_properties_len: usize,
}

impl Default for ClientSettings {
    fn default() -> Self {
        let settings = ParserSettings::default();

        Self {
            parser_max_bytes_string: settings.max_bytes_string,
            parser_max_bytes_binary_data: settings.max_bytes_binary_data,
            parser_max_remaining_bytes: settings.max_remaining_bytes,
            parser_max_subscriptions_len: settings.max_subscriptions_len,
            parser_max_user_properties_len: settings.max_user_properties_len,
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
    pub subscription_identifier: Option<NonZero<u64>>,
    pub content_type: Option<Utf8String>,
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
#[derive(Debug, Clone, PartialEq)]
pub enum UserWriteOut {
    ReceivedMessage(Option<NonZero<u16>>, BrokerMessage),
    PublishAcknowledged(NonZero<u16>, PubAckReasonCode),
    PublishCompleted(NonZero<u16>, PubCompReasonCode),
    PublishDroppedDueToSessionNotResumed(NonZero<u16>),
    PublishDroppedDueToBrokerRejectedPubRec(NonZero<u16>, PubRecReasonCode),
    Connected,
    Disconnected,
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
#[derive(Debug, Clone, PartialEq)]
pub enum UserWriteIn {
    Connect(ConnectionOptions),
    PublishMessage(ClientMessage),
    AcknowledgeMessage(NonZero<u16>),
    RejectMessage(NonZero<u16>, IncomingRejectReason),
    Subscribe(SubscribeOptions),
    Unsubscribe(UnsubscribeOptions),
    Disconnect,
}

// Driver events to the protocol
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DriverEventIn {
    SocketClosed,
    SocketConnected,
    SocketError,
}

// Actions that the protocol wants to perform on the driver
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DriverEventOut {
    OpenSocket,
    CloseSocket,
    Quit,
}
