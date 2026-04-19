//! MQTT v5.0 `DISCONNECT` packet
//! ([§3.14](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901205)).
//!
//! Final packet exchanged by either peer to indicate the reason for
//! closing the Network Connection. Conformance: `[MQTT-3.14.0-1]`.
use super::*;

/// MQTT v5.0 `DISCONNECT` packet
/// ([§3.14](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901205)).
///
/// Conformance: `[MQTT-3.14.0-1]`, `[MQTT-3.14.1-1]`,
/// `[MQTT-3.14.2-1]`, `[MQTT-3.14.4-1]`, `[MQTT-3.14.4-2]`,
/// `[MQTT-3.14.4-3]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Disconnect {
    /// Disconnect Reason Code
    /// ([§3.14.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901208)).
    pub reason_code: DisconnectReasonCode,
    /// `DISCONNECT` Properties
    /// ([§3.14.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901209)).
    pub properties: DisconnectProperties,
}

/// Fixed-header flags byte for `DISCONNECT`
/// ([§3.14.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901206)).
///
/// MUST be `0b0000`; any other value is Malformed Packet
/// ([MQTT-3.14.1-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DisconnectHeaderFlags;

impl From<DisconnectHeaderFlags> for u8 {
    fn from(_: DisconnectHeaderFlags) -> u8 {
        0
    }
}

/// `DISCONNECT` Properties
/// ([§3.14.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901209)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct DisconnectProperties {
    /// Session Expiry Interval override
    /// ([§3.14.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901211),
    /// [MQTT-3.14.2-2]).
    pub session_expiry_interval: Option<u32>,
    /// Reason String — optional human-readable diagnostic
    /// ([§3.14.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901212),
    /// [MQTT-3.14.2-3]).
    pub reason_string: Option<Utf8String>,
    /// User Properties
    /// ([§3.14.2.2.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901213)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
    /// Server Reference used with UseAnotherServer / ServerMoved
    /// Reason Codes
    /// ([§3.14.2.2.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901214),
    /// [MQTT-3.14.2-4]).
    pub server_reference: Option<Utf8String>,
}

impl DisconnectProperties {
    /// Returns `true` if no properties are set; permits omitting the
    /// properties section when
    /// [§3.14.2.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901210)
    /// allows.
    pub fn is_empty(&self) -> bool {
        self.session_expiry_interval.is_none()
            && self.reason_string.is_none()
            && self.user_properties.is_empty()
            && self.server_reference.is_none()
    }
}
