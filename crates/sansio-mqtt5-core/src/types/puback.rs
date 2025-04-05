use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PubAck<'input> {
    pub packet_id: NonZero<u16>,
    pub reason_code: PubAckReasonCode,
    pub properties: PubAckProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct PubAckHeaderFlags;

impl From<PubAckHeaderFlags> for u8 {
    fn from(_: PubAckHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct PubAckProperties<'input> {
    pub reason_string: Option<MQTTString<'input>>,

    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}

impl PubAckProperties<'_> {
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
