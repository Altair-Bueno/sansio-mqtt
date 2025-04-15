use super::*;

impl PublishHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)>
            + FromExternalError<(Input, usize), InvalidQosError>
            + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                combinator::trace("dup", bits::bool).context(StrContext::Label("dup")),
                Qos::parse,
                combinator::trace("retain", bits::bool).context(StrContext::Label("retain")),
            )
                // The DUP flag MUST be set to 0 for all QoS 0 messages [MQTT-3.3.1-2].
                .verify_map(|(dup, qos, retain)| {
                    let kind = match (dup, GuaranteedQoS::try_from(qos)) {
                        (false, Err(_)) => PublishHeaderFlagsKind::Simple,
                        (dup, Ok(qos)) => PublishHeaderFlagsKind::Advanced { qos, dup },
                        _ => return None,
                    };
                    Some(PublishHeaderFlags { kind, retain })
                }),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "PUBLISH Header Flags",
        )))
        .parse_next(input)
    }
}

impl<'input> Publish<'input> {
    #[inline]
    pub fn parse<'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings Settings,
        header_flags: PublishHeaderFlags,
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
            + FromExternalError<ByteInput, MQTTStringError>
            + FromExternalError<ByteInput, PublishTopicError>
            + FromExternalError<ByteInput, TryFromIntError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<(ByteInput, usize)> + ErrorConvert<ByteError>,
    {
        combinator::trace(type_name::<Self>(), move |input: &mut ByteInput| {
            let PublishHeaderFlags { kind, retain } = header_flags.clone();
            let topic = PublishTopic::parse(parser_settings).parse_next(input)?;
            let kind = match kind {
                PublishHeaderFlagsKind::Simple => PublishKind::FireAndForget,
                PublishHeaderFlagsKind::Advanced { qos, dup } => {
                    let packet_id = two_byte_integer
                        .try_map(TryInto::try_into)
                        .parse_next(input)?;
                    PublishKind::Repetible {
                        packet_id,
                        qos,
                        dup,
                    }
                }
            };
            let properties = PublishProperties::parse(parser_settings).parse_next(input)?;
            let payload =
                combinator::trace("payload", token::rest.output_into()).parse_next(input)?;
            Ok(Publish {
                kind,
                retain,
                topic,
                properties,
                payload,
            })
        })
    }
}

impl<'input> PublishProperties<'input> {
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
            + FromExternalError<Input, MQTTStringError>
            + FromExternalError<Input, PublishTopicError>
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
                                Property::TopicAlias(value) => match &mut properties.topic_alias {
                                    slot @ None => *slot = Some(value),
                                    _ => {
                                        return Err(PropertiesError::from(
                                            DuplicatedPropertyError { property_type },
                                        ))
                                    }
                                },
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
                                Property::SubscriptionIdentifier(value) => {
                                    match &mut properties.subscription_identifier {
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
