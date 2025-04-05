use super::*;

pub type TwoByteInteger = encode::combinators::BE<u16>;
pub type FourByteInteger = encode::combinators::BE<u32>;
pub type BinaryData<'input> =
    encode::combinators::LengthPrefix<&'input [u8], TwoByteInteger, EncodeError>;

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
    E: Encoder,
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
            encoder.put_byte(encoded_byte)?;
            if value == 0 {
                break;
            }
        }
        Ok(())
    }
}

impl<E: Encoder> Encodable<E> for MQTTString<'_>
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        BinaryData::new(self.as_bytes()).encode(encoder)
    }
}
impl<E: Encoder> Encodable<E> for PublishTopic<'_>
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        MQTTString::encode(self.as_ref(), encoder)
    }
}

impl<E: Encoder> Encodable<E> for FormatIndicator {
    type Error = E::Error;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        u8::from(*self).encode(encoder)
    }
}

impl<E: Encoder> Encodable<E> for Subscription<'_>
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
        impl<E: Encoder> Encodable<E> for $name {
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
