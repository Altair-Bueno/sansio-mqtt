use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Unsubscribe<'input> {
    pub packet_id: NonZero<u16>,
    pub properties: UnsubscribeProperties<'input>,
    pub topics: Vec1<Utf8String<'input>>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct UnsubscribeHeaderFlags;

impl From<UnsubscribeHeaderFlags> for u8 {
    fn from(_: UnsubscribeHeaderFlags) -> u8 {
        0b0000_0010
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct UnsubscribeProperties<'input> {
    pub user_properties: Vec<(Utf8String<'input>, Utf8String<'input>)>,
}
