use super::*;

impl<E> Encodable<E> for PublishProperties<'_>
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let payload_format_indicator = self
            .payload_format_indicator
            .map(Property::PayloadFormatIndicator);
        let message_expiry_interval = self
            .message_expiry_interval
            .map(Property::MessageExpiryInterval);
        let topic_alias = self.topic_alias.map(Property::TopicAlias);
        let response_topic = self.response_topic.clone().map(Property::ResponseTopic);
        let correlation_data = self.correlation_data.clone().map(Property::CorrelationData);
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
        let content_type = self.content_type.clone().map(Property::ContentType);

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            payload_format_indicator,
            message_expiry_interval,
            topic_alias,
            response_topic,
            correlation_data,
            user_properties,
            subscription_identifier,
            content_type,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for Publish<'_>
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let (kind, packet_id) = match self.kind {
            PublishKind::FireAndForget => (PublishHeaderFlagsKind::Simple, None),
            PublishKind::Repetible {
                packet_id,
                qos,
                dup,
            } => (
                PublishHeaderFlagsKind::Advanced { qos, dup },
                Some(packet_id),
            ),
        };

        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::Publish) << 4;
        header_flags |= u8::from(PublishHeaderFlags {
            kind,
            retain: self.retain,
        });
        header_flags.encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            &self.topic,
            encode::combinators::FromError::<_, Self::Error>::new(
                packet_id.map(NonZero::get).map(TwoByteInteger::new),
            ),
            &self.properties,
            encode::combinators::FromError::new(self.payload.as_ref()),
        ))
        .encode(encoder)
    }
}
