use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PubRec<'input> {
    pub packet_id: NonZero<u16>,
    pub reason_code: PubRecReasonCode,
    pub properties: PubRecProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubRecHeaderFlags;

impl From<PubRecHeaderFlags> for u8 {
    fn from(_: PubRecHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubRecProperties<'input> {
    pub reason_string: Option<Utf8String<'input>>,

    pub user_properties: Vec<(Utf8String<'input>, Utf8String<'input>)>,
}

impl PubRecProperties<'_> {
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
