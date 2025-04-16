use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PubRec {
    pub packet_id: NonZero<u16>,
    pub reason_code: PubRecReasonCode,
    pub properties: PubRecProperties,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubRecHeaderFlags;

impl From<PubRecHeaderFlags> for u8 {
    fn from(_: PubRecHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubRecProperties {
    pub reason_string: Option<Utf8String>,

    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl PubRecProperties {
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
