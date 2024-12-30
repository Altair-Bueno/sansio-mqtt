use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PubComp<'input> {
    pub packet_id: NonZero<u16>,
    pub reason_code: ReasonCode,
    
    pub properties: PubCompProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubCompHeaderFlags;

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubCompProperties<'input> {
    
    pub reason_string: Option<MQTTString<'input>>,
    
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
