use std::ops::Deref;

use super::*;

#[derive(Debug, PartialEq, Clone, Hash, PartialOrd, Eq, Ord)]
pub struct MQTTString<'input>(Cow<'input, str>);

impl<'input> MQTTString<'input> {
    #[inline]
    pub fn new<S>(value: S) -> Option<Self>
    where
        S: Into<Cow<'input, str>>,
    {
        let value = value.into();
        Self::is_valid(&value).then_some(Self(value))
    }

    #[inline(always)]
    pub fn is_valid(s: impl AsRef<str>) -> bool {
        !s.as_ref().chars().any(Self::is_invalid_character)
    }

    #[inline(always)]
    pub fn is_invalid_character(c: char) -> bool {
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

impl<'s> From<MQTTString<'s>> for Cow<'s, str> {
    #[inline]
    fn from(MQTTString(value): MQTTString<'s>) -> Self {
        value
    }
}

impl Deref for MQTTString<'_> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for MQTTString<'_> {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for MQTTString<'_> {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, PartialEq, Clone, Hash, PartialOrd, Eq, Ord)]
pub struct PublishTopic<'input>(MQTTString<'input>);

impl<'input> PublishTopic<'input> {
    #[inline]
    pub fn new<S>(value: S) -> Option<Self>
    where
        S: Into<Cow<'input, str>>,
    {
        let value = MQTTString::new(value)?;
        value.try_into().ok()
    }

    #[inline(always)]
    pub fn is_valid(s: &MQTTString<'_>) -> bool {
        !s.contains(['#', '+'])
    }
}

impl<'input> TryFrom<MQTTString<'input>> for PublishTopic<'input> {
    type Error = ();

    #[inline]
    fn try_from(value: MQTTString<'input>) -> Result<Self, Self::Error> {
        if Self::is_valid(&value) {
            Ok(Self(value))
        } else {
            Err(())
        }
    }
}

impl<'s> From<PublishTopic<'s>> for MQTTString<'s> {
    #[inline]
    fn from(PublishTopic(value): PublishTopic<'s>) -> Self {
        value
    }
}

impl Deref for PublishTopic<'_> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Borrow<str> for PublishTopic<'_> {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for PublishTopic<'_> {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'s> AsRef<MQTTString<'s>> for PublishTopic<'s> {
    #[inline]
    fn as_ref(&self) -> &MQTTString<'s> {
        &self.0
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
