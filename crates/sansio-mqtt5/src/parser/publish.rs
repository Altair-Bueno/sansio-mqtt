use super::*;

impl PublishHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> PResult<Self, Error>
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
            + FromExternalError<ByteInput, UnknownFormatIndicatorError>
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
                        .verify_map(NonZero::new)
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
                combinator::trace("payload", combinator::rest.output_into()).parse_next(input)?;
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
                        Property::PayloadFormatIndicator(value) => {
                            properties.payload_format_indicator.replace(value);
                        }
                        Property::MessageExpiryInterval(value) => {
                            properties.message_expiry_interval.replace(value);
                        }
                        Property::TopicAlias(value) => {
                            properties.topic_alias.replace(value);
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
                        Property::SubscriptionIdentifier(value) => {
                            properties.subscription_identifier.replace(value);
                        }
                        Property::ContentType(value) => {
                            properties.content_type.replace(value);
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
