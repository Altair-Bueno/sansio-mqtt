use super::EncodeError;
use super::basic::FourByteInteger;
use super::basic::TwoByteInteger;
use super::basic::VariableByteInteger;
use super::basic::user_properties_iter;
use crate::FormatIndicator;
use crate::Property;
use crate::PropertyType;
use crate::PubAckProperties;
use crate::PubCompProperties;
use crate::PubRecProperties;
use crate::PubRelProperties;
use crate::SubAckProperties;
use crate::Topic;
use crate::UnsubAckProperties;
use crate::Utf8String;
use encode::ByteEncoder;
use encode::Encodable;

impl<E: ByteEncoder> Encodable<E> for PropertyType
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        VariableByteInteger::from(u8::from(*self)).encode(encoder)
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
            Self::PayloadFormatIndicator(value) => {
                FormatIndicator::encode(value, encoder)?;
            }
            Self::MessageExpiryInterval(value)
            | Self::SessionExpiryInterval(value)
            | Self::WillDelayInterval(value) => {
                FourByteInteger::new(*value).encode(encoder)?;
            }
            Self::ContentType(value)
            | Self::AssignedClientIdentifier(value)
            | Self::AuthenticationMethod(value)
            | Self::ResponseInformation(value)
            | Self::ServerReference(value)
            | Self::ReasonString(value) => {
                Utf8String::encode(value, encoder)?;
            }
            Self::ResponseTopic(value) => {
                Topic::encode(value, encoder)?;
            }
            Self::CorrelationData(value) | Self::AuthenticationData(value) => {
                value.encode(encoder)?;
            }
            Self::SubscriptionIdentifier(value) => {
                VariableByteInteger::try_from(value.get())?.encode(encoder)?;
            }
            Self::ServerKeepAlive(value) | Self::TopicAliasMaximum(value) => {
                TwoByteInteger::new(*value).encode(encoder)?;
            }
            Self::RequestProblemInformation(value)
            | Self::RequestResponseInformation(value)
            | Self::RetainAvailable(value)
            | Self::WildcardSubscriptionAvailable(value)
            | Self::SubscriptionIdentifiersAvailable(value)
            | Self::SharedSubscriptionAvailable(value) => {
                bool::encode(value, encoder)?;
            }
            Self::ReceiveMaximum(value) | Self::TopicAlias(value) => {
                TwoByteInteger::new(value.get()).encode(encoder)?;
            }
            Self::MaximumQoS(value) => {
                u8::from(*value).encode(encoder)?;
            }
            Self::UserProperty(k, v) => {
                Utf8String::encode(k, encoder)?;
                Utf8String::encode(v, encoder)?;
            }
            Self::MaximumPacketSize(value) => {
                FourByteInteger::new(value.get()).encode(encoder)?;
            }
        }
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
