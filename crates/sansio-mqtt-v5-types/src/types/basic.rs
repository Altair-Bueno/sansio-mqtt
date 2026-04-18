use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Payload(bytes::Bytes);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid MQTT binary data")]
pub struct BinaryDataError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BinaryData(bytes::Bytes);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Utf8StringError {
    #[error("MQTT UTF-8 string exceeds u16::MAX bytes")]
    TooLong,
    #[error("MQTT UTF-8 string contains invalid UTF-8")]
    InvalidUtf8,
    #[error("MQTT UTF-8 string contains disallowed characters")]
    DisallowedCharacter,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Utf8String(bytes::Bytes);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid MQTT topic")]
pub struct TopicError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Topic(Utf8String);

impl Payload {
    /// Constructs a [`Payload`] from any value convertible into [`bytes::Bytes`].
    ///
    /// This constructor is infallible and always returns `Ok`.
    #[inline]
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, core::convert::Infallible> {
        Ok(Self(value.into()))
    }

    /// Constructs a [`Payload`] from any value convertible into [`bytes::Bytes`].
    ///
    /// This constructor is infallible and never panics.
    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value).expect("Payload::try_new is infallible")
    }

    /// Creates a [`Payload`] without applying additional checks.
    ///
    /// # Safety
    ///
    /// Callers must ensure `value` satisfies all invariants expected by downstream
    /// protocol logic that consumes `Payload`.
    #[inline]
    pub unsafe fn new_unchecked(value: bytes::Bytes) -> Self {
        Self(value)
    }

    #[inline]
    pub fn into_inner(self) -> bytes::Bytes {
        self.0
    }
}

impl BinaryData {
    /// Constructs a [`BinaryData`] from any value convertible into [`bytes::Bytes`].
    ///
    /// Returns an error when the payload exceeds MQTT's 2-byte length limit.
    #[inline]
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, BinaryDataError> {
        let value = value.into();
        if value.len() <= u16::MAX as usize {
            Ok(Self(value))
        } else {
            Err(BinaryDataError)
        }
    }

    /// Constructs a [`BinaryData`] from any value convertible into [`bytes::Bytes`].
    ///
    /// Panics with `"BinaryData::new received invalid MQTT binary data"` when validation fails.
    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value).expect("BinaryData::new received invalid MQTT binary data")
    }

    /// Creates a [`BinaryData`] without enforcing the MQTT length limit.
    ///
    /// # Safety
    ///
    /// Callers must ensure `value.len() <= u16::MAX as usize`.
    #[inline]
    pub unsafe fn new_unchecked(value: bytes::Bytes) -> Self {
        Self(value)
    }

    #[inline]
    pub fn into_inner(self) -> bytes::Bytes {
        self.0
    }
}

impl Utf8String {
    /// Constructs a [`Utf8String`] from any value convertible into [`bytes::Bytes`].
    ///
    /// Returns an error when bytes are not valid UTF-8, exceed MQTT's 2-byte length limit,
    /// or contain MQTT-disallowed characters.
    #[inline]
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, Utf8StringError> {
        let value = value.into();
        if value.len() > u16::MAX as usize {
            return Err(Utf8StringError::TooLong);
        }

        let value_str = core::str::from_utf8(&value).map_err(|_| Utf8StringError::InvalidUtf8)?;
        if value_str.chars().any(Self::is_invalid_character) {
            return Err(Utf8StringError::DisallowedCharacter);
        }

        Ok(Self(value))
    }

    /// Constructs a [`Utf8String`] from any value convertible into [`bytes::Bytes`].
    ///
    /// Panics with `"Utf8String::new received invalid MQTT utf8 string"` when validation fails.
    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value).expect("Utf8String::new received invalid MQTT utf8 string")
    }

    /// Creates a [`Utf8String`] without UTF-8 or MQTT character validation.
    ///
    /// # Safety
    ///
    /// Callers must ensure `value` is valid UTF-8, has length at most `u16::MAX`,
    /// and does not contain MQTT-disallowed characters.
    #[inline]
    pub unsafe fn new_unchecked(value: bytes::Bytes) -> Self {
        Self(value)
    }

    #[inline]
    pub fn into_inner(self) -> bytes::Bytes {
        self.0
    }
}

impl Topic {
    /// Constructs a [`Topic`] from any value convertible into [`bytes::Bytes`].
    ///
    /// Returns an error when the topic is not a valid MQTT UTF-8 string or contains
    /// wildcard characters (`#` or `+`).
    #[inline]
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, TopicError> {
        let utf8 = Utf8String::try_new(value).map_err(|_| TopicError)?;
        Self::try_from(utf8)
    }

    /// Constructs a [`Topic`] from any value convertible into [`bytes::Bytes`].
    ///
    /// Panics with `"Topic::new received invalid MQTT topic"` when validation fails.
    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value).expect("Topic::new received invalid MQTT topic")
    }

    /// Creates a [`Topic`] without validating wildcard constraints.
    ///
    /// # Safety
    ///
    /// Callers must ensure the inner value is a valid MQTT topic string and does
    /// not contain `#` or `+` wildcard characters.
    #[inline]
    pub unsafe fn new_unchecked(value: Utf8String) -> Self {
        Self(value)
    }

    #[inline]
    pub fn into_inner(self) -> Utf8String {
        self.0
    }
}

impl core::convert::AsRef<bytes::Bytes> for Payload {
    #[inline]
    fn as_ref(&self) -> &bytes::Bytes {
        &self.0
    }
}

impl core::ops::Deref for Payload {
    type Target = bytes::Bytes;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::borrow::Borrow<bytes::Bytes> for Payload {
    #[inline]
    fn borrow(&self) -> &bytes::Bytes {
        &self.0
    }
}

impl From<bytes::Bytes> for Payload {
    #[inline]
    fn from(value: bytes::Bytes) -> Self {
        Self(value)
    }
}

impl From<Payload> for bytes::Bytes {
    #[inline]
    fn from(value: Payload) -> Self {
        value.0
    }
}

impl core::convert::AsRef<bytes::Bytes> for BinaryData {
    #[inline]
    fn as_ref(&self) -> &bytes::Bytes {
        &self.0
    }
}

impl core::ops::Deref for BinaryData {
    type Target = bytes::Bytes;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::borrow::Borrow<bytes::Bytes> for BinaryData {
    #[inline]
    fn borrow(&self) -> &bytes::Bytes {
        &self.0
    }
}

impl TryFrom<bytes::Bytes> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: bytes::Bytes) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<BinaryData> for bytes::Bytes {
    #[inline]
    fn from(value: BinaryData) -> Self {
        value.0
    }
}

impl core::convert::AsRef<bytes::Bytes> for Utf8String {
    #[inline]
    fn as_ref(&self) -> &bytes::Bytes {
        &self.0
    }
}

impl TryFrom<bytes::Bytes> for Utf8String {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: bytes::Bytes) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<Utf8String> for bytes::Bytes {
    #[inline]
    fn from(value: Utf8String) -> Self {
        value.0
    }
}

impl core::convert::AsRef<Utf8String> for Topic {
    #[inline]
    fn as_ref(&self) -> &Utf8String {
        &self.0
    }
}

impl core::ops::Deref for Topic {
    type Target = Utf8String;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::borrow::Borrow<Utf8String> for Topic {
    #[inline]
    fn borrow(&self) -> &Utf8String {
        &self.0
    }
}

impl TryFrom<Utf8String> for Topic {
    type Error = TopicError;

    #[inline]
    fn try_from(value: Utf8String) -> Result<Self, Self::Error> {
        if value.contains(['#', '+']) {
            Err(TopicError)
        } else {
            Ok(Self(value))
        }
    }
}

impl From<Topic> for Utf8String {
    #[inline]
    fn from(value: Topic) -> Self {
        value.0
    }
}

impl core::fmt::Display for Topic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_ref(), f)
    }
}

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
        let value: &str = self;
        f.debug_tuple("Utf8String").field(&value).finish()
    }
}

impl core::fmt::Display for Utf8String {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self)
    }
}

impl Utf8String {
    pub fn as_bytes(&self) -> &[u8] {
        let b: &bytes::Bytes = self.as_ref();
        b
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
        Self::new(value)
    }
}

impl From<&'static [u8]> for Payload {
    #[inline]
    fn from(value: &'static [u8]) -> Self {
        Self::new(value)
    }
}

impl<const SIZE: usize> From<&'static [u8; SIZE]> for Payload {
    #[inline]
    fn from(value: &'static [u8; SIZE]) -> Self {
        Self::new(&value[..] as &[u8])
    }
}

impl TryFrom<Vec<u8>> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&'static [u8]> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: &'static [u8]) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl<const SIZE: usize> TryFrom<&'static [u8; SIZE]> for BinaryData {
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
        Self::try_new(value)
    }
}

impl TryFrom<&'static str> for Utf8String {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        Self::try_new(value)
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
