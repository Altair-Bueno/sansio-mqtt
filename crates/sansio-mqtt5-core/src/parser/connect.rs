use super::*;

#[inline]
pub fn flags<Input, BitError, ByteError>(
    input: &mut Input,
) -> Result<(bool, bool, bool, Qos, bool, bool), ByteError>
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
            + FromExternalError<ByteInput, PropertiesError>
            + FromExternalError<ByteInput, UnknownFormatIndicatorError>
            + FromExternalError<ByteInput, Utf8StringError>
            + FromExternalError<ByteInput, TopicError>
            + FromExternalError<ByteInput, TryFromIntError>
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
                combinator::trace("Protocol name", Utf8String::parse(parser_settings)),
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
                combinator::trace("Client identifier", Utf8String::parse(parser_settings)),
            )
                .parse_next(input)?;

            let (will, user_name, password, _) = (
                combinator::trace(
                    "will",
                    combinator::cond(
                        will_flag,
                        (
                            WillProperties::parse(parser_settings),
                            Topic::parse(parser_settings).map(Into::into),
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
                        Utf8String::parse(parser_settings).map(Into::into),
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
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> Result<Self, Error>
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
            + FromExternalError<Input, PropertiesError>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + FromExternalError<Input, Utf8StringError>
            + FromExternalError<Input, TryFromIntError>
            + FromExternalError<Input, TopicError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(
                variable_byte_integer,
                (
                    combinator::repeat(.., Property::parse(parser_settings))
                        .try_fold(
                            Default::default,
                            |(
                                mut properties,
                                mut authentication_data,
                                mut authentication_method,
                            ): (Self, Option<_>, Option<_>),
                             property| {
                                let property_type = PropertyType::from(&property);
                                match property {
                                    Property::SessionExpiryInterval(value) => {
                                        match &mut properties.session_expiry_interval {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::ReceiveMaximum(value) => {
                                        match &mut properties.receive_maximum {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::MaximumPacketSize(value) => {
                                        match &mut properties.maximum_packet_size {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::TopicAliasMaximum(value) => {
                                        match &mut properties.topic_alias_maximum {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::RequestResponseInformation(value) => {
                                        match &mut properties.request_response_information {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::RequestProblemInformation(value) => {
                                        match &mut properties.request_problem_information {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::UserProperty(key, value) => {
                                        if properties.user_properties.len()
                                            >= parser_settings.max_user_properties_len
                                        {
                                            return Err(PropertiesError::from(
                                                TooManyUserPropertiesError,
                                            ));
                                        }
                                        properties.user_properties.push((key, value))
                                    }
                                    Property::AuthenticationMethod(value) => {
                                        match &mut authentication_method {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::AuthenticationData(value) => {
                                        match &mut authentication_data {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    _ => {
                                        return Err(PropertiesError::from(
                                            UnsupportedPropertyError { property_type },
                                        ))
                                    }
                                };
                                Ok((properties, authentication_data, authentication_method))
                            },
                        )
                        .try_map(
                            |(mut properties, authentication_data, authentication_method)| -> Result<_, PropertiesError> {
                                // It is a Protocol Error to include Authentication Data if there is no Authentication Method
                                properties.authentication = AuthenticationKind::try_from_parts((
                                    authentication_method,
                                    authentication_data,
                                ))?;
                                Ok(properties)
                            },
                        ),
                    combinator::eof,
                )
                    .map(|(properties, _)| properties),
            ),
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
            + FromExternalError<Input, PropertiesError>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + FromExternalError<Input, Utf8StringError>
            + FromExternalError<Input, TopicError>
            + FromExternalError<Input, TryFromIntError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(
                variable_byte_integer,
                (
                    combinator::repeat(.., Property::parse(parser_settings)).try_fold(
                        Self::default,
                        |mut properties, property| {
                            let property_type = PropertyType::from(&property);
                            match property {
                                Property::WillDelayInterval(value) => {
                                    match &mut properties.will_delay_interval {
                                        slot @ None => *slot = Some(value),
                                        _ => {
                                            return Err(PropertiesError::from(
                                                DuplicatedPropertyError { property_type },
                                            ))
                                        }
                                    }
                                }
                                Property::PayloadFormatIndicator(value) => {
                                    match &mut properties.payload_format_indicator {
                                        slot @ None => *slot = Some(value),
                                        _ => {
                                            return Err(PropertiesError::from(
                                                DuplicatedPropertyError { property_type },
                                            ))
                                        }
                                    }
                                }
                                Property::MessageExpiryInterval(value) => {
                                    match &mut properties.message_expiry_interval {
                                        slot @ None => *slot = Some(value),
                                        _ => {
                                            return Err(PropertiesError::from(
                                                DuplicatedPropertyError { property_type },
                                            ))
                                        }
                                    }
                                }
                                Property::ContentType(value) => {
                                    match &mut properties.content_type {
                                        slot @ None => *slot = Some(value),
                                        _ => {
                                            return Err(PropertiesError::from(
                                                DuplicatedPropertyError { property_type },
                                            ))
                                        }
                                    }
                                }
                                Property::ResponseTopic(value) => {
                                    match &mut properties.response_topic {
                                        slot @ None => *slot = Some(value),
                                        _ => {
                                            return Err(PropertiesError::from(
                                                DuplicatedPropertyError { property_type },
                                            ))
                                        }
                                    }
                                }
                                Property::CorrelationData(value) => {
                                    match &mut properties.correlation_data {
                                        slot @ None => *slot = Some(value),
                                        _ => {
                                            return Err(PropertiesError::from(
                                                DuplicatedPropertyError { property_type },
                                            ))
                                        }
                                    }
                                }
                                Property::UserProperty(key, value) => {
                                    if properties.user_properties.len()
                                        >= parser_settings.max_user_properties_len
                                    {
                                        return Err(PropertiesError::from(
                                            TooManyUserPropertiesError,
                                        ));
                                    }
                                    properties.user_properties.push((key, value))
                                }
                                _ => {
                                    return Err(PropertiesError::from(UnsupportedPropertyError {
                                        property_type,
                                    }))
                                }
                            };
                            Ok(properties)
                        },
                    ),
                    combinator::eof,
                )
                    .map(|(properties, _)| properties),
            ),
        )
        .context(StrContext::Label(type_name::<Self>()))
    }
}
