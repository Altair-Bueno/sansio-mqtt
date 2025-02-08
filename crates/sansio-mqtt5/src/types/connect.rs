use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct Connect<'input> {
    pub protocol_name: MQTTString<'input>,
    pub protocol_version: u8,
    pub clean_start: bool,
    pub client_identifier: MQTTString<'input>,
    pub will: Option<Will<'input>>,
    pub user_name: Option<MQTTString<'input>>,
    pub password: Option<Cow<'input, [u8]>>,
    pub keep_alive: u16,
    pub properties: ConnectProperties<'input>,
}
#[derive(Debug, PartialEq, Clone)]
pub struct ConnectHeaderFlags;

#[derive(Debug, PartialEq, Clone)]
pub struct Will<'input> {
    pub topic: PublishTopic<'input>,
    pub payload: Cow<'input, [u8]>,
    pub qos: Qos,
    pub retain: bool,
    pub properties: WillProperties<'input>,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct WillProperties<'input> {
    pub will_delay_interval: Option<u32>,
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<u32>,
    pub content_type: Option<MQTTString<'input>>,
    pub response_topic: Option<PublishTopic<'input>>,
    pub correlation_data: Option<Cow<'input, [u8]>>,
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ConnectProperties<'input> {
    pub session_expiry_interval: Option<u32>,
    pub receive_maximum: Option<NonZero<u16>>,
    pub maximum_packet_size: Option<NonZero<u32>>,
    pub topic_alias_maximum: Option<u16>,
    pub request_response_information: Option<bool>,
    pub request_problem_information: Option<bool>,
    pub authentication: Option<AuthenticationKind<'input>>,
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
