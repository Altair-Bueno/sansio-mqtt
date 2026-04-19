//! MQTT v5.0 `PUBREL` packet
//! ([§3.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901141)).
//!
//! Second response in the QoS 2 flow, sent by the originator of the
//! `PUBLISH`. Conformance: `[MQTT-3.6.0-1]`.
use super::*;

/// MQTT v5.0 `PUBREL` packet
/// ([§3.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901141)).
///
/// Sent in response to a `PUBREC`; third step of the four-packet QoS
/// 2 flow. Conformance: `[MQTT-3.6.0-1]`, `[MQTT-3.6.1-1]`,
/// `[MQTT-3.6.2-1]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PubRel {
    /// Packet Identifier matching the originating `PUBLISH` and
    /// `PUBREC` ([§3.6.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901143)).
    pub packet_id: NonZero<u16>,
    /// Reason Code
    /// ([§3.6.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901144)).
    pub reason_code: PubRelReasonCode,
    /// `PUBREL` Properties
    /// ([§3.6.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901145)).
    pub properties: PubRelProperties,
}

/// Fixed-header flags byte for `PUBREL`
/// ([§3.6.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901142)).
///
/// MUST be `0b0010`; any other value is Malformed Packet
/// ([MQTT-3.6.1-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PubRelHeaderFlags;

impl From<PubRelHeaderFlags> for u8 {
    fn from(_: PubRelHeaderFlags) -> u8 {
        0b0000_0010
    }
}

/// `PUBREL` Properties
/// ([§3.6.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901145)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct PubRelProperties {
    /// Reason String — optional human-readable diagnostic
    /// ([§3.6.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901147),
    /// [MQTT-3.6.2-2]).
    pub reason_string: Option<Utf8String>,
    /// User Properties
    /// ([§3.6.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901148)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

impl PubRelProperties {
    /// Returns `true` if no properties are set; permits omitting the
    /// properties section when [§3.6.2.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901146)
    /// allows.
    pub fn is_empty(&self) -> bool {
        self.reason_string.is_none() && self.user_properties.is_empty()
    }
}
