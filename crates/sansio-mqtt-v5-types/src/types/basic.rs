use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid MQTT payload")]
pub struct PayloadError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Payload(bytes::Bytes);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid MQTT binary data")]
pub struct BinaryDataError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BinaryData(bytes::Bytes);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid MQTT UTF-8 string")]
pub struct Utf8StringError;

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
    pub fn try_new(value: impl Into<bytes::Bytes>) -> Result<Self, PayloadError> {
        let value = value.into();
        if value.len() > u64::MAX as usize {
            // This check should never fail in practice since sizeof(u64) == sizeof(usize) on
            // basically all platforms, but we include it for completeness.
            return Err(PayloadError);
        }

        // SAFETY: Invariants have been checked above.
        Ok(unsafe { Self::new_unchecked(value) })
    }

    /// Constructs a [`Payload`] from any value convertible into [`bytes::Bytes`].
    ///
    /// This constructor is infallible and never panics.
    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value).expect("Payload::new received an invalid MQTT payload")
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
        if value.len() > u16::MAX as usize {
            return Err(BinaryDataError);
        }

        // SAFETY: Invariants have been checked above.
        Ok(unsafe { Self::new_unchecked(value) })
    }

    /// Constructs a [`BinaryData`] from any value convertible into [`bytes::Bytes`].
    ///
    /// Panics with `"BinaryData::new received invalid MQTT binary data"` when validation fails.
    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value).expect("BinaryData::new received an invalid MQTT binary data")
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
            return Err(Utf8StringError);
        }

        let value_str = core::str::from_utf8(&value).map_err(|_| Utf8StringError)?;
        if value_str.chars().any(Self::is_invalid_character) {
            return Err(Utf8StringError);
        }

        // SAFETY: Invariants have been checked above.
        Ok(unsafe { Self::new_unchecked(value) })
    }

    /// Constructs a [`Utf8String`] from any value convertible into [`bytes::Bytes`].
    ///
    /// Panics with `"Utf8String::new received invalid MQTT utf8 string"` when validation fails.
    #[inline]
    pub fn new(value: impl Into<bytes::Bytes>) -> Self {
        Self::try_new(value).expect("Utf8String::new received an invalid MQTT utf8 string")
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
        Self::try_new(value).expect("Topic::new received an invalid MQTT topic")
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

impl From<Payload> for bytes::Bytes {
    #[inline]
    fn from(value: Payload) -> Self {
        value.into_inner()
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
            return Err(TopicError);
        }
        // SAFETY: Invariants have been checked above.
        Ok(unsafe { Self::new_unchecked(value) })
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

impl<'a> From<&'a [u8]> for Payload {
    #[inline]
    fn from(value: &'a [u8]) -> Self {
        Self::new(bytes::Bytes::copy_from_slice(value))
    }
}

impl<'a, const SIZE: usize> From<&'a [u8; SIZE]> for Payload {
    #[inline]
    fn from(value: &'a [u8; SIZE]) -> Self {
        Self::from(&value[..] as &[u8])
    }
}

impl TryFrom<Vec<u8>> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl<'a> TryFrom<&'a [u8]> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Self::try_new(bytes::Bytes::copy_from_slice(value))
    }
}

impl<'a, const SIZE: usize> TryFrom<&'a [u8; SIZE]> for BinaryData {
    type Error = BinaryDataError;

    #[inline]
    fn try_from(value: &'a [u8; SIZE]) -> Result<Self, Self::Error> {
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

impl<'a> TryFrom<&'a str> for Utf8String {
    type Error = Utf8StringError;

    #[inline]
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::try_new(bytes::Bytes::copy_from_slice(value.as_bytes()))
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error)]
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error)]
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Error)]
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

#[cfg(test)]
mod marker_trait_guards {
    use super::*;

    trait MustNotImplementHash {}
    impl<T: core::hash::Hash> MustNotImplementHash for T {}

    trait MustNotImplementOrd {}
    impl<T: Ord> MustNotImplementOrd for T {}

    trait MustNotImplementPartialOrd {}
    impl<T: PartialOrd> MustNotImplementPartialOrd for T {}

    impl MustNotImplementHash for PayloadError {}
    impl MustNotImplementOrd for PayloadError {}
    impl MustNotImplementPartialOrd for PayloadError {}

    impl MustNotImplementHash for BinaryDataError {}
    impl MustNotImplementOrd for BinaryDataError {}
    impl MustNotImplementPartialOrd for BinaryDataError {}

    impl MustNotImplementHash for Utf8StringError {}
    impl MustNotImplementOrd for Utf8StringError {}
    impl MustNotImplementPartialOrd for Utf8StringError {}

    impl MustNotImplementHash for TopicError {}
    impl MustNotImplementOrd for TopicError {}
    impl MustNotImplementPartialOrd for TopicError {}

    fn assert_not_hash<T: MustNotImplementHash>() {}
    fn assert_not_ord<T: MustNotImplementOrd>() {}
    fn assert_not_partial_ord<T: MustNotImplementPartialOrd>() {}

    #[test]
    fn marker_errors_are_not_ordered_or_hashed() {
        assert_not_hash::<PayloadError>();
        assert_not_ord::<PayloadError>();
        assert_not_partial_ord::<PayloadError>();

        assert_not_hash::<BinaryDataError>();
        assert_not_ord::<BinaryDataError>();
        assert_not_partial_ord::<BinaryDataError>();

        assert_not_hash::<Utf8StringError>();
        assert_not_ord::<Utf8StringError>();
        assert_not_partial_ord::<Utf8StringError>();

        assert_not_hash::<TopicError>();
        assert_not_ord::<TopicError>();
        assert_not_partial_ord::<TopicError>();
    }
}
