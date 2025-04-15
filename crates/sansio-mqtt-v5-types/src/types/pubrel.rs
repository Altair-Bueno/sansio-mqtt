use super::*;
#[derive(Debug, PartialEq, Clone)]

pub struct PubRel<'input> {
    pub packet_id: NonZero<u16>,
    pub reason_code: PubRelReasonCode,
    pub properties: PubRelProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubRelHeaderFlags;

impl From<PubRelHeaderFlags> for u8 {
    fn from(_: PubRelHeaderFlags) -> u8 {
        0b0000_0010
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubRelProperties<'input> {
    pub reason_string: Option<Utf8String<'input>>,
    pub user_properties: Vec<(Utf8String<'input>, Utf8String<'input>)>,
}

impl PubRelProperties<'_> {
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
