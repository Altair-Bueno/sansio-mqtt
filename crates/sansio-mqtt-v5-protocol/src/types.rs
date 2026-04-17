use core::num::NonZero;
use core::time::Duration;

use alloc::vec::Vec;
use sansio_mqtt_v5_types::AuthenticationKind;
use sansio_mqtt_v5_types::BinaryData;
use sansio_mqtt_v5_types::FormatIndicator;
use sansio_mqtt_v5_types::Payload;
use sansio_mqtt_v5_types::Qos;
use sansio_mqtt_v5_types::Topic;
use sansio_mqtt_v5_types::Utf8String;
use sansio_mqtt_v5_types::Vec1;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConnectionOptions {
    pub clean_start: bool,
    pub client_identifier: Utf8String,
    pub user_name: Option<Utf8String>,
    pub password: Option<BinaryData>,
    pub keep_alive: Option<NonZero<u16>>,
    pub will: Option<Will>,

    pub session_expiry_interval: Option<u32>,
    pub topic_alias_maximum: Option<u16>,
    // pub receive_maximum: Option<NonZero<u16>>,
    // pub maximum_packet_size: Option<NonZero<u32>>,
    pub request_response_information: Option<bool>,
    pub request_problem_information: Option<bool>,
    pub authentication: Option<AuthenticationKind>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Will {
    pub topic: Topic,
    pub payload: Payload,
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<Duration>,
    pub topic_alias: Option<NonZero<u16>>,
    pub response_topic: Option<Topic>,
    pub correlation_data: Option<BinaryData>,
    pub content_type: Option<Utf8String>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
    pub will_delay_interval: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientMessage {
    pub topic: Topic,
    pub payload: Payload,
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<Duration>,
    pub topic_alias: Option<NonZero<u16>>,
    pub response_topic: Option<Topic>,
    pub correlation_data: Option<BinaryData>,
    pub content_type: Option<Utf8String>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrokerMessage {
    pub topic: Topic,
    pub payload: Payload,
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<Duration>,
    pub topic_alias: Option<NonZero<u16>>,
    pub response_topic: Option<Topic>,
    pub correlation_data: Option<BinaryData>,
    pub subscription_identifier: Option<NonZero<u64>>,
    pub content_type: Option<Utf8String>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscribeOptions {
    pub subscriptions: Vec1<Utf8String>,
    pub qos: Qos,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: u8,
    pub subscription_identifier: Option<NonZero<u64>>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnsubscribeOptions {
    pub subscriptions: Vec1<Utf8String>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

// Things that the protocol can read from the socket (via the driver)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserWriteOut {
    ReceivedMessage(BrokerMessage),
    Connected,
    Disconnected,
}

// Things that the client can write to the socket (via the driver)
#[derive(Debug, Clone, PartialEq)]
pub enum UserWriteIn {
    Connect(ConnectionOptions),
    PublishMessage(ClientMessage),
    Subscribe(SubscribeOptions),
    Unsubcribe(UnsubscribeOptions),
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
