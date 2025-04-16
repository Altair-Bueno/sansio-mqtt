use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct Connect {
    pub protocol_name: Utf8String,
    pub protocol_version: u8,
    pub clean_start: bool,
    pub client_identifier: Utf8String,
    pub will: Option<Will>,
    pub user_name: Option<Utf8String>,
    pub password: Option<BinaryData>,
    pub keep_alive: Option<NonZero<u16>>,
    pub properties: ConnectProperties,
}
#[derive(Debug, PartialEq, Clone)]
pub struct ConnectHeaderFlags;

impl From<ConnectHeaderFlags> for u8 {
    fn from(_: ConnectHeaderFlags) -> u8 {
        0b0000_0000
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Will {
    pub topic: Topic,
    pub payload: BinaryData,
    pub qos: Qos,
    pub retain: bool,
    pub properties: WillProperties,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct WillProperties {
    pub will_delay_interval: Option<u32>,
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<u32>,
    pub content_type: Option<Utf8String>,
    pub response_topic: Option<Topic>,
    pub correlation_data: Option<BinaryData>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ConnectProperties {
    pub session_expiry_interval: Option<u32>,
    pub receive_maximum: Option<NonZero<u16>>,
    pub maximum_packet_size: Option<NonZero<u32>>,
    pub topic_alias_maximum: Option<u16>,
    pub request_response_information: Option<bool>,
    pub request_problem_information: Option<bool>,
    pub authentication: Option<AuthenticationKind>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
