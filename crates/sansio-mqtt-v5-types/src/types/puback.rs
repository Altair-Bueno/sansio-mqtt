use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PubAck {
    pub packet_id: NonZero<u16>,
    pub reason_code: PubAckReasonCode,
    pub properties: PubAckProperties,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubAckHeaderFlags;

impl From<PubAckHeaderFlags> for u8 {
    fn from(_: PubAckHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubAckProperties {
    pub reason_string: Option<Utf8String>,

    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl PubAckProperties {
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
