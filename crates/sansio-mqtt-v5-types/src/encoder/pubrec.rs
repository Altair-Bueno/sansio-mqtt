use super::*;

impl<E> Encodable<E> for PubRecProperties
where
    E: ByteEncoder,
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

impl<E> Encodable<E> for PubRec
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::PubRec) << 4;
        header_flags |= u8::from(PubRecHeaderFlags);
        header_flags.encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::<_, Self::Error>::new(TwoByteInteger::new(
                self.packet_id.get(),
            )),
            encode::combinators::Cond::new(
                (
                    encode::combinators::FromError::new(self.reason_code),
                    &self.properties,
                ),
                |(reason_code, properties)| {
                    let success = **reason_code == PubRecReasonCode::Success;
                    let no_properties = properties.is_empty();

                    !(success && no_properties)
                },
            ),
        ))
        .encode(encoder)
    }
}
