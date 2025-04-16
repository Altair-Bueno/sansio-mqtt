use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Unsubscribe {
    pub packet_id: NonZero<u16>,
    pub properties: UnsubscribeProperties,
    pub topics: Vec1<Utf8String>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct UnsubscribeHeaderFlags;

impl From<UnsubscribeHeaderFlags> for u8 {
    fn from(_: UnsubscribeHeaderFlags) -> u8 {
        0b0000_0010
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct UnsubscribeProperties {
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
