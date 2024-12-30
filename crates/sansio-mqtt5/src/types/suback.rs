use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct SubAck<'input> {
    pub packet_id: NonZero<u16>,
    
    pub properties: SubAckProperties<'input>,
    pub reason_codes: Vec<ReasonCode>,
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct SubAckProperties<'input> {
    
    pub reason_string: Option<MQTTString<'input>>,
    
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct SubAckHeaderFlags;
