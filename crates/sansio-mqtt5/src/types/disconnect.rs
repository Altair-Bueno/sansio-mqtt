use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Disconnect<'input> {
    pub reason_code: ReasonCode,

    pub properties: DisconnectProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct DisconnectHeaderFlags;

#[derive(Debug, PartialEq, Clone, Default)]

pub struct DisconnectProperties<'input> {
    pub session_expiry_interval: Option<u32>,

    pub reason_string: Option<MQTTString<'input>>,

    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,

    pub server_reference: Option<MQTTString<'input>>,
}
