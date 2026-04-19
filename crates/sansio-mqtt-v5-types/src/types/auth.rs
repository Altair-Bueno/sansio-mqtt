//! MQTT v5.0 `AUTH` packet
//! ([§3.15](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901217)).
//!
//! Exchanges enhanced-authentication data between Client and Server.
//! Conformance: `[MQTT-3.15.0-1]`, `[MQTT-3.15.1-1]`,
//! `[MQTT-3.15.2-1]`.
use super::*;

/// MQTT v5.0 `AUTH` packet
/// ([§3.15](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901217)).
///
/// Drives the enhanced authentication exchange introduced in v5.0.
/// Conformance: `[MQTT-3.15.0-1]`, `[MQTT-3.15.1-1]`,
/// `[MQTT-3.15.2-1]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Auth {
    /// Authenticate Reason Code
    /// ([§3.15.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901220)).
    pub reason_code: AuthReasonCode,
    /// `AUTH` Properties
    /// ([§3.15.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901221)).
    pub properties: AuthProperties,
}

/// Fixed-header flags byte for `AUTH`
/// ([§3.15.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901218)).
///
/// MUST be `0b0000`; any other value is Malformed Packet
/// ([MQTT-3.15.1-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AuthHeaderFlags;

impl From<AuthHeaderFlags> for u8 {
    fn from(_: AuthHeaderFlags) -> u8 {
        0
    }
}

/// `AUTH` Properties
/// ([§3.15.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901221)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct AuthProperties {
    /// Reason String — optional human-readable diagnostic
    /// ([§3.15.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901224)).
    pub reason_string: Option<Utf8String>,
    /// Authentication Method and optional Authentication Data
    /// ([§3.15.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901223),
    /// [MQTT-3.15.2-2]).
    pub authentication: Option<AuthenticationKind>,
    /// User Properties
    /// ([§3.15.2.2.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901225)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl AuthProperties {
    /// Returns `true` if no properties are set; permits omitting the
    /// properties section when
    /// [§3.15.2.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901222)
    /// allows.
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none()
            && self.authentication.is_none()
            && self.user_properties.is_empty()
    }
}
