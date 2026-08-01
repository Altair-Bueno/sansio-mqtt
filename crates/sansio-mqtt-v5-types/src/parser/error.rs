use super::*;

/// Concrete decoding error for MQTT v5.0 control packets.
///
/// The parsers in this crate are generic over their error type so that
/// callers can trade diagnostics for size. This type is the default
/// choice: it preserves *which* rule was violated, which is precisely
/// the information needed to pick the Reason Code for the DISCONNECT
/// that [§4.13 — Handling errors](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901252)
/// requires. Use [`DecodeError::disconnect_reason_code`] to obtain it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The bytes at this position do not match the grammar for the
    /// packet being decoded — a wrong length, a missing required
    /// field, or trailing bytes after the packet body.
    ///
    /// This is the error produced by
    /// [`winnow::error::ParserError::from_input`], so it also covers
    /// structural failures that carry no richer cause.
    #[error("malformed packet: bytes do not match the expected structure")]
    Structure,

    /// A UTF-8 Encoded String was not well-formed UTF-8
    /// ([§1.5.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901010),
    /// [MQTT-1.5.4-1]).
    #[error(transparent)]
    Utf8(#[from] Utf8Error),

    /// A UTF-8 Encoded String violated an MQTT string invariant — too
    /// long, contains `U+0000`, or contains a disallowed control
    /// character or non-character ([MQTT-1.5.4-1], [MQTT-1.5.4-2],
    /// [MQTT-1.5.4-3]).
    #[error(transparent)]
    Utf8String(#[from] Utf8StringError),

    /// A Binary Data field exceeded the `u16::MAX` wire limit
    /// ([§1.5.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901012),
    /// [MQTT-1.5.6-1]).
    #[error(transparent)]
    BinaryData(#[from] BinaryDataError),

    /// A Topic Name contained a wildcard character
    /// ([§4.7.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901242),
    /// [MQTT-4.7.1-1], [MQTT-4.7.1-2]).
    #[error(transparent)]
    Topic(#[from] TopicError),

    /// A property identifier was not one defined by
    /// [§2.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901029).
    #[error(transparent)]
    InvalidPropertyType(#[from] InvalidPropertyTypeError),

    /// The property section was structurally valid but violated a rule
    /// about which properties may appear, or how often.
    #[error(transparent)]
    Properties(#[from] PropertiesError),

    /// A QoS field held a value other than 0, 1 or 2
    /// ([§3.3.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901103)).
    #[error(transparent)]
    InvalidQos(#[from] InvalidQosError),

    /// A Payload Format Indicator held a value other than 0 or 1
    /// ([§3.3.2.3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901111)).
    #[error(transparent)]
    UnknownFormatIndicator(#[from] UnknownFormatIndicatorError),

    /// A Retain Handling field held the value 3
    /// ([§3.8.3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901169)).
    #[error(transparent)]
    InvalidRetainHandling(#[from] InvalidRetainHandlingError),

    /// A Reason Code was not defined for the packet type carrying it
    /// ([§2.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901031)).
    #[error(transparent)]
    InvalidReasonCode(#[from] InvalidReasonCode),

    /// The Control Packet Type nibble was 0 (Reserved, forbidden) or
    /// otherwise not a defined type
    /// ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)).
    #[error(transparent)]
    InvalidControlPacketType(#[from] InvalidControlPacketTypeError),

    /// A field that the spec requires to be non-zero was zero, or an
    /// integer did not fit its target width.
    ///
    /// The spec calls out a zero value as a Protocol Error for Packet
    /// Identifier ([MQTT-2.2.1-3]), Topic Alias
    /// ([§3.3.2.3.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901113)),
    /// Receive Maximum
    /// ([§3.1.2.11.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901049))
    /// and Subscription Identifier
    /// ([§3.3.2.3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901117)).
    #[error(transparent)]
    OutOfRange(#[from] TryFromIntError),
}

impl DecodeError {
    /// Returns the Reason Code to send in a DISCONNECT for this error
    /// ([§4.13](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901252)).
    ///
    /// The classification follows the definitions in §4.13: a
    /// **Malformed Packet** (`0x81`) is one that "cannot be parsed
    /// according to this specification", whereas a **Protocol Error**
    /// (`0x82`) is detected after parsing succeeds, when a value "is
    /// not allowed by the protocol or is inconsistent with the state of
    /// the Client or Server".
    ///
    /// Two mappings are worth calling out because the spec states them
    /// explicitly, and they differ despite both concerning properties:
    ///
    /// * A property identifier that is *not valid for its packet type*
    ///   is a Malformed Packet — "A Control Packet which contains an
    ///   Identifier which is not valid for its packet type, or contains
    ///   a value not of the specified data type, is a Malformed Packet"
    ///   ([§2.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901029)).
    /// * A property *repeated* when it may appear at most once is a
    ///   Protocol Error — stated per property, e.g. "It is a Protocol
    ///   Error to include the Session Expiry Interval more than once"
    ///   ([§3.1.2.11.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901048)).
    ///
    /// Limits that come from [`ParserSettings`] rather than from the
    /// spec map to `0x83` (Implementation specific error), since no
    /// spec rule was violated — the packet merely exceeded a ceiling
    /// this implementation chose.
    ///
    /// For [`DecodeError::Topic`] and
    /// [`DecodeError::InvalidRetainHandling`] the spec does not label
    /// the receive-side failure explicitly; both values parse cleanly
    /// and are rejected for being disallowed, so by the §4.13
    /// definition this crate classifies them as Protocol Errors.
    #[inline]
    pub const fn disconnect_reason_code(&self) -> DisconnectReasonCode {
        match self {
            // Cannot be parsed per the specification.
            Self::Structure
            | Self::Utf8(_)
            | Self::Utf8String(_)
            | Self::BinaryData(_)
            | Self::InvalidPropertyType(_)
            | Self::InvalidQos(_)
            | Self::UnknownFormatIndicator(_)
            | Self::InvalidReasonCode(_)
            | Self::InvalidControlPacketType(_) => DisconnectReasonCode::MalformedPacket,
            // Parsed, but the value is not allowed by the protocol.
            Self::Topic(_) | Self::InvalidRetainHandling(_) | Self::OutOfRange(_) => {
                DisconnectReasonCode::ProtocolError
            }
            Self::Properties(PropertiesError::UnsupportedProperty(_)) => {
                DisconnectReasonCode::MalformedPacket
            }
            Self::Properties(
                PropertiesError::DuplicatedProperty(_)
                | PropertiesError::MissingAuthenticationMethod(_),
            ) => DisconnectReasonCode::ProtocolError,
            Self::Properties(
                PropertiesError::TooManyUserProperties(_)
                | PropertiesError::TooManySubscriptionIdentifiers(_),
            ) => DisconnectReasonCode::ImplementationSpecificError,
        }
    }

    /// Returns `true` when this error is a Malformed Packet
    /// (Reason Code `0x81`) rather than a Protocol Error.
    #[inline]
    pub const fn is_malformed_packet(&self) -> bool {
        matches!(
            self.disconnect_reason_code(),
            DisconnectReasonCode::MalformedPacket
        )
    }
}

impl<Input: Stream> ParserError<Input> for DecodeError {
    type Inner = Self;

    #[inline]
    fn from_input(_: &Input) -> Self {
        Self::Structure
    }

    #[inline]
    fn into_inner(self) -> Result<Self::Inner, Self> {
        Ok(self)
    }

    /// Keeps whichever branch reported a specific cause.
    ///
    /// [`winnow::combinator::alt`] is used for sections the spec allows
    /// to be truncated (for example the Reason Code and properties of a
    /// PUBACK). Preferring a classified cause over [`Self::Structure`]
    /// means the surviving error explains the real violation instead of
    /// the last branch that happened to fail.
    #[inline]
    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Structure, other) => other,
            (this, Self::Structure) => this,
            (_, other) => other,
        }
    }

    /// Only a structural mismatch is recoverable.
    ///
    /// Every other variant names a rule the packet definitively broke,
    /// and a violated rule cannot become valid by trying a different
    /// branch. This matters for the property section, which is decoded
    /// with [`winnow::combinator::repeat`]: were these errors
    /// backtrackable, `repeat` would read a rejected property as "end
    /// of the list" and the real cause would be replaced by the
    /// subsequent end-of-input mismatch.
    ///
    /// The truncatable Reason Code / properties tail of PUBACK, PUBREC,
    /// PUBREL, PUBCOMP, DISCONNECT and AUTH still works: the branch
    /// that assumes an omitted tail fails on end-of-input, which is
    /// [`Self::Structure`] and therefore recoverable.
    #[inline]
    fn is_backtrack(&self) -> bool {
        matches!(self, Self::Structure)
    }
}

/// Diagnostic context is discarded: the variant already identifies the
/// violated rule, which is what callers act on.
impl<Input: Stream> AddContext<Input, StrContext> for DecodeError {}

impl<Input, External> FromExternalError<Input, External> for DecodeError
where
    Self: From<External>,
{
    #[inline]
    fn from_external_error(_: &Input, error: External) -> Self {
        Self::from(error)
    }
}

impl ErrorConvert<DecodeError> for DecodeError {
    #[inline]
    fn convert(self) -> Self {
        self
    }
}
