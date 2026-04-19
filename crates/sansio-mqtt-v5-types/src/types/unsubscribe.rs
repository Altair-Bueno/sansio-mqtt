//! MQTT v5.0 `UNSUBSCRIBE` packet
//! ([§3.10](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901179)).
//!
//! Removes one or more existing subscriptions.
//! Conformance: `[MQTT-3.10.0-1]`, `[MQTT-3.10.1-1]`,
//! `[MQTT-3.10.3-1]`, `[MQTT-3.10.3-2]`.
use super::*;

/// MQTT v5.0 `UNSUBSCRIBE` packet
/// ([§3.10](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901179)).
///
/// At least one Topic Filter is required ([MQTT-3.10.3-1]); additional
/// filters are stored in [`Unsubscribe::extra_filters`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Unsubscribe {
    /// Packet Identifier
    /// ([§3.10.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901181),
    /// [MQTT-2.2.1-3]).
    pub packet_id: NonZero<u16>,
    /// `UNSUBSCRIBE` Properties
    /// ([§3.10.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901182)).
    pub properties: UnsubscribeProperties,
    /// First Topic Filter to unsubscribe from
    /// ([MQTT-3.10.3-1]).
    pub filter: Utf8String,
    /// Additional Topic Filters beyond the first, in wire order.
    pub extra_filters: Vec<Utf8String>,
}

/// Fixed-header flags byte for `UNSUBSCRIBE`
/// ([§3.10.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901180)).
///
/// MUST be `0b0010`; any other value is Malformed Packet
/// ([MQTT-3.10.1-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UnsubscribeHeaderFlags;

impl From<UnsubscribeHeaderFlags> for u8 {
    fn from(_: UnsubscribeHeaderFlags) -> u8 {
        0b0000_0010
    }
}

/// `UNSUBSCRIBE` Properties
/// ([§3.10.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901182)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct UnsubscribeProperties {
    /// User Properties
    /// ([§3.10.2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901183)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
