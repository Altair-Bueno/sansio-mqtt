//! MQTT v5.0 `PUBREC` packet
//! ([§3.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901131)).
//!
//! First acknowledgement in the QoS 2 flow. Conformance:
//! `[MQTT-3.5.0-1]`.
use super::*;

/// MQTT v5.0 `PUBREC` packet
/// ([§3.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901131)).
///
/// Sent by the receiver of a QoS 2 `PUBLISH` as the first step of the
/// four-packet QoS 2 flow. Conformance: `[MQTT-3.5.0-1]`,
/// `[MQTT-3.5.2-1]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PubRec {
    /// Packet Identifier copied from the acknowledged `PUBLISH`
    /// ([§3.5.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901133)).
    pub packet_id: NonZero<u16>,
    /// Reason Code
    /// ([§3.5.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901134)).
    pub reason_code: PubRecReasonCode,
    /// `PUBREC` Properties
    /// ([§3.5.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901135)).
    pub properties: PubRecProperties,
}

/// Fixed-header flags byte for `PUBREC`
/// ([§3.5.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901132)).
///
/// MUST be `0b0000`; any other value is Malformed Packet.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PubRecHeaderFlags;

impl From<PubRecHeaderFlags> for u8 {
    fn from(_: PubRecHeaderFlags) -> u8 {
        0
    }
}

/// `PUBREC` Properties
/// ([§3.5.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901135)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct PubRecProperties {
    /// Reason String — optional human-readable diagnostic
    /// ([§3.5.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901137),
    /// [MQTT-3.5.2-2]).
    pub reason_string: Option<Utf8String>,
    /// User Properties
    /// ([§3.5.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901138)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl PubRecProperties {
    /// Returns `true` if no properties are set; permits omitting the
    /// properties section when [§3.5.2.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901136)
    /// allows.
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
