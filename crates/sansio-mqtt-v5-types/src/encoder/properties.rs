use super::*;

impl<E: ByteEncoder> Encodable<E> for PropertyType
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        VariableByteInteger(u64::from(*self)).encode(encoder)
    }
}

impl<E: ByteEncoder> Encodable<E> for Property
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        // The identifier is derived from the variant so the tag on the
        // wire cannot disagree with the payload that follows it
        // ([MQTT-2.2.2-1]).
        PropertyType::from(self).encode(encoder)?;

        match self {
            Property::PayloadFormatIndicator(value) => {
                FormatIndicator::encode(value, encoder)?;
            }
            Property::MessageExpiryInterval(value) => {
                FourByteInteger::new(*value).encode(encoder)?;
            }
            Property::ContentType(value) => {
                Utf8String::encode(value, encoder)?;
            }
            Property::ResponseTopic(value) => {
                Topic::encode(value, encoder)?;
            }
            Property::CorrelationData(value) => {
                value.encode(encoder)?;
            }
            Property::SubscriptionIdentifier(value) => {
                VariableByteInteger(value.get()).encode(encoder)?;
            }
            Property::SessionExpiryInterval(value) => {
                FourByteInteger::new(*value).encode(encoder)?;
            }
            Property::AssignedClientIdentifier(value) => {
                Utf8String::encode(value, encoder)?;
            }
            Property::ServerKeepAlive(value) => {
                TwoByteInteger::new(*value).encode(encoder)?;
            }
            Property::AuthenticationMethod(value) => {
                Utf8String::encode(value, encoder)?;
            }
            Property::AuthenticationData(value) => {
                value.encode(encoder)?;
            }
            Property::RequestProblemInformation(value) => {
                bool::encode(value, encoder)?;
            }
            Property::WillDelayInterval(value) => {
                FourByteInteger::new(*value).encode(encoder)?;
            }
            Property::RequestResponseInformation(value) => {
                bool::encode(value, encoder)?;
            }
            Property::ResponseInformation(value) => {
                Utf8String::encode(value, encoder)?;
            }
            Property::ServerReference(value) => {
                Utf8String::encode(value, encoder)?;
            }
            Property::ReasonString(value) => {
                Utf8String::encode(value, encoder)?;
            }
            Property::ReceiveMaximum(value) => {
                TwoByteInteger::new(value.get()).encode(encoder)?;
            }
            Property::TopicAliasMaximum(value) => {
                TwoByteInteger::new(*value).encode(encoder)?;
            }
            Property::TopicAlias(value) => {
                TwoByteInteger::new(value.get()).encode(encoder)?;
            }
            Property::MaximumQoS(value) => {
                u8::from(*value).encode(encoder)?;
            }
            Property::RetainAvailable(value) => {
                bool::encode(value, encoder)?;
            }
            Property::UserProperty(k, v) => {
                Utf8String::encode(k, encoder)?;
                Utf8String::encode(v, encoder)?;
            }
            Property::MaximumPacketSize(value) => {
                FourByteInteger::new(value.get()).encode(encoder)?;
            }
            Property::WildcardSubscriptionAvailable(value) => {
                bool::encode(value, encoder)?;
            }
            Property::SubscriptionIdentifiersAvailable(value) => {
                bool::encode(value, encoder)?;
            }
            Property::SharedSubscriptionAvailable(value) => {
                bool::encode(value, encoder)?;
            }
        };
        Ok(())
    }
}

/// Generates the properties-section encoder shared by the acknowledgement
/// packets whose only permitted properties are Reason String and User
/// Property ([§2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901027)).
macro_rules! impl_encode_for_ack_properties {
    ($name:ty) => {
        impl<E> Encodable<E> for $name
        where
            E: ByteEncoder,
            EncodeError: From<E::Error>,
        {
            type Error = EncodeError;

            fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
                let reason_string = self.reason_string.clone().map(Property::ReasonString);

                encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
                    reason_string,
                    user_properties_iter(&self.user_properties),
                ))
                .encode(encoder)
            }
        }
    };
}

impl_encode_for_ack_properties!(PubAckProperties);
impl_encode_for_ack_properties!(PubRecProperties);
impl_encode_for_ack_properties!(PubRelProperties);
impl_encode_for_ack_properties!(PubCompProperties);
impl_encode_for_ack_properties!(SubAckProperties);
impl_encode_for_ack_properties!(UnsubAckProperties);
