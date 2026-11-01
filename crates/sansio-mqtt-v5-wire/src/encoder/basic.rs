use encode::combinators::LengthPrefix;

use super::EncodeError;
use crate::AuthReasonCode;
use crate::BinaryData;
use crate::ConnackReasonCode;
use crate::ConnectReasonCode;
use crate::ControlPacketType;
use crate::DisconnectReasonCode;
use crate::FormatIndicator;
use crate::Payload;
use crate::Property;
use crate::PubAckReasonCode;
use crate::PubCompReasonCode;
use crate::PubRecReasonCode;
use crate::PubRelReasonCode;
use crate::PublishReasonCode;
use crate::SubAckReasonCode;
use crate::Subscription;
use crate::Topic;
use crate::UnsubAckReasonCode;
use crate::Utf8String;
use encode::ByteEncoder;
use encode::Encodable;

pub(super) type TwoByteInteger = encode::combinators::BE<u16>;
pub(super) type FourByteInteger = encode::combinators::BE<u32>;

/// Builds the Fixed Header byte from a Control Packet type and its
/// 4-bit flags nibble
/// ([§2.1.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901021)).
#[inline]
pub(super) fn fixed_header(packet_type: ControlPacketType, flags: u8) -> u8 {
    (u8::from(packet_type) << 4) | flags
}

/// Wraps a User Property list as the [`Property`] sequence the
/// encoders expect
/// ([§2.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901029)).
#[inline]
pub(super) fn user_properties_iter(
    properties: &[(Utf8String, Utf8String)],
) -> encode::combinators::Iter<impl Iterator<Item = Property> + Clone + '_> {
    encode::combinators::Iter::new(
        properties
            .iter()
            .cloned()
            .map(|(key, value)| Property::UserProperty(key, value)),
    )
}

/// A value that fits in a Variable Byte Integer
/// ([§1.5.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901011)):
/// at most [`Self::MAX`], so it always encodes in four bytes or fewer
/// [MQTT-1.5.5-1].
pub(super) struct VariableByteInteger(u32);

impl VariableByteInteger {
    /// The largest value four bytes can encode.
    pub(super) const MAX: u32 = 268_435_455;
}

impl From<u8> for VariableByteInteger {
    #[inline]
    fn from(value: u8) -> Self {
        Self(u32::from(value))
    }
}

impl TryFrom<u64> for VariableByteInteger {
    type Error = EncodeError;

    #[inline]
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        u32::try_from(value)
            .ok()
            .filter(|value| *value <= Self::MAX)
            .map(Self)
            .ok_or(EncodeError::VariableByteIntegerTooLarge(value))
    }
}

impl TryFrom<usize> for VariableByteInteger {
    type Error = EncodeError;

    #[inline]
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::try_from(u64::try_from(value)?)
    }
}

impl<E> Encodable<E> for VariableByteInteger
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    #[inline]
    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut value = self.0;
        loop {
            let [low_byte, ..] = value.to_le_bytes();
            let mut encoded_byte = low_byte & 0x7F;
            value /= 128;
            if value > 0 {
                encoded_byte |= 128;
            }
            encoded_byte.encode(encoder)?;
            if value == 0 {
                break;
            }
        }
        Ok(())
    }
}

impl<E: ByteEncoder> Encodable<E> for Payload
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let bytes: &[u8] = self.as_ref();
        bytes.encode(encoder)?;
        Ok(())
    }
}

impl<E: ByteEncoder> Encodable<E> for BinaryData
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let bytes: &[u8] = self.as_ref();
        LengthPrefix::<_, TwoByteInteger, _>::new(bytes).encode(encoder)
    }
}

impl<E: ByteEncoder> Encodable<E> for Utf8String
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let bytes: &[u8] = self.as_bytes();
        LengthPrefix::<_, TwoByteInteger, _>::new(bytes).encode(encoder)
    }
}
impl<E: ByteEncoder> Encodable<E> for Topic
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        Utf8String::encode(self.as_ref(), encoder)
    }
}

impl<E: ByteEncoder> Encodable<E> for FormatIndicator {
    type Error = E::Error;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        u8::from(*self).encode(encoder)
    }
}

impl<E: ByteEncoder> Encodable<E> for Subscription
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        self.topic_filter.encode(encoder)?;
        let mut flags = 0u8;
        flags |= u8::from(self.qos);
        flags |= u8::from(self.no_local) << 2;
        flags |= u8::from(self.retain_as_published) << 3;
        flags |= u8::from(self.retain_handling) << 4;
        flags.encode(encoder)?;
        Ok(())
    }
}

macro_rules! impl_encode_for_reason_code {
    ($name:ty) => {
        impl<E: ByteEncoder> Encodable<E> for $name {
            type Error = E::Error;

            #[inline]
            fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
                u8::from(*self).encode(encoder)
            }
        }
    };
}

impl_encode_for_reason_code!(ConnectReasonCode);
impl_encode_for_reason_code!(ConnackReasonCode);
impl_encode_for_reason_code!(PublishReasonCode);
impl_encode_for_reason_code!(PubAckReasonCode);
impl_encode_for_reason_code!(PubRecReasonCode);
impl_encode_for_reason_code!(PubRelReasonCode);
impl_encode_for_reason_code!(PubCompReasonCode);
impl_encode_for_reason_code!(SubAckReasonCode);
impl_encode_for_reason_code!(UnsubAckReasonCode);
impl_encode_for_reason_code!(DisconnectReasonCode);
impl_encode_for_reason_code!(AuthReasonCode);
