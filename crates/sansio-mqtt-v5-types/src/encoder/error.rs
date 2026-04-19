use core::convert::Infallible;
use core::num::TryFromIntError;

use encode::encoders::InsufficientSpace;

/// Error returned while encoding an MQTT v5.0 control packet.
///
/// Encoding never produces malformed packets on its own; the two
/// failure modes are (1) the packet would exceed the MQTT size limits
/// expressed in [§1.5 — Data representations](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901007)
/// (Variable Byte Integer cap of 268_435_455 for Remaining Length,
/// [MQTT-1.5.5-1]; and 65 535-byte limits on UTF-8 Strings and Binary
/// Data, [MQTT-1.5.4-1], [MQTT-1.5.6-1]) and (2) the destination
/// buffer does not have enough room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EncodeError {
    /// The packet could not be encoded because one of its length
    /// fields overflowed an MQTT wire integer — most commonly the
    /// Variable Byte Integer Remaining Length ([MQTT-1.5.5-1]) but
    /// also Two Byte Integer lengths for strings and binary data
    /// ([MQTT-1.5.4-1], [MQTT-1.5.6-1]).
    #[error("{0}. MQTT packet must be less than {variable_len} bytes in size with strings/byte arrays of at most {two_len} bytes long", two_len = u16::MAX, variable_len = 268_435_455)]
    PacketTooLarge(#[from] TryFromIntError),
    /// The caller-provided output buffer does not have enough room
    /// for the serialised packet.
    #[error(transparent)]
    InsufficientSpace(#[from] InsufficientSpace),
}

impl From<Infallible> for EncodeError {
    fn from(_: Infallible) -> Self {
        unreachable!("Infallible cannot be constructed")
    }
}
