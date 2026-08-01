use core::num::NonZero;

use super::*;

#[inline]
pub fn flags<Input, BitError, ByteError>(
    input: &mut Input,
) -> Result<(bool, bool, bool, Qos, bool, bool), ByteError>
where
    BitError: ParserError<Bits<Input>>
        + ErrorConvert<ByteError>
        + FromExternalError<Bits<Input>, InvalidQosError>
        + AddContext<Bits<Input>, StrContext>,
    ByteError: ParserError<Input>,
    Bits<Input>: Stream,
    Input: Stream<Token = u8> + StreamIsPartial + Clone,
{
    let (username_flag, password_flag, will_retain, will_qos, will_flag, clean_start, _) =
        bits::bits::<_, _, BitError, _, _>((
            bits::bool,
            bits::bool,
            bits::bool,
            Qos::parser,
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

impl Connect {
    /// Returns a parser for the body of a `CONNECT` packet
    /// ([§3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901033)).
    #[inline]
    pub fn parser<'input, 'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings ParserSettings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial
            + Stream<Token = u8, Slice = &'input [u8]>
            + BytesSource
            + Clone
            + UpdateSlice,
        ByteError: ParserError<ByteInput>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, InvalidQosError>
            + FromExternalError<ByteInput, InvalidPropertyTypeError>
            + FromExternalError<ByteInput, PropertiesError>
            + FromExternalError<ByteInput, UnknownFormatIndicatorError>
            + FromExternalError<ByteInput, Utf8StringError>
            + FromExternalError<ByteInput, TopicError>
            + FromExternalError<ByteInput, TryFromIntError>
            + FromExternalError<ByteInput, BinaryDataError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<Bits<ByteInput>>
            + ErrorConvert<ByteError>
            + FromExternalError<Bits<ByteInput>, InvalidQosError>
            + AddContext<Bits<ByteInput>, StrContext>,
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
                combinator::trace("Protocol name", Utf8String::parser(parser_settings)),
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
                combinator::trace("Keep alive", self::two_byte_integer.map(NonZero::new)),
                ConnectProperties::parser(parser_settings),
                combinator::trace("Client identifier", Utf8String::parser(parser_settings)),
            )
                .parse_next(input)?;

            let (will, user_name, password, _) = (
                combinator::trace(
                    "will",
                    combinator::cond(
                        will_flag,
                        (
                            WillProperties::parser(parser_settings),
                            Topic::parser(parser_settings),
                            BinaryData::parser(parser_settings),
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
                        Utf8String::parser(parser_settings).map(Into::into),
                    ),
                ),
                combinator::trace(
                    "password",
                    combinator::cond(
                        password_flag,
                        BinaryData::parser(parser_settings).map(Into::into),
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
    /// Parses the 4-bit Fixed Header flags for `CONNECT`
    /// ([§3.1.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901034),
    /// [MQTT-3.1.1-1]).
    #[inline]
    pub fn parser<Input, Error>(input: &mut Bits<Input>) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<Bits<Input>> + AddContext<Bits<Input>, StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "CONNECT Header Flags",
            )))
            .parse_next(input)
    }
}

impl ConnectProperties {
    /// Returns a parser for the `CONNECT` properties section
    /// ([§3.1.2.11](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901046)).
    #[inline]
    pub fn parser<'input, 'settings, Input, Error>(
        parser_settings: &'settings ParserSettings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]>
            + BytesSource
            + UpdateSlice
            + StreamIsPartial
            + Clone,
        Error: ParserError<Input>
            + AddContext<Input, StrContext>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, PropertiesError>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + FromExternalError<Input, Utf8StringError>
            + FromExternalError<Input, TryFromIntError>
            + FromExternalError<Input, TopicError>
            + FromExternalError<Input, BinaryDataError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(
                variable_byte_integer,
                (
                    combinator::repeat(.., Property::parser(parser_settings))
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
                                    Property::SessionExpiryInterval(value) => set_once(&mut properties.session_expiry_interval, value, property_type)?,Property::ReceiveMaximum(value) => set_once(&mut properties.receive_maximum, value, property_type)?,Property::MaximumPacketSize(value) => set_once(&mut properties.maximum_packet_size, value, property_type)?,Property::TopicAliasMaximum(value) => set_once(&mut properties.topic_alias_maximum, value, property_type)?,Property::RequestResponseInformation(value) => set_once(&mut properties.request_response_information, value, property_type)?,Property::RequestProblemInformation(value) => set_once(&mut properties.request_problem_information, value, property_type)?,Property::UserProperty(key, value) => push_capped(&mut properties.user_properties, (key, value), parser_settings.max_user_properties_len, PropertiesError::from(TooManyUserPropertiesError))?,Property::AuthenticationMethod(value) => set_once(&mut authentication_method, value, property_type)?,Property::AuthenticationData(value) => set_once(&mut authentication_data, value, property_type)?,_ => {
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

impl WillProperties {
    /// Returns a parser for the Will Properties section of a `CONNECT` payload
    /// ([§3.1.3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901060)).
    #[inline]
    pub fn parser<'input, 'settings, Input, Error>(
        parser_settings: &'settings ParserSettings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]>
            + BytesSource
            + UpdateSlice
            + StreamIsPartial
            + Clone,
        Error: ParserError<Input>
            + AddContext<Input, StrContext>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, PropertiesError>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + FromExternalError<Input, Utf8StringError>
            + FromExternalError<Input, TopicError>
            + FromExternalError<Input, TryFromIntError>
            + FromExternalError<Input, BinaryDataError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(
                variable_byte_integer,
                (
                    combinator::repeat(.., Property::parser(parser_settings)).try_fold(
                        Self::default,
                        |mut properties, property| {
                            let property_type = PropertyType::from(&property);
                            match property {
                                Property::WillDelayInterval(value) => set_once(
                                    &mut properties.will_delay_interval,
                                    value,
                                    property_type,
                                )?,
                                Property::PayloadFormatIndicator(value) => set_once(
                                    &mut properties.payload_format_indicator,
                                    value,
                                    property_type,
                                )?,
                                Property::MessageExpiryInterval(value) => set_once(
                                    &mut properties.message_expiry_interval,
                                    value,
                                    property_type,
                                )?,
                                Property::ContentType(value) => {
                                    set_once(&mut properties.content_type, value, property_type)?
                                }
                                Property::ResponseTopic(value) => {
                                    set_once(&mut properties.response_topic, value, property_type)?
                                }
                                Property::CorrelationData(value) => set_once(
                                    &mut properties.correlation_data,
                                    value,
                                    property_type,
                                )?,
                                Property::UserProperty(key, value) => push_capped(
                                    &mut properties.user_properties,
                                    (key, value),
                                    parser_settings.max_user_properties_len,
                                    PropertiesError::from(TooManyUserPropertiesError),
                                )?,
                                _ => {
                                    return Err(PropertiesError::from(UnsupportedPropertyError {
                                        property_type,
                                    }));
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
