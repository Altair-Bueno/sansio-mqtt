//! MQTT v5.0 `PINGRESP` packet
//! ([§3.13](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901200)).
//!
//! Keep-alive response sent by the Server. Conformance:
//! `[MQTT-3.13.0-1]`.
use super::*;

/// MQTT v5.0 `PINGRESP` packet
/// ([§3.13](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901200)).
///
/// Sent by the Server in response to a [`PingReq`]; has no variable
/// header and no payload. Conformance: `[MQTT-3.13.0-1]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PingResp {}

/// Fixed-header flags byte for `PINGRESP`
/// ([§3.13.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901201)).
///
/// MUST be `0b0000`; any other value is Malformed Packet.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PingRespHeaderFlags;

impl From<PingRespHeaderFlags> for u8 {
    fn from(_: PingRespHeaderFlags) -> u8 {
        0
    }
}
