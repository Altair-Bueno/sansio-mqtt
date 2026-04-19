//! MQTT v5.0 `PUBCOMP` packet
//! ([§3.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901151)).
//!
//! Final acknowledgement in the QoS 2 flow. Conformance:
//! `[MQTT-3.7.0-1]`.
use super::*;

/// MQTT v5.0 `PUBCOMP` packet
/// ([§3.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901151)).
///
/// Sent in response to a `PUBREL`; fourth and final step of the
/// four-packet QoS 2 flow. Conformance: `[MQTT-3.7.0-1]`,
/// `[MQTT-3.7.2-1]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PubComp {
    /// Packet Identifier matching the originating `PUBLISH` and
    /// `PUBREL` ([§3.7.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901153)).
    pub packet_id: NonZero<u16>,
    /// Reason Code
    /// ([§3.7.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901154)).
    pub reason_code: PubCompReasonCode,
    /// `PUBCOMP` Properties
    /// ([§3.7.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901155)).
    pub properties: PubCompProperties,
}

/// Fixed-header flags byte for `PUBCOMP`
/// ([§3.7.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901152)).
///
/// MUST be `0b0000`; any other value is Malformed Packet.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PubCompHeaderFlags;

impl From<PubCompHeaderFlags> for u8 {
    fn from(_: PubCompHeaderFlags) -> u8 {
        0
    }
}

/// `PUBCOMP` Properties
/// ([§3.7.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901155)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct PubCompProperties {
    /// Reason String — optional human-readable diagnostic
    /// ([§3.7.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901157),
    /// [MQTT-3.7.2-2]).
    pub reason_string: Option<Utf8String>,
    /// User Properties
    /// ([§3.7.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901158)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl PubCompProperties {
    /// Returns `true` if no properties are set; permits omitting the
    /// properties section when [§3.7.2.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901156)
    /// allows.
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
