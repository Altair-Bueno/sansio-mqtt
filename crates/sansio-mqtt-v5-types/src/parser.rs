mod basic;
mod bytes_source;
mod error;
mod properties;

pub use basic::*;
pub use bytes_source::BytesSource;
pub use error::DecodeError;

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
mod suback;
mod subscribe;
mod unsuback;
mod unsubscribe;

use crate::AuthReasonCode;
use crate::BinaryData;
use crate::BinaryDataError;
use crate::ConnackReasonCode;
use crate::ConnectReasonCode;
use crate::ControlPacketType;
use crate::DisconnectReasonCode;
use crate::DuplicatedPropertyError;
use crate::FormatIndicator;
use crate::InvalidControlPacketTypeError;
use crate::InvalidPropertyTypeError;
use crate::InvalidQosError;
use crate::InvalidReasonCode;
use crate::InvalidRetainHandlingError;
use crate::Payload;
use crate::PropertiesError;
use crate::Property;
use crate::PropertyType;
use crate::PubAckProperties;
use crate::PubAckReasonCode;
use crate::PubCompProperties;
use crate::PubCompReasonCode;
use crate::PubRecProperties;
use crate::PubRecReasonCode;
use crate::PubRelProperties;
use crate::PubRelReasonCode;
use crate::PublishReasonCode;
use crate::Qos;
use crate::RetainHandling;
use crate::SubAckProperties;
use crate::SubAckReasonCode;
use crate::Subscription;
use crate::TooManyUserPropertiesError;
use crate::Topic;
use crate::TopicError;
use crate::UnknownFormatIndicatorError;
use crate::UnsubAckProperties;
use crate::UnsubAckReasonCode;
use crate::UnsupportedPropertyError;
use crate::Utf8String;
use crate::Utf8StringError;
use alloc::vec::Vec;
use core::any::type_name;
use core::num::TryFromIntError;
use core::str::Utf8Error;
use winnow::binary;
use winnow::binary::bits;
use winnow::binary::bits::Bits;
use winnow::combinator;
use winnow::error::AddContext;
use winnow::error::ErrorConvert;
use winnow::error::FromExternalError;
use winnow::error::ParserError;
use winnow::error::Result;
use winnow::error::StrContext;
use winnow::error::StrContextValue;
use winnow::prelude::*;
use winnow::stream::Stream;
use winnow::stream::StreamIsPartial;
use winnow::stream::UpdateSlice;
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
/// parser-side ceilings. Use [`ParserSettings::default`] for
/// conservative defaults, [`ParserSettings::builder`] to override some of
/// them, or [`ParserSettings::unlimited`] to disable every cap.
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct ParserSettings {
    /// Maximum number of bytes accepted for a single UTF-8 Encoded
    /// String ([§1.5.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901010),
    /// [MQTT-1.5.4-1]). Defaults to 5 KiB.
    #[builder(default = 5 * 1024)]
    pub max_bytes_string: u16,
    /// Maximum number of bytes accepted for a single Binary Data
    /// value ([§1.5.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901012),
    /// [MQTT-1.5.6-1]). Defaults to 5 KiB.
    #[builder(default = 5 * 1024)]
    pub max_bytes_binary_data: u16,
    /// Maximum value accepted for the Remaining Length Variable Byte
    /// Integer ([§1.5.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901011),
    /// [MQTT-1.5.5-1]). Defaults to 1 MiB.
    ///
    /// The encoding cannot represent more than `268_435_455`, so any larger
    /// value behaves as no limit.
    #[builder(default = 1024 * 1024)]
    pub max_remaining_bytes: u32,
    /// Maximum number of Topic Filters allowed in a single
    /// `SUBSCRIBE` payload
    /// ([§3.8.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901168)).
    /// Defaults to 32.
    #[builder(default = 32)]
    pub max_subscriptions_len: u32,
    /// Maximum number of User Property entries allowed in any single
    /// property section
    /// ([§2.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901029)).
    /// Defaults to 32.
    #[builder(default = 32)]
    pub max_user_properties_len: usize,
    /// Maximum number of Subscription Identifiers that a single
    /// `PUBLISH` is allowed to carry
    /// ([§3.3.2.3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901117)).
    /// Defaults to 32.
    #[builder(default = 32)]
    pub max_subscription_identifiers_len: usize,
}

impl ParserSettings {
    /// Returns a [`ParserSettings`] preset with every limit raised to
    /// the maximum representable value. Useful for tests; not
    /// recommended for production as it disables the resource-
    /// exhaustion guard rails.
    #[inline]
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            max_bytes_string: u16::MAX,
            max_bytes_binary_data: u16::MAX,
            max_remaining_bytes: u32::MAX,
            max_subscriptions_len: u32::MAX,
            max_user_properties_len: usize::MAX,
            max_subscription_identifiers_len: usize::MAX,
        }
    }
}

impl Default for ParserSettings {
    #[inline]
    fn default() -> Self {
        Self::builder().build()
    }
}
