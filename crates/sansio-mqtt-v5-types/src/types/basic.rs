use super::*;

#[nutype::nutype(
    new_unchecked,
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, AsRef, Deref, Hash, Borrow, From, Into, Default),
    default = Default::default()
)]
pub struct Payload(bytes::Bytes);

#[nutype::nutype(
    validate(predicate = |s| s.len() <= u16::MAX as usize),
    new_unchecked,
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord,AsRef,Deref, Hash, Borrow, TryFrom, Into, Default),
    default = Default::default()
)]
pub struct BinaryData(bytes::Bytes);

#[nutype::nutype(
    validate(predicate = |s| {
        let valid_len = s.len() <= u16::MAX as usize;
        let has_invalid_chars = core::str::from_utf8(s).map(|s| s.chars().any(Utf8String::is_invalid_character)).unwrap_or(true);
        valid_len && !has_invalid_chars
    }),
    new_unchecked,
    derive(Clone, PartialEq, Eq, PartialOrd, Ord,AsRef, Hash, TryFrom, Into, Default),
    default = Default::default()
)]
pub struct Utf8String(bytes::Bytes);

#[nutype::nutype(
    validate(predicate = |s| !s.contains(['#', '+'])),
    new_unchecked,
    derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord,AsRef,Deref, Hash, Display, Borrow, TryFrom, Into, Default),
    default = Default::default()
)]
pub struct Topic(Utf8String);

impl core::convert::AsRef<str> for Utf8String {
    #[inline]
    fn as_ref(&self) -> &str {
        // SAFETY: The Utf8String is guaranteed to be valid UTF-8 as per the validation predicate.
        unsafe { core::str::from_utf8_unchecked(self.as_bytes()) }
    }
}

impl core::ops::Deref for Utf8String {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl core::fmt::Debug for Utf8String {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Utf8String").field(&*self).finish()
    }
}

impl core::fmt::Display for Utf8String {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&*self)
    }
}

impl Utf8String {
    pub fn as_bytes(&self) -> &[u8] {
        let b: &bytes::Bytes = self.as_ref();
        &*b
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

impl From<Vec<u8>> for Payload {
    #[inline]
    fn from(value: Vec<u8>) -> Self {
        Self::new(value.into())
    }
}

impl From<&'static [u8]> for Payload {
    #[inline]
    fn from(value: &'static [u8]) -> Self {
        Self::new(value.into())
    }
}

impl<const SIZE: usize> From<&'static [u8; SIZE]> for Payload {
    #[inline]
    fn from(value: &'static [u8; SIZE]) -> Self {
        Self::new((&value[..] as &[u8]).into())
    }
}

impl TryFrom<Vec<u8>> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_new(value.into())
    }
}

impl TryFrom<&'static [u8]> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: &'static [u8]) -> Result<Self, Self::Error> {
        Self::try_new(value.into())
    }
}

impl<'input, const SIZE: usize> TryFrom<&'static [u8; SIZE]> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: &'static [u8; SIZE]) -> Result<Self, Self::Error> {
        (&value[..] as &[u8]).try_into()
    }
}

impl TryFrom<String> for Utf8String {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value.into())
    }
}

impl TryFrom<&'static str> for Utf8String {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        Self::try_new(value.into())
    }
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default)]
pub enum RetainHandling {
    #[default]
    SendRetained = 0,
    SendRetainedIfSubscriptionDoesNotExist = 1,
    DoNotSend = 2,
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default)]
pub enum FormatIndicator {
    #[default]
    Unspecified = 0,
    Utf8 = 1,
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash, PartialOrd, Eq, Ord, Default)]
pub enum Qos {
    #[default]
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
