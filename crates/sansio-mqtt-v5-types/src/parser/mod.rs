mod basic;
mod properties;

pub use basic::*;

mod auth;
mod connack;
mod connect;
mod control_packet;
mod disconnect;
mod pingreq;
mod pingresp;
mod puback;
mod pubcomp;
mod publish;
mod pubrec;
mod pubrel;
mod reserved;
mod suback;
mod subscribe;
mod unsuback;
mod unsubscribe;

use super::*;
use alloc::vec::Vec;
use core::any::type_name;
use core::num::TryFromIntError;
use core::str::Utf8Error;
use winnow::binary;
use winnow::binary::bits;
use winnow::combinator;
use winnow::error::*;
use winnow::prelude::*;
use winnow::stream::*;
use winnow::token;

/// Caller-provided limits applied by the parsers to guard against
/// resource-exhaustion when decoding untrusted input.
///
/// MQTT v5 allows implementation-defined upper bounds for several
/// fields — see for example the Maximum Packet Size property
/// ([§3.1.2.11.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901050),
/// [MQTT-3.1.2-25]) and the limits implicit in
/// [§1.5 Data representations](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901007).
/// These settings translate those spec-level knobs into concrete
/// parser-side ceilings. Use [`ParserSettings::new`] for sensible
/// defaults or [`ParserSettings::unlimited`] to disable every cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserSettings {
    /// Maximum number of bytes accepted for a single UTF-8 Encoded
    /// String ([§1.5.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901010),
    /// [MQTT-1.5.4-1]). MUST be `<= u16::MAX`.
    pub max_bytes_string: u16,
    /// Maximum number of bytes accepted for a single Binary Data
    /// value ([§1.5.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901012),
    /// [MQTT-1.5.6-1]). MUST be `<= u16::MAX`.
    pub max_bytes_binary_data: u16,
    /// Maximum value accepted for the Remaining Length Variable Byte
    /// Integer ([§1.5.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901011),
    /// [MQTT-1.5.5-1]). Spec ceiling is `268_435_455`.
    pub max_remaining_bytes: u64,
    /// Maximum number of Topic Filters allowed in a single
    /// `SUBSCRIBE` payload
    /// ([§3.8.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901168)).
    pub max_subscriptions_len: u32,
    /// Maximum number of User Property entries allowed in any single
    /// property section
    /// ([§2.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901029)).
    pub max_user_properties_len: usize,
    /// Maximum number of Subscription Identifiers that a single
    /// `PUBLISH` is allowed to carry
    /// ([§3.3.2.3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901117)).
    pub max_subscription_identifiers_len: usize,
}

impl ParserSettings {
    /// Returns a [`ParserSettings`] preset with conservative
    /// defaults: 5 KiB strings and binary blobs, 1 MiB of Remaining
    /// Length, 32 subscriptions, 32 User Properties, 32 Subscription
    /// Identifiers.
    #[inline]
    pub const fn new() -> Self {
        Self {
            max_bytes_string: 5 * 1024,           // 5 KiB
            max_bytes_binary_data: 5 * 1024,      // 5 KiB
            max_remaining_bytes: 1024 * 1024,     // 1 MiB
            max_subscriptions_len: 32,            // 32 subscriptions
            max_user_properties_len: 32,          // 32 properties
            max_subscription_identifiers_len: 32, // 32 identifiers
        }
    }

    /// Returns a [`ParserSettings`] preset with every limit raised to
    /// the maximum representable value. Useful for tests; not
    /// recommended for production as it disables the resource-
    /// exhaustion guard rails.
    #[inline]
    pub const fn unlimited() -> Self {
        Self {
            max_bytes_string: u16::MAX,
            max_bytes_binary_data: u16::MAX,
            max_remaining_bytes: u64::MAX,
            max_subscriptions_len: u32::MAX,
            max_user_properties_len: usize::MAX,
            max_subscription_identifiers_len: usize::MAX,
        }
    }
}

impl Default for ParserSettings {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
