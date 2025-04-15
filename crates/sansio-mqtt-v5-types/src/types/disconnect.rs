use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Disconnect<'input> {
    pub reason_code: DisconnectReasonCode,
    pub properties: DisconnectProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct DisconnectHeaderFlags;

impl From<DisconnectHeaderFlags> for u8 {
    fn from(_: DisconnectHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct DisconnectProperties<'input> {
    pub session_expiry_interval: Option<u32>,

    pub reason_string: Option<Utf8String<'input>>,

    pub user_properties: Vec<(Utf8String<'input>, Utf8String<'input>)>,

    pub server_reference: Option<Utf8String<'input>>,
}

impl DisconnectProperties<'_> {
    pub fn is_empty(&self) -> bool {
        self.session_expiry_interval.is_none()
            && self.reason_string.is_none()
            && self.user_properties.is_empty()
            && self.server_reference.is_none()
    }
}
