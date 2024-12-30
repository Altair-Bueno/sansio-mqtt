use super::*;
#[derive(Debug, PartialEq, Clone)]

pub struct PubRel<'input> {
    pub packet_id: NonZero<u16>,
    pub reason_code: ReasonCode,

    pub properties: PubRelProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubRelHeaderFlags;

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubRelProperties<'input> {
    pub reason_string: Option<MQTTString<'input>>,

    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
