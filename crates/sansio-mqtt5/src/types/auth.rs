use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Auth<'input> {
    pub reason_code: ReasonCode,

    pub properties: AuthProperties<'input>,
}

#[derive(Debug, PartialEq, Clone)]

pub struct AuthHeaderFlags;

#[derive(Debug, PartialEq, Clone, Default)]

pub struct AuthProperties<'input> {
    pub reason_string: Option<MQTTString<'input>>,

    pub authentication: Option<AuthenticationKind<'input>>,

    pub user_properties: Vec<(MQTTString<'input>, MQTTString<'input>)>,
}
