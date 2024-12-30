#[inline]
pub fn flags<Input, BitError, ByteError>(input: &mut Input) -> PResult<(bool,), ByteError>
where
    BitError: ParserError<(Input, usize)> + ErrorConvert<ByteError>,
    ByteError: ParserError<Input>,
    (Input, usize): Stream,
    Input: Stream<Token = u8> + StreamIsPartial + Clone,
{
    let (_, session_present) =
        bits::bits::<_, _, BitError, _, _>((bits::pattern(0, 7usize), bits::bool))
            .parse_next(input)?;
    Ok((session_present,))
}

use super::*;

impl<'input> ConnAck<'input> {
    #[inline]
    pub fn parse<'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial
            + Stream<Token = u8, Slice = &'input [u8]>
            + Clone
            + UpdateSlice
            + 'input,
        ByteError: ParserError<ByteInput>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, InvalidQosError>
            + FromExternalError<ByteInput, InvalidPropertyTypeError>
            + FromExternalError<ByteInput, UnknownFormatIndicatorError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<(ByteInput, usize)> + ErrorConvert<ByteError>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                flags::<_, BitError, _>,
                ReasonCode::parse_connack,
                ConnAckProperties::parse(parser_settings),
                combinator::eof,
            )
                .verify_map(move |((session_present,), reason_code, properties, _)| {
                    // If a Server sends a CONNACK packet containing a non-zero Reason Code it MUST set Session Present to 0 [MQTT-3.2.2-6].
                    let kind = match (session_present, reason_code) {
                        (true, ReasonCode::Success) => ConnAckKind::ResumePreviousSession,
                        (false, reason_code) => ConnAckKind::Other { reason_code },
                        (true, _) => return None,
                    };
                    Some(ConnAck { kind, properties })
                }),
        )
    }
}

impl ConnAckHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> PResult<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "CONNACK Header Flags",
            )))
            .parse_next(input)
    }
}

impl<'input> ConnAckProperties<'input> {
    #[inline]
    pub fn parse<'settings, Input, Error>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]>
            + UpdateSlice
            + StreamIsPartial
            + Clone
            + 'input,
        Error: ParserError<Input>
            + AddContext<Input, StrContext>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, UnknownFormatIndicatorError>,
    {
        combinator::trace(type_name::<Self>(), |input: &mut Input| {
            // TODO: Can't use binary::length_and_then because it doesn't work
            let data = binary::length_take(variable_byte_integer).parse_next(input)?;
            let mut input = input.clone().update_slice(data);
            let input = &mut input;

            let mut properties = Self::default();
            let mut authentication_method = None;
            let mut authentication_data = None;

            let mut parser = combinator::alt((
                combinator::eof.value(None),
                Property::parse(parser_settings).map(Some),
            ));

            while let Some(p) = parser.parse_next(input)? {
                match p {
                    Property::SessionExpiryInterval(value) => {
                        properties.session_expiry_interval.replace(value);
                    }
                    Property::ReceiveMaximum(value) => {
                        properties.receive_maximum.replace(value);
                    }
                    Property::MaximumQoS(value) => {
                        properties.maximum_qos.replace(value);
                    }
                    Property::RetainAvailable(value) => {
                        properties.retain_available.replace(value);
                    }
                    Property::MaximumPacketSize(value) => {
                        properties.maximum_packet_size.replace(value);
                    }
                    Property::AssignedClientIdentifier(value) => {
                        properties.assigned_client_identifier.replace(value);
                    }
                    Property::TopicAliasMaximum(value) => {
                        properties.topic_alias_maximum.replace(value);
                    }
                    Property::ReasonString(value) => {
                        properties.reason_string.replace(value);
                    }
                    Property::UserProperty(key, value) => {
                        properties.user_properties.push((key, value))
                    }
                    Property::WildcardSubscriptionAvailable(value) => {
                        properties.wildcard_subscription_available.replace(value);
                    }
                    Property::SubscriptionIdentifiersAvailable(value) => {
                        properties.subscription_identifiers_available.replace(value);
                    }
                    Property::SharedSubscriptionAvailable(value) => {
                        properties.shared_subscription_available.replace(value);
                    }
                    Property::ServerKeepAlive(value) => {
                        properties.server_keep_alive.replace(value);
                    }
                    Property::ResponseInformation(value) => {
                        properties.response_information.replace(value);
                    }
                    Property::ServerReference(value) => {
                        properties.server_reference.replace(value);
                    }
                    Property::AuthenticationMethod(value) => {
                        authentication_method.replace(value);
                    }
                    Property::AuthenticationData(value) => {
                        authentication_data.replace(value);
                    }

                    _ => return Err(ErrMode::Cut(Error::assert(input, "Invalid property type"))),
                };
            }

            // It is a Protocol Error to include Authentication Data if there is no Authentication Method
            properties.authentication = match (authentication_method, authentication_data) {
                (None, None) => None,
                (Some(method), None) => Some(AuthenticationKind::WithoutData { method }),
                (Some(method), Some(data)) => Some(AuthenticationKind::WithData { method, data }),
                (None, Some(_)) => {
                    return Err(ErrMode::Cut(Error::assert(
                        input,
                        "Authentication Data without Authentication Method",
                    )))
                }
            };
            Ok(properties)
        })
        .context(StrContext::Label(type_name::<Self>()))
    }
}
