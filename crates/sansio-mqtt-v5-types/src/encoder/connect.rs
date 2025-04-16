use super::*;

impl<E> Encodable<E> for ConnectProperties<'_>
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let session_expiry_interval = self
            .session_expiry_interval
            .clone()
            .map(Property::SessionExpiryInterval);
        let receive_maximum = self.receive_maximum.clone().map(Property::ReceiveMaximum);
        let maximum_packet_size = self
            .maximum_packet_size
            .clone()
            .map(Property::MaximumPacketSize);
        let topic_alias_maximum = self
            .topic_alias_maximum
            .clone()
            .map(Property::TopicAliasMaximum);
        let authentication = match &self.authentication {
            Some(AuthenticationKind::WithoutData { method }) => {
                (Some(Property::AuthenticationMethod(method.clone())), None)
            }
            Some(AuthenticationKind::WithData { method, data }) => (
                Some(Property::AuthenticationMethod(method.clone())),
                Some(Property::AuthenticationData(data.clone())),
            ),
            None => (None, None),
        };
        let request_response_information = self
            .request_response_information
            .clone()
            .map(Property::RequestResponseInformation);
        let request_problem_information = self
            .request_problem_information
            .clone()
            .map(Property::RequestProblemInformation);
        let user_properties = encode::combinators::Iter::new(
            self.user_properties
                .iter()
                .cloned()
                .map(|(k, v)| Property::UserProperty(k, v)),
        );

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            session_expiry_interval,
            receive_maximum,
            maximum_packet_size,
            topic_alias_maximum,
            authentication,
            request_response_information,
            request_problem_information,
            user_properties,
        ))
        .encode(encoder)
    }
}
impl<E> Encodable<E> for WillProperties<'_>
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let will_delay_interval = self
            .will_delay_interval
            .clone()
            .map(Property::WillDelayInterval);
        let payload_format_indicator = self
            .payload_format_indicator
            .clone()
            .map(Property::PayloadFormatIndicator);
        let message_expiry_interval = self
            .message_expiry_interval
            .clone()
            .map(Property::MessageExpiryInterval);
        let content_type = self.content_type.clone().map(Property::ContentType);
        let response_topic = self.response_topic.clone().map(Property::ResponseTopic);
        let correlation_data = self.correlation_data.clone().map(Property::CorrelationData);
        let user_properties = encode::combinators::Iter::new(
            self.user_properties
                .iter()
                .cloned()
                .map(|(k, v)| Property::UserProperty(k, v)),
        );

        encode::combinators::LengthPrefix::<_, VariableByteInteger, _>::new((
            will_delay_interval,
            payload_format_indicator,
            message_expiry_interval,
            content_type,
            response_topic,
            correlation_data,
            user_properties,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for Connect<'_>
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::Connect) << 4;
        header_flags |= u8::from(ConnectHeaderFlags);
        header_flags.encode(encoder)?;

        let mut flags = 0u8;
        flags |= u8::from(self.clean_start) << 1;
        if let Some(will) = &self.will {
            flags |= 1 << 2;
            flags |= u8::from(will.qos) << 3;
            flags |= u8::from(will.retain) << 5;
        }
        flags |= u8::from(self.user_name.is_some()) << 6;
        flags |= u8::from(self.password.is_some()) << 7;
        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            &self.protocol_name,
            encode::combinators::FromError::new(self.protocol_version),
            encode::combinators::FromError::new(flags),
            encode::combinators::FromError::new(TwoByteInteger::new(
                self.keep_alive.map(|x| x.get()).unwrap_or_default(),
            )),
            &self.properties,
            &self.client_identifier,
            self.will
                .as_ref()
                .map(|will| (&will.properties, &will.topic, &will.payload)),
            &self.user_name,
            &self.password,
        ))
        .encode(encoder)
    }
}
