use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Auth<'input> {
    pub reason_code: ReasonCode,

    pub properties: AuthProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct AuthHeaderFlags;

impl From<AuthHeaderFlags> for u8 {
    fn from(_: AuthHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]

pub struct AuthProperties<'input> {
    pub reason_string: Option<MQTTString<'input>>,
    pub authentication: Option<AuthenticationKind<'input>>,
    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}

impl AuthProperties<'_> {
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none()
            && self.authentication.is_none()
            && self.user_properties.is_empty()
    }
}
