use super::*;
#[derive(Debug, PartialEq, Clone)]

pub struct UnsubAck {
    pub packet_id: NonZero<u16>,
    pub properties: UnsubAckProperties,
    pub reason_codes: Vec<UnsubAckReasonCode>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct UnsubAckHeaderFlags;

impl From<UnsubAckHeaderFlags> for u8 {
    fn from(_: UnsubAckHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct UnsubAckProperties {
    pub reason_string: Option<Utf8String>,

    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
