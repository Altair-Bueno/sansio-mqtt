use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Subscribe {
    pub packet_id: NonZero<u16>,
    pub subscriptions: Vec1<Subscription>,
    pub properties: SubscribeProperties,
}

#[derive(Debug, PartialEq, Clone)]

pub struct SubscribeHeaderFlags;

impl From<SubscribeHeaderFlags> for u8 {
    fn from(_: SubscribeHeaderFlags) -> u8 {
        0b0000_0010
    }
}

#[derive(Debug, PartialEq, Clone)]

pub struct Subscription {
    pub topic_filter: Utf8String,
    pub qos: Qos,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: RetainHandling,
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct SubscribeProperties {
    pub subscription_identifier: Option<NonZero<u64>>,

    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
