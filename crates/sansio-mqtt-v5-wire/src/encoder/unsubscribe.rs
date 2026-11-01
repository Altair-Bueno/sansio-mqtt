use super::EncodeError;
use super::basic::TwoByteInteger;
use super::basic::VariableByteInteger;
use super::basic::fixed_header;
use super::basic::user_properties_iter;
use crate::ControlPacketType;
use crate::Unsubscribe;
use crate::UnsubscribeHeaderFlags;
use crate::UnsubscribeProperties;
use encode::ByteEncoder;
use encode::Encodable;

impl<E> Encodable<E> for UnsubscribeProperties
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let user_properties = user_properties_iter(&self.user_properties);

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new(user_properties)
            .encode(encoder)
    }
}

impl<E> Encodable<E> for Unsubscribe
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(
            ControlPacketType::Unsubscribe,
            u8::from(UnsubscribeHeaderFlags),
        )
        .encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::new(TwoByteInteger::new(self.packet_id.get())),
            &self.properties,
            encode::combinators::Iter::new(
                core::iter::once(&self.filter).chain(self.extra_filters.iter()),
            ),
        ))
        .encode(encoder)
    }
}
