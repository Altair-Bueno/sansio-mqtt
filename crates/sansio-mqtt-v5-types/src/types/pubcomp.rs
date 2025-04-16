use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PubComp {
    pub packet_id: NonZero<u16>,
    pub reason_code: PubCompReasonCode,
    pub properties: PubCompProperties,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubCompHeaderFlags;

impl From<PubCompHeaderFlags> for u8 {
    fn from(_: PubCompHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubCompProperties {
    pub reason_string: Option<Utf8String>,

    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl PubCompProperties {
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
