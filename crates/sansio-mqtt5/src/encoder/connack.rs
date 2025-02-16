use super::*;

impl<E> Encodable<E> for ConnAckProperties<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let session_expiry_interval = self
            .session_expiry_interval
            .clone()
            .map(Property::SessionExpiryInterval);
        let receive_maximum = self.receive_maximum.clone().map(Property::ReceiveMaximum);
        let maximum_qos = self.maximum_qos.clone().map(Property::MaximumQoS);
        let retain_available = self.retain_available.clone().map(Property::RetainAvailable);
        let maximum_packet_size = self
            .maximum_packet_size
            .clone()
            .map(Property::MaximumPacketSize);
        let assigned_client_identifier = self
            .assigned_client_identifier
            .clone()
            .map(Property::AssignedClientIdentifier);
        let topic_alias_maximum = self
            .topic_alias_maximum
            .clone()
            .map(Property::TopicAliasMaximum);
        let reason_string = self.reason_string.clone().map(Property::ReasonString);
        let wildcard_subscription_available = self
            .wildcard_subscription_available
            .clone()
            .map(Property::WildcardSubscriptionAvailable);
        let subscription_identifiers_available = self
            .subscription_identifiers_available
            .clone()
            .map(Property::SubscriptionIdentifiersAvailable);
        let shared_subscription_available = self
            .shared_subscription_available
            .clone()
            .map(Property::SharedSubscriptionAvailable);
        let server_keep_alive = self
            .server_keep_alive
            .clone()
            .map(Property::ServerKeepAlive);
        let response_information = self
            .response_information
            .clone()
            .map(Property::ResponseInformation);
        let server_reference = self.server_reference.clone().map(Property::ServerReference);
        let authentication = match self.authentication.clone() {
            Some(AuthenticationKind::WithoutData { method }) => {
                (Some(Property::AuthenticationMethod(method)), None)
            }
            Some(AuthenticationKind::WithData { method, data }) => (
                Some(Property::AuthenticationMethod(method)),
                Some(Property::AuthenticationData(data)),
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
            session_expiry_interval,
            receive_maximum,
            maximum_qos,
            retain_available,
            maximum_packet_size,
            assigned_client_identifier,
            topic_alias_maximum,
            reason_string,
            wildcard_subscription_available,
            subscription_identifiers_available,
            shared_subscription_available,
            server_keep_alive,
            response_information,
            server_reference,
            authentication,
            user_properties,
        ))
        .encode(encoder)
    }
}

impl<E> Encodable<E> for ConnAck<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::ConnAck) << 4;
        header_flags |= u8::from(ConnAckHeaderFlags);
        encoder.put_byte(header_flags)?;

        let ack_flags = encode::combinators::Flags::new([
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            self.kind == ConnAckKind::ResumePreviousSession, // Session Present
        ]);
        let reason_code = if let ConnAckKind::Other { reason_code } = self.kind {
            reason_code
        } else {
            ReasonCode::Success
        };
        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::<_, Self::Error>::new(ack_flags),
            encode::combinators::FromError::<_, Self::Error>::new(reason_code),
            &self.properties,
        ))
        .encode(encoder)
    }
}
