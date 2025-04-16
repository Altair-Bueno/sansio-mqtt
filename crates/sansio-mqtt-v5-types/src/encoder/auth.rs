use super::*;

impl<E> Encodable<E> for AuthProperties
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let reason_string = self.reason_string.clone().map(Property::ReasonString);
        let auth = match &self.authentication {
            Some(AuthenticationKind::WithoutData { method }) => {
                (Some(Property::AuthenticationMethod(method.clone())), None)
            }
            Some(AuthenticationKind::WithData { method, data }) => (
                Some(Property::AuthenticationMethod(method.clone())),
                Some(Property::AuthenticationData(data.clone())),
            ),
            None => (None, None),
        };
        let user_properties = encode::combinators::Iter::new(
            self.user_properties
                .iter()
                .cloned()
                .map(|(k, v)| Property::UserProperty(k, v)),
        );

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            reason_string,
            auth,
            user_properties,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for Auth
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::Auth) << 4;
        header_flags |= u8::from(AuthHeaderFlags);
        header_flags.encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new(
            encode::combinators::Cond::new(
                (
                    encode::combinators::FromError::new(self.reason_code),
                    &self.properties,
                ),
                |(reason_code, properties)| {
                    let success = **reason_code == AuthReasonCode::Success;
                    let no_properties = properties.is_empty();

                    !(success && no_properties)
                },
            ),
        )
        .encode(encoder)
    }
}
