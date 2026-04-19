//! MQTT v5.0 `UNSUBACK` packet
//! ([§3.11](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901187)).
//!
//! The Server's acknowledgement to [`Unsubscribe`]. Conformance:
//! `[MQTT-3.11.0-1]`, `[MQTT-3.11.3-1]`, `[MQTT-3.11.3-2]`.
use super::*;

/// MQTT v5.0 `UNSUBACK` packet
/// ([§3.11](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901187)).
///
/// The Reason Code list is ordered to match the Topic Filters of the
/// originating `UNSUBSCRIBE` ([MQTT-3.11.3-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UnsubAck {
    /// Packet Identifier copied from the acknowledged `UNSUBSCRIBE`
    /// ([§3.11.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901189)).
    pub packet_id: NonZero<u16>,
    /// `UNSUBACK` Properties
    /// ([§3.11.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901190)).
    pub properties: UnsubAckProperties,
    /// Reason Codes, one per Topic Filter in the original
    /// `UNSUBSCRIBE` ([§3.11.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901194),
    /// [MQTT-3.11.3-1], [MQTT-3.11.3-2]).
    pub reason_codes: Vec<UnsubAckReasonCode>,
}

/// Fixed-header flags byte for `UNSUBACK`
/// ([§3.11.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901188)).
///
/// MUST be `0b0000`; any other value is Malformed Packet.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UnsubAckHeaderFlags;

impl From<UnsubAckHeaderFlags> for u8 {
    fn from(_: UnsubAckHeaderFlags) -> u8 {
        0
    }
}

/// `UNSUBACK` Properties
/// ([§3.11.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901190)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct UnsubAckProperties {
    /// Reason String — optional human-readable diagnostic
    /// ([§3.11.2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901192)).
    pub reason_string: Option<Utf8String>,
    /// User Properties
    /// ([§3.11.2.1.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901193)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
