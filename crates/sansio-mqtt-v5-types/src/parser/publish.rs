use super::*;

impl PublishHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `PUBLISH`
    /// ([§3.3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901101)),
    /// including DUP, QoS level, and RETAIN.
    ///
    /// Enforces [MQTT-3.3.1-2]: the DUP flag MUST be `0` for QoS 0 packets.
    #[inline]
    pub fn parser<Input, Error>(input: &mut Bits<Input>) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<Bits<Input>>
            + FromExternalError<Bits<Input>, InvalidQosError>
            + AddContext<Bits<Input>, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                combinator::trace("dup", bits::bool).context(StrContext::Label("dup")),
                Qos::parser,
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

impl Publish {
    /// Returns a parser for the body of a `PUBLISH` packet
    /// ([§3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901100)).
    ///
    /// The pre-parsed `header_flags` disambiguates whether a Packet
    /// Identifier is present (QoS > 0).
    #[inline]
    pub fn parser<'input, 'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings ParserSettings,
        header_flags: PublishHeaderFlags,
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
        BitError: ParserError<Bits<ByteInput>> + ErrorConvert<ByteError>,
    {
        combinator::trace(type_name::<Self>(), move |input: &mut ByteInput| {
            let PublishHeaderFlags { kind, retain } = header_flags;
            let topic = Topic::parser(parser_settings).parse_next(input)?;
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
            let properties = PublishProperties::parser(parser_settings).parse_next(input)?;
            let payload = Payload::parser(parser_settings).parse_next(input)?;
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

impl PublishProperties {
    /// Returns a parser for the `PUBLISH` properties section
    /// ([§3.3.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901109)).
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
                                Property::TopicAlias(value) => {
                                    set_once(&mut properties.topic_alias, value, property_type)?
                                }
                                Property::ResponseTopic(value) => {
                                    set_once(&mut properties.response_topic, value, property_type)?
                                }
                                Property::CorrelationData(value) => set_once(
                                    &mut properties.correlation_data,
                                    value,
                                    property_type,
                                )?,
                                Property::SubscriptionIdentifier(value) => push_capped(
                                    &mut properties.subscription_identifiers,
                                    value,
                                    parser_settings.max_subscription_identifiers_len,
                                    PropertiesError::from(TooManySubscriptionIdentifiersError),
                                )?,
                                Property::ContentType(value) => {
                                    set_once(&mut properties.content_type, value, property_type)?
                                }
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
