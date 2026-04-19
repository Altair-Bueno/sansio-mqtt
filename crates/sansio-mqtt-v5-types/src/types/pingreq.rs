//! MQTT v5.0 `PINGREQ` packet
//! ([§3.12](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901195)).
//!
//! Keep-alive request from Client to Server. Conformance:
//! `[MQTT-3.12.0-1]`.
use super::*;

/// MQTT v5.0 `PINGREQ` packet
/// ([§3.12](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901195)).
///
/// Has no variable header and no payload. Conformance:
/// `[MQTT-3.12.0-1]`, `[MQTT-3.12.4-1]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PingReq {}

/// Fixed-header flags byte for `PINGREQ`
/// ([§3.12.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901196)).
///
/// MUST be `0b0000`; any other value is Malformed Packet.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PingReqHeaderFlags;

impl From<PingReqHeaderFlags> for u8 {
    fn from(_: PingReqHeaderFlags) -> u8 {
        0
    }
}
