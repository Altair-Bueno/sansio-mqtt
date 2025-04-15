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

impl<E: ByteEncoder> Encodable<E> for Property<'_>
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        match self {
            Property::PayloadFormatIndicator(value) => {
                PropertyType::PayloadFormatIndicator.encode(encoder)?;
                FormatIndicator::encode(value, encoder)?;
            }
            Property::MessageExpiryInterval(value) => {
                PropertyType::MessageExpiryInterval.encode(encoder)?;
                FourByteInteger::new(*value).encode(encoder)?;
            }
            Property::ContentType(value) => {
                PropertyType::ContentType.encode(encoder)?;
                MQTTString::encode(value, encoder)?;
            }
            Property::ResponseTopic(value) => {
                PropertyType::ResponseTopic.encode(encoder)?;
                PublishTopic::encode(value, encoder)?;
            }
            Property::CorrelationData(value) => {
                PropertyType::CorrelationData.encode(encoder)?;
                BinaryData::new(value.as_ref()).encode(encoder)?;
            }
            Property::SubscriptionIdentifier(value) => {
                PropertyType::SubscriptionIdentifier.encode(encoder)?;
                VariableByteInteger(value.get()).encode(encoder)?;
            }
            Property::SessionExpiryInterval(value) => {
                PropertyType::SessionExpiryInterval.encode(encoder)?;
                FourByteInteger::new(*value).encode(encoder)?;
            }
            Property::AssignedClientIdentifier(value) => {
                PropertyType::AssignedClientIdentifier.encode(encoder)?;
                MQTTString::encode(value, encoder)?;
            }
            Property::ServerKeepAlive(value) => {
                PropertyType::ServerKeepAlive.encode(encoder)?;
                TwoByteInteger::new(*value).encode(encoder)?;
            }
            Property::AuthenticationMethod(value) => {
                PropertyType::AuthenticationMethod.encode(encoder)?;
                MQTTString::encode(value, encoder)?;
            }
            Property::AuthenticationData(value) => {
                PropertyType::AuthenticationData.encode(encoder)?;
                BinaryData::new(value.as_ref()).encode(encoder)?;
            }
            Property::RequestProblemInformation(value) => {
                PropertyType::RequestProblemInformation.encode(encoder)?;
                bool::encode(value, encoder)?;
            }
            Property::WillDelayInterval(value) => {
                PropertyType::WillDelayInterval.encode(encoder)?;
                FourByteInteger::new(*value).encode(encoder)?;
            }
            Property::RequestResponseInformation(value) => {
                PropertyType::RequestResponseInformation.encode(encoder)?;
                bool::encode(value, encoder)?;
            }
            Property::ResponseInformation(value) => {
                PropertyType::ResponseInformation.encode(encoder)?;
                MQTTString::encode(value, encoder)?;
            }
            Property::ServerReference(value) => {
                PropertyType::ServerReference.encode(encoder)?;
                MQTTString::encode(value, encoder)?;
            }
            Property::ReasonString(value) => {
                PropertyType::ReasonString.encode(encoder)?;
                MQTTString::encode(value, encoder)?;
            }
            Property::ReceiveMaximum(value) => {
                PropertyType::ReceiveMaximum.encode(encoder)?;
                TwoByteInteger::new(value.get()).encode(encoder)?;
            }
            Property::TopicAliasMaximum(value) => {
                PropertyType::TopicAliasMaximum.encode(encoder)?;
                TwoByteInteger::new(*value).encode(encoder)?;
            }
            Property::TopicAlias(value) => {
                PropertyType::TopicAlias.encode(encoder)?;
                TwoByteInteger::new(value.get()).encode(encoder)?;
            }
            Property::MaximumQoS(value) => {
                PropertyType::MaximumQoS.encode(encoder)?;
                u8::from(*value).encode(encoder)?;
            }
            Property::RetainAvailable(value) => {
                PropertyType::RetainAvailable.encode(encoder)?;
                bool::encode(value, encoder)?;
            }
            Property::UserProperty(k, v) => {
                PropertyType::UserProperty.encode(encoder)?;
                MQTTString::encode(k, encoder)?;
                MQTTString::encode(v, encoder)?;
            }
            Property::MaximumPacketSize(value) => {
                PropertyType::MaximumPacketSize.encode(encoder)?;
                FourByteInteger::new(value.get()).encode(encoder)?;
            }
            Property::WildcardSubscriptionAvailable(value) => {
                PropertyType::WildcardSubscriptionAvailable.encode(encoder)?;
                bool::encode(value, encoder)?;
            }
            Property::SubscriptionIdentifiersAvailable(value) => {
                PropertyType::SubscriptionIdentifiersAvailable.encode(encoder)?;
                bool::encode(value, encoder)?;
            }
            Property::SharedSubscriptionAvailable(value) => {
                PropertyType::SharedSubscriptionAvailable.encode(encoder)?;
                bool::encode(value, encoder)?;
            }
        };
        Ok(())
    }
}
