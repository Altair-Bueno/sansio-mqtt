use encode::combinators::LengthPrefix;

use super::*;

pub type TwoByteInteger = encode::combinators::BE<u16>;
pub type FourByteInteger = encode::combinators::BE<u32>;

pub struct VariableByteInteger(pub u64);

impl TryFrom<usize> for VariableByteInteger {
    type Error = EncodeError;

    #[inline]
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(VariableByteInteger(u64::try_from(value)?))
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
            let mut encoded_byte = (value % 128) as u8;
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
