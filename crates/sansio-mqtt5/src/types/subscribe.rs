use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Subscribe<'input> {
    pub packet_id: NonZero<u16>,
    
    pub subscriptions: Vec<Subscription<'input>>,
    
    pub properties: SubscribeProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct SubscribeHeaderFlags;

#[derive(Debug, PartialEq, Clone)]

pub struct Subscription<'input> {
    
    pub topic: MQTTString<'input>,
    pub qos: Qos,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: RetainHandling,
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct SubscribeProperties<'input> {
    pub subscription_identifier: Option<NonZero<u64>>,
    
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
