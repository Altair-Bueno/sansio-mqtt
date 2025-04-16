use super::*;

#[nutype::nutype(
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, AsRef, Deref, Hash, Borrow, From, Into, Default),
    default = Default::default()
)]
pub struct Payload<'input>(Cow<'input, [u8]>);

#[nutype::nutype(
    validate(predicate = BinaryData::is_valid),
    // new_unchecked,
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord,AsRef,Deref, Hash, Borrow, TryFrom, Into, Default),
    default = Default::default()
)]
pub struct BinaryData<'input>(Cow<'input, [u8]>);

#[nutype::nutype(
    validate(predicate = Utf8String::is_valid),
    // new_unchecked,
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord,AsRef,Deref, Hash, Display, Borrow, TryFrom, Into, Default),
    default = Default::default()
)]
pub struct Utf8String<'input>(Cow<'input, str>);

#[nutype::nutype(
    validate(predicate = Topic::is_valid),
    // new_unchecked,
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord,AsRef,Deref, Hash, Display, Borrow, TryFrom, Into, Default),
    default = Default::default()
)]
pub struct Topic<'input>(Utf8String<'input>);

impl BinaryData<'_> {
    #[inline]
    fn is_valid(s: &Cow<'_, [u8]>) -> bool {
        s.len() <= u16::MAX as usize
    }
}

impl Utf8String<'_> {
    #[inline]
    fn is_valid(s: &Cow<'_, str>) -> bool {
        s.len() <= u16::MAX as usize && !s.as_ref().chars().any(Self::is_invalid_character)
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

impl Topic<'_> {
    #[inline(always)]
    fn is_valid(s: &Utf8String<'_>) -> bool {
        !s.contains(['#', '+'])
    }
}

impl From<Vec<u8>> for Payload<'static> {
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        Self::new(value.into())
    }
}

impl<'input> From<&'input [u8]> for Payload<'input> {
    #[inline]
    fn from(value: &'input [u8]) -> Self {
        Self::new(value.into())
    }
}

impl<'input, const SIZE: usize> From<&'input [u8; SIZE]> for Payload<'input> {
    #[inline]
    fn from(value: &'input [u8; SIZE]) -> Self {
        Self::new((&value[..] as &[u8]).into())
    }
}

impl TryFrom<Vec<u8>> for BinaryData<'static> {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_new(value.into())
    }
}

impl<'input> TryFrom<&'input [u8]> for BinaryData<'input> {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: &'input [u8]) -> Result<Self, Self::Error> {
        Self::try_new(value.into())
    }
}

impl<'input, const SIZE: usize> TryFrom<&'input [u8; SIZE]> for BinaryData<'input> {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: &'input [u8; SIZE]) -> Result<Self, Self::Error> {
        (&value[..] as &[u8]).try_into()
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
