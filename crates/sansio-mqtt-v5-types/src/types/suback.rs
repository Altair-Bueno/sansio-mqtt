//! MQTT v5.0 `SUBACK` packet
//! ([§3.9](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901171)).
//!
//! The Server's acknowledgement to a [`Subscribe`]. Conformance:
//! `[MQTT-3.9.0-1]`, `[MQTT-3.9.3-1]`, `[MQTT-3.9.3-2]`.
use super::*;

/// MQTT v5.0 `SUBACK` packet
/// ([§3.9](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901171)).
///
/// The Reason Code list is ordered to match the Topic Filters of the
/// originating `SUBSCRIBE` ([MQTT-3.9.3-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SubAck {
    /// Packet Identifier copied from the acknowledged `SUBSCRIBE`
    /// ([§3.9.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901173)).
    pub packet_id: NonZero<u16>,
    /// `SUBACK` Properties
    /// ([§3.9.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901174)).
    pub properties: SubAckProperties,
    /// Reason Codes, one per Topic Filter in the original
    /// `SUBSCRIBE` ([§3.9.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901178),
    /// [MQTT-3.9.3-1], [MQTT-3.9.3-2]).
    pub reason_codes: Vec<SubAckReasonCode>,
}

/// `SUBACK` Properties
/// ([§3.9.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901174)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct SubAckProperties {
    /// Reason String — optional human-readable diagnostic
    /// ([§3.9.2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901176)).
    pub reason_string: Option<Utf8String>,
    /// User Properties
    /// ([§3.9.2.1.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901177)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

/// Fixed-header flags byte for `SUBACK`
/// ([§3.9.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901172)).
///
/// MUST be `0b0000`; any other value is Malformed Packet.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SubAckHeaderFlags;

impl From<SubAckHeaderFlags> for u8 {
    fn from(_: SubAckHeaderFlags) -> u8 {
        0
    }
}
