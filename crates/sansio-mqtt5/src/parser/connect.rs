use super::*;

#[inline]
pub fn flags<Input, BitError, ByteError>(
    input: &mut Input,
) -> PResult<(bool, bool, bool, Qos, bool, bool), ByteError>
where
    BitError: ParserError<(Input, usize)>
        + ErrorConvert<ByteError>
        + FromExternalError<(Input, usize), InvalidQosError>
        + AddContext<(Input, usize), StrContext>,
    ByteError: ParserError<Input>,
    (Input, usize): Stream,
    Input: Stream<Token = u8> + StreamIsPartial + Clone,
{
    let (username_flag, password_flag, will_retain, will_qos, will_flag, clean_start, _) =
        bits::bits::<_, _, BitError, _, _>((
            bits::bool,
            bits::bool,
            bits::bool,
            Qos::parse,
            bits::bool,
            bits::bool,
            bits::pattern(0u8, 1usize),
        ))
        .parse_next(input)?;
    Ok((
        username_flag,
        password_flag,
        will_retain,
        will_qos,
        will_flag,
        clean_start,
    ))
}

impl<'input> Connect<'input> {
    #[inline]
    pub fn parse<'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + Clone + UpdateSlice,
        ByteError: ParserError<ByteInput>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, InvalidQosError>
            + FromExternalError<ByteInput, InvalidPropertyTypeError>
            + FromExternalError<ByteInput, UnknownFormatIndicatorError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<(ByteInput, usize)>
            + ErrorConvert<ByteError>
            + FromExternalError<(ByteInput, usize), InvalidQosError>
            + AddContext<(ByteInput, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), move |input: &mut ByteInput| {
            let (
                protocol_name,
                protocol_version,
                (username_flag, password_flag, will_retain, will_qos, will_flag, clean_start),
                keep_alive,
                properties,
                client_identifier,
            ) = (
                combinator::trace("Protocol name", MQTTString::parse(parser_settings)),
                combinator::trace("Protocol version", token::any),
                self::flags::<_, BitError, _>.verify(
                    |(_, _, will_retain, will_qos, will_flag, _)| {
                        if *will_flag {
                            true
                        } else {
                            !will_retain && *will_qos == Qos::AtMostOnce
                        }
                    },
                ),
                combinator::trace("Keep alive", self::two_byte_integer),
                ConnectProperties::parse(parser_settings),
                combinator::trace("Client identifier", MQTTString::parse(parser_settings)),
            )
                .parse_next(input)?;

            let (will, user_name, password, _) = (
                combinator::trace(
                    "will",
                    combinator::cond(
                        will_flag,
                        (
                            WillProperties::parse(parser_settings),
                            MQTTString::parse(parser_settings).map(Into::into),
                            self::binary_data(parser_settings).map(Into::into),
                        )
                            .map(|(properties, topic, payload)| Will {
                                properties,
                                topic,
                                payload,
                                retain: will_retain,
                                qos: will_qos,
                            }),
                    ),
                ),
                combinator::trace(
                    "username",
                    combinator::cond(
                        username_flag,
                        MQTTString::parse(parser_settings).map(Into::into),
                    ),
                ),
                combinator::trace(
                    "password",
                    combinator::cond(
                        password_flag,
                        self::binary_data(parser_settings).map(Into::into),
                    ),
                ),
                combinator::eof,
            )
                .parse_next(input)?;

            Ok(Connect {
                protocol_name,
                protocol_version,
                clean_start,
                client_identifier,
                properties,
                will,
                user_name,
                password,
                keep_alive,
            })
        })
    }
}

impl ConnectHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> PResult<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "CONNECT Header Flags",
            )))
            .parse_next(input)
    }
}

impl<'input> ConnectProperties<'input> {
    #[inline]
    pub fn parse<'settings, Input, Error>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]> + UpdateSlice + StreamIsPartial + Clone,
        Error: ParserError<Input>
            + AddContext<Input, StrContext>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, UnknownFormatIndicatorError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(variable_byte_integer, |input: &mut Input| {
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
                        Property::MaximumPacketSize(value) => {
                            properties.maximum_packet_size.replace(value);
                        }
                        Property::TopicAliasMaximum(value) => {
                            properties.topic_alias_maximum.replace(value);
                        }
                        Property::RequestResponseInformation(value) => {
                            properties.request_response_information.replace(value);
                        }
                        Property::RequestProblemInformation(value) => {
                            properties.request_problem_information.replace(value);
                        }
                        Property::AuthenticationMethod(value) => {
                            authentication_method.replace(value);
                        }
                        Property::AuthenticationData(value) => {
                            authentication_data.replace(value);
                        }
                        Property::UserProperty(key, value) => {
                            properties.user_properties.push((key, value))
                        }
                        _ => {
                            return Err(ErrMode::Cut(Error::assert(input, "Invalid property type")))
                        }
                    };
                }

                // It is a Protocol Error to include Authentication Data if there is no Authentication Method
                properties.authentication = match (authentication_method, authentication_data) {
                    (None, None) => None,
                    (Some(method), None) => Some(AuthenticationKind::WithoutData { method }),
                    (Some(method), Some(data)) => {
                        Some(AuthenticationKind::WithData { method, data })
                    }
                    (None, Some(_)) => {
                        return Err(ErrMode::Cut(Error::assert(
                            input,
                            "Authentication Data without Authentication Method",
                        )))
                    }
                };
                Ok(properties)
            }),
        )
        .context(StrContext::Label(type_name::<Self>()))
    }
}

impl<'input> WillProperties<'input> {
    #[inline]
    pub fn parse<'settings, Input, Error>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]> + UpdateSlice + StreamIsPartial + Clone,
        Error: ParserError<Input>
            + AddContext<Input, StrContext>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, UnknownFormatIndicatorError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(variable_byte_integer, |input: &mut Input| {
                let mut properties = Self::default();

                let mut parser = combinator::alt((
                    combinator::eof.value(None),
                    Property::parse(parser_settings).map(Some),
                ));

                while let Some(p) = parser.parse_next(input)? {
                    match p {
                        Property::WillDelayInterval(value) => {
                            properties.will_delay_interval.replace(value);
                        }
                        Property::PayloadFormatIndicator(value) => {
                            properties.payload_format_indicator.replace(value);
                        }
                        Property::MessageExpiryInterval(value) => {
                            properties.message_expiry_interval.replace(value);
                        }
                        Property::ContentType(value) => {
                            properties.content_type.replace(value);
                        }
                        Property::ResponseTopic(value) => {
                            properties.response_topic.replace(value);
                        }
                        Property::CorrelationData(value) => {
                            properties.correlation_data.replace(value);
                        }
                        Property::UserProperty(key, value) => {
                            properties.user_properties.push((key, value))
                        }
                        _ => {
                            return Err(ErrMode::Cut(Error::assert(input, "Invalid property type")))
                        }
                    }
                }

                Ok(properties)
            }),
        )
        .context(StrContext::Label(type_name::<Self>()))
    }
}
