use super::*;

impl<E> Encodable<E> for SubscribeProperties
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let subscription_identifier = self
            .subscription_identifier
            .map(Property::SubscriptionIdentifier);
        let user_properties = user_properties_iter(&self.user_properties);

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            subscription_identifier,
            user_properties,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for Subscribe
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(ControlPacketType::Subscribe, u8::from(SubscribeHeaderFlags))
            .encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::new(TwoByteInteger::new(self.packet_id.get())),
            &self.properties,
            encode::combinators::FromError::new(encode::combinators::Iter::new(
                core::iter::once(&self.subscription).chain(self.extra_subscriptions.iter()),
            )),
        ))
        .encode(encoder)
    }
}
