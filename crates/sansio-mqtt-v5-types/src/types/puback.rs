//! MQTT v5.0 `PUBACK` packet
//! ([§3.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901121)).
//!
//! Acknowledgement to a QoS 1 [`Publish`]. Conformance:
//! `[MQTT-3.4.0-1]`.
use super::*;

/// MQTT v5.0 `PUBACK` packet
/// ([§3.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901121)).
///
/// Sent in response to a QoS 1 `PUBLISH`. Conformance:
/// `[MQTT-3.4.0-1]`, `[MQTT-3.4.2-1]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PubAck {
    /// Packet Identifier of the acknowledged `PUBLISH`
    /// ([§3.4.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901123),
    /// [MQTT-2.2.1-3]).
    pub packet_id: NonZero<u16>,
    /// Reason Code
    /// ([§3.4.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901124)).
    pub reason_code: PubAckReasonCode,
    /// `PUBACK` Properties
    /// ([§3.4.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901125)).
    pub properties: PubAckProperties,
}

/// Fixed-header flags byte for `PUBACK`
/// ([§3.4.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901122)).
///
/// MUST be `0b0000`; any other value is Malformed Packet.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PubAckHeaderFlags;

impl From<PubAckHeaderFlags> for u8 {
    fn from(_: PubAckHeaderFlags) -> u8 {
        0
    }
}

/// `PUBACK` Properties
/// ([§3.4.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901125)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct PubAckProperties {
    /// Reason String — optional human-readable diagnostic
    /// ([§3.4.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901127),
    /// [MQTT-3.4.2-2]).
    pub reason_string: Option<Utf8String>,
    /// User Properties
    /// ([§3.4.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901128)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl PubAckProperties {
    /// Returns `true` if no properties are set.
    ///
    /// Callers can use this to omit the properties section on the
    /// wire when permitted by [§3.4.2.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901126).
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
