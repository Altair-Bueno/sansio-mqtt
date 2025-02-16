use super::*;

impl UnsubscribeHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> ModalResult<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            bits::pattern(0b0000_0010, 4usize).value(Self),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "UNSUBSCRIBE Header Flags",
        )))
        .parse_next(input)
    }
}

impl<'input> Unsubscribe<'input> {
    #[inline]
    pub fn parse<'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings Settings,
    ) -> impl ModalParser<ByteInput, Self, ByteError>
           + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + Clone + UpdateSlice,
        ByteError: ParserError<ByteInput>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, InvalidQosError>
            + FromExternalError<ByteInput, InvalidPropertyTypeError>
            + FromExternalError<ByteInput, PropertiesError>
            + FromExternalError<ByteInput, UnknownFormatIndicatorError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<(ByteInput, usize)> + ErrorConvert<ByteError>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                combinator::trace("Packet ID", two_byte_integer.verify_map(NonZero::new)),
                UnsubscribeProperties::parse(parser_settings),
                combinator::trace(
                    "topics",
                    combinator::repeat_till(
                        1..=parser_settings.max_subscriptions_len as usize,
                        MQTTString::parse(parser_settings),
                        combinator::eof,
                    ),
                ),
            )
                .map(move |(packet_id, properties, (topics, _))| Unsubscribe {
                    packet_id,
                    topics: Vec1::try_from_vec(topics)
                        .expect("topics length is guaranteed to be at least 1"),
                    properties,
                }),
        )
    }
}

impl<'input> UnsubscribeProperties<'input> {
    #[inline]
    pub fn parse<'settings, Input, Error>(
        parser_settings: &'settings Settings,
    ) -> impl ModalParser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]> + UpdateSlice + StreamIsPartial + Clone,
        Error: ParserError<Input>
            + AddContext<Input, StrContext>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, PropertiesError>
            + FromExternalError<Input, UnknownFormatIndicatorError>,
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
