use super::*;

impl<E> Encodable<E> for UnsubAckProperties<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let reason_string = self.reason_string.clone().map(Property::ReasonString);
        let user_properties = encode::combinators::Iter::new(
            self.user_properties
                .iter()
                .cloned()
                .map(|(k, v)| Property::UserProperty(k, v)),
        );

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            reason_string,
            user_properties,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for UnsubAck<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::UnsubAck) << 4;
        header_flags |= u8::from(UnsubAckHeaderFlags);
        encoder.put_byte(header_flags)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::new(TwoByteInteger::new(self.packet_id.get())),
            &self.properties,
            encode::combinators::FromError::new(encode::combinators::Iter::new(
                self.reason_codes.iter(),
            )),
        ))
        .encode(encoder)
    }
}
