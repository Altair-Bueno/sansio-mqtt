use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PubRec<'input> {
    pub packet_id: NonZero<u16>,
    pub reason_code: ReasonCode,
    
    pub properties: PubRecProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubRecHeaderFlags;

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubRecProperties<'input> {
    
    pub reason_string: Option<MQTTString<'input>>,
    
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
