use super::*;

impl<E> Encodable<E> for UnsubscribeProperties<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let user_properties = encode::combinators::Iter::new(
            self.user_properties
                .iter()
                .cloned()
                .map(|(k, v)| Property::UserProperty(k, v)),
        );

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new(user_properties)
            .encode(encoder)
    }
}

impl<E> Encodable<E> for Unsubscribe<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::Unsubscribe) << 4;
        header_flags |= u8::from(UnsubscribeHeaderFlags);
        encoder.put_byte(header_flags)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::new(TwoByteInteger::new(self.packet_id.get())),
            &self.properties,
            encode::combinators::Iter::new(self.topics.iter()),
        ))
        .encode(encoder)
    }
}
