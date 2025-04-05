use super::*;
#[derive(Debug, PartialEq, Clone)]

pub struct UnsubAck<'input> {
    pub packet_id: NonZero<u16>,
    pub properties: UnsubAckProperties<'input>,
    pub reason_codes: Vec<UnsubAckReasonCode>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct UnsubAckHeaderFlags;

impl From<UnsubAckHeaderFlags> for u8 {
    fn from(_: UnsubAckHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct UnsubAckProperties<'input> {
    pub reason_string: Option<MQTTString<'input>>,

    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
