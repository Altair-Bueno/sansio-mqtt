use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Unsubscribe<'input> {
    pub packet_id: NonZero<u16>,

    pub properties: UnsubscribeProperties<'input>,

    pub topics: Vec<MQTTString<'input>>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct UnsubscribeHeaderFlags;

#[derive(Debug, PartialEq, Clone, Default)]

pub struct UnsubscribeProperties<'input> {
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
