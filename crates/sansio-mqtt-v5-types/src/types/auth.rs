use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct Auth {
    pub reason_code: AuthReasonCode,
    pub properties: AuthProperties,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AuthHeaderFlags;

impl From<AuthHeaderFlags> for u8 {
    fn from(_: AuthHeaderFlags) -> u8 {
        0
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct AuthProperties {
    pub reason_string: Option<Utf8String>,
    pub authentication: Option<AuthenticationKind>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl AuthProperties {
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none()
            && self.authentication.is_none()
            && self.user_properties.is_empty()
    }
}
