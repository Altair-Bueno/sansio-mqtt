use super::*;

#[derive(Debug, PartialEq, Eq, Clone)]

pub struct SubAck {
    pub packet_id: NonZero<u16>,
    pub properties: SubAckProperties,
    pub reason_codes: Vec<SubAckReasonCode>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]

pub struct SubAckProperties {
    pub reason_string: Option<Utf8String>,

    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, PartialEq, Eq, Clone)]

pub struct SubAckHeaderFlags;

impl From<SubAckHeaderFlags> for u8 {
    fn from(_: SubAckHeaderFlags) -> u8 {
        0
    }
}
