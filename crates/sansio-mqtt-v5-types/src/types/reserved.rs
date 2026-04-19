//! Reserved control packet placeholder
//! ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)).
//!
//! Control Packet Type 0 is reserved; receiving one is a Malformed
//! Packet ([MQTT-2.1.3-1]). The struct exists so the parser can
//! surface the value through the [`ControlPacket`] enum without
//! crashing.
use super::*;

/// Placeholder for the reserved Control Packet Type 0
/// ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)).
///
/// The spec says type 0 is reserved and MUST NOT be used; the parser
/// decodes it only so that the error path can report the situation.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Reserved {}

/// Fixed-header flags byte for the Reserved packet type
/// ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)).
///
/// MUST be `0b0000`; any other value is Malformed Packet.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ReservedHeaderFlags;

impl From<ReservedHeaderFlags> for u8 {
    fn from(_: ReservedHeaderFlags) -> u8 {
        0
    }
}
