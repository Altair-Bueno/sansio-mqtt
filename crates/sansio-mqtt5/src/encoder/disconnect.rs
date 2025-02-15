use super::*;

impl<E> Encodable<E> for DisconnectProperties<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let reason_string = self.reason_string.clone().map(Property::ReasonString);
        let session_expiry_interval = self
            .session_expiry_interval
            .map(|v| Property::SessionExpiryInterval(v));
        let server_reference = self.server_reference.clone().map(Property::ServerReference);
        let user_properties = encode::combinators::Iter::new(
            self.user_properties
                .iter()
                .cloned()
                .map(|(k, v)| Property::UserProperty(k, v)),
        );

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            reason_string,
            session_expiry_interval,
            server_reference,
            user_properties,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for Disconnect<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::Disconnect) << 4;
        header_flags |= u8::from(DisconnectHeaderFlags);
        encoder.put_byte(header_flags)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new(
            encode::combinators::Cond::new(
                (
                    encode::combinators::FromError::new(self.reason_code),
                    &self.properties,
                ),
                |(reason_code, properties)| {
                    let success = **reason_code == ReasonCode::Success;
                    let no_properties = properties.is_empty();

                    !(success && no_properties)
                },
            ),
        )
        .encode(encoder)
    }
}
