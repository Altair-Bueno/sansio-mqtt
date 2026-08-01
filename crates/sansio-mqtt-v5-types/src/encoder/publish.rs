use super::*;

impl<E> Encodable<E> for PublishProperties
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
        let subscription_identifiers = encode::combinators::Iter::new(
            self.subscription_identifiers
                .iter()
                .copied()
                .map(Property::SubscriptionIdentifier),
        );
        let user_properties = user_properties_iter(&self.user_properties);
        let content_type = self.content_type.clone().map(Property::ContentType);

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            payload_format_indicator,
            message_expiry_interval,
            topic_alias,
            response_topic,
            correlation_data,
            user_properties,
            subscription_identifiers,
            content_type,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for Publish
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

        fixed_header(
            ControlPacketType::Publish,
            u8::from(PublishHeaderFlags {
                kind,
                retain: self.retain,
            }),
        )
        .encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            &self.topic,
            encode::combinators::FromError::<_, Self::Error>::new(
                packet_id.map(|x| TwoByteInteger::new(x.get())),
            ),
            &self.properties,
            encode::combinators::FromError::new(&self.payload),
        ))
        .encode(encoder)
    }
}
