use super::*;

impl<E> Encodable<E> for SubscribeProperties<'_>
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let subscription_identifier = self
            .subscription_identifier
            .clone()
            .map(Property::SubscriptionIdentifier);
        let user_properties = encode::combinators::Iter::new(
            self.user_properties
                .iter()
                .cloned()
                .map(|(k, v)| Property::UserProperty(k, v)),
        );

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            subscription_identifier,
            user_properties,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for Subscribe<'_>
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::Subscribe) << 4;
        header_flags |= u8::from(SubscribeHeaderFlags);
        header_flags.encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::new(TwoByteInteger::new(self.packet_id.get())),
            &self.properties,
            encode::combinators::FromError::new(encode::combinators::Iter::new(
                self.subscriptions.iter(),
            )),
        ))
        .encode(encoder)
    }
}
