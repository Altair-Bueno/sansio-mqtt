use super::*;

#[nutype::nutype(
    validate(predicate = Utf8String::is_valid),
    // new_unchecked,
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord,AsRef,Deref, Hash, Display, Borrow, TryFrom, Into),
)]
pub struct Utf8String<'input>(Cow<'input, str>);

#[nutype::nutype(
    validate(predicate = Topic::is_valid),
    // new_unchecked,
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord,AsRef,Deref, Hash, Display, Borrow, TryFrom, Into),
)]
pub struct Topic<'input>(Utf8String<'input>);

impl<'input> Utf8String<'input> {
    #[inline]
    fn is_valid(s: &Cow<'_, str>) -> bool {
        !s.as_ref().chars().any(Self::is_invalid_character)
    }

    #[inline]
    const fn is_invalid_character(c: char) -> bool {
        matches!(
            c,
            // Control characters
            '\u{0001}'..='\u{001F}' |
            '\u{007F}'..='\u{009F}' |
            // Null character
            '\0' |
            // Non-characters
            '\u{FFFE}'|
            '\u{FFFF}'
        )
    }
}

impl<'input> Topic<'input> {
    #[inline(always)]
    fn is_valid(s: &Utf8String<'_>) -> bool {
        !s.contains(['#', '+'])
    }
}

impl TryFrom<String> for Utf8String<'static> {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value.into())
    }
}

impl<'input> TryFrom<&'input str> for Utf8String<'input> {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: &'input str) -> Result<Self, Self::Error> {
        Self::try_new(value.into())
    }
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord)]
pub enum RetainHandling {
    SendRetained = 0,
    SendRetainedIfSubscriptionDoesNotExist = 1,
    DoNotSend = 2,
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord)]
pub enum FormatIndicator {
    Unspecified = 0,
    Utf8 = 1,
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord)]
pub enum Qos {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord)]
pub enum MaximumQoS {
    AtMostOnce = 0,
    AtLeastOnce = 1,
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord)]
pub enum GuaranteedQoS {
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

impl From<RetainHandling> for u8 {
    #[inline]
    fn from(value: RetainHandling) -> Self {
        value as u8
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Error)]
#[error("Invalid retain handling value: {value}")]
#[repr(transparent)]
pub struct InvalidRetainHandlingError {
    pub value: u8,
}

impl TryFrom<u8> for RetainHandling {
    type Error = InvalidRetainHandlingError;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::iter()
            .find(|v| *v as u8 == value)
            .ok_or(InvalidRetainHandlingError { value })
    }
}
impl From<FormatIndicator> for u8 {
    #[inline]
    fn from(value: FormatIndicator) -> Self {
        value as u8
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Error)]
#[error("Unknown format indicator: {format_indicator}")]
#[repr(transparent)]
pub struct UnknownFormatIndicatorError {
    pub format_indicator: u8,
}

impl TryFrom<u8> for FormatIndicator {
    type Error = UnknownFormatIndicatorError;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::iter()
            .find(|v| *v as u8 == value)
            .ok_or(UnknownFormatIndicatorError {
                format_indicator: value,
            })
    }
}

impl From<Qos> for u8 {
    #[inline]
    fn from(value: Qos) -> Self {
        value as u8
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Error)]
#[error("Invalid QoS value: {qos}")]
#[repr(transparent)]
pub struct InvalidQosError {
    pub qos: u8,
}

impl TryFrom<u8> for Qos {
    type Error = InvalidQosError;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::iter()
            .find(|v| *v as u8 == value)
            .ok_or(InvalidQosError { qos: value })
    }
}

impl From<MaximumQoS> for u8 {
    #[inline]
    fn from(value: MaximumQoS) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for MaximumQoS {
    type Error = InvalidQosError;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::iter()
            .find(|v| *v as u8 == value)
            .ok_or(InvalidQosError { qos: value })
    }
}

impl From<MaximumQoS> for Qos {
    #[inline]
    fn from(value: MaximumQoS) -> Self {
        Self::try_from(u8::from(value)).expect("Should be a valid QoS value")
    }
}

impl TryFrom<Qos> for MaximumQoS {
    type Error = InvalidQosError;

    #[inline]
    fn try_from(value: Qos) -> Result<Self, Self::Error> {
        Self::try_from(u8::from(value))
    }
}

impl From<GuaranteedQoS> for u8 {
    #[inline]
    fn from(value: GuaranteedQoS) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for GuaranteedQoS {
    type Error = InvalidQosError;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::iter()
            .find(|v| *v as u8 == value)
            .ok_or(InvalidQosError { qos: value })
    }
}

impl From<GuaranteedQoS> for Qos {
    #[inline]
    fn from(value: GuaranteedQoS) -> Self {
        Self::try_from(u8::from(value)).expect("Should be a valid QoS value")
    }
}

impl TryFrom<Qos> for GuaranteedQoS {
    type Error = InvalidQosError;

    #[inline]
    fn try_from(value: Qos) -> Result<Self, Self::Error> {
        Self::try_from(u8::from(value))
    }
}
