use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PubAck<'input> {
    pub packet_id: NonZero<u16>,
    pub reason_code: ReasonCode,
    
    pub properties: PubAckProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubAckHeaderFlags;

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubAckProperties<'input> {
    
    pub reason_string: Option<MQTTString<'input>>,
    
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
