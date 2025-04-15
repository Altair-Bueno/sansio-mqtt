use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Subscribe<'input> {
    pub packet_id: NonZero<u16>,
    pub subscriptions: Vec1<Subscription<'input>>,
    pub properties: SubscribeProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct SubscribeHeaderFlags;

impl From<SubscribeHeaderFlags> for u8 {
    fn from(_: SubscribeHeaderFlags) -> u8 {
        0b0000_0010
    }
}

#[derive(Debug, PartialEq, Clone)]

pub struct Subscription<'input> {
    pub topic_filter: Utf8String<'input>,
    pub qos: Qos,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: RetainHandling,
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct SubscribeProperties<'input> {
    pub subscription_identifier: Option<NonZero<u64>>,

    pub user_properties: Vec<(Utf8String<'input>, Utf8String<'input>)>,
}
