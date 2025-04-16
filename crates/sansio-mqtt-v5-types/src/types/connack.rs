use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct ConnAck {
    pub kind: ConnAckKind,

    pub properties: ConnAckProperties,
}

#[derive(Debug, PartialEq, Clone)]

pub enum ConnAckKind {
    ResumePreviousSession,
    Other { reason_code: ConnackReasonCode },
}

#[derive(Debug, PartialEq, Clone)]
pub struct ConnAckHeaderFlags;

impl From<ConnAckHeaderFlags> for u8 {
    fn from(_: ConnAckHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct ConnAckProperties {
    pub session_expiry_interval: Option<u32>,
    pub receive_maximum: Option<NonZero<u16>>,
    pub maximum_qos: Option<MaximumQoS>,
    pub retain_available: Option<bool>,
    pub maximum_packet_size: Option<NonZero<u32>>,

    pub assigned_client_identifier: Option<Utf8String>,
    pub topic_alias_maximum: Option<u16>,

    pub reason_string: Option<Utf8String>,
    pub wildcard_subscription_available: Option<bool>,
    pub subscription_identifiers_available: Option<bool>,
    pub shared_subscription_available: Option<bool>,
    pub server_keep_alive: Option<u16>,

    pub response_information: Option<Utf8String>,

    pub server_reference: Option<Utf8String>,

    pub authentication: Option<AuthenticationKind>,

    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
