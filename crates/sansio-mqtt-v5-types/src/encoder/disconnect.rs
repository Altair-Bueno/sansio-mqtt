use super::*;

impl<E> Encodable<E> for DisconnectProperties
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let reason_string = self.reason_string.clone().map(Property::ReasonString);
        let session_expiry_interval = self
            .session_expiry_interval
            .map(Property::SessionExpiryInterval);
        let server_reference = self.server_reference.clone().map(Property::ServerReference);
        let user_properties = user_properties_iter(&self.user_properties);

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            reason_string,
            session_expiry_interval,
            server_reference,
            user_properties,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for Disconnect
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(
            ControlPacketType::Disconnect,
            u8::from(DisconnectHeaderFlags),
        )
        .encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new(
            encode::combinators::Cond::new(
                (
                    encode::combinators::FromError::new(self.reason_code),
                    &self.properties,
                ),
                |(reason_code, properties)| {
                    let success = **reason_code == DisconnectReasonCode::NormalDisconnection;
                    let no_properties = properties.is_empty();

                    !(success && no_properties)
                },
            ),
        )
        .encode(encoder)
    }
}
