use super::*;

impl AuthHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> ModalResult<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "AUTH Header Flags",
            )))
            .parse_next(input)
    }
}

impl<'input> Auth<'input> {
    #[inline]
    pub fn parse<'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings Settings,
    ) -> impl ModalParser<ByteInput, Self, ByteError>
           + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + Clone + UpdateSlice,
        ByteError: ParserError<ByteInput>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, InvalidQosError>
            + FromExternalError<ByteInput, InvalidPropertyTypeError>
            + FromExternalError<ByteInput, PropertiesError>
            + FromExternalError<ByteInput, UnknownFormatIndicatorError>
            + FromExternalError<ByteInput, InvalidReasonCode>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<(ByteInput, usize)> + ErrorConvert<ByteError>,
    {
        combinator::trace(
            type_name::<Self>(),
            combinator::alt((
                (
                    combinator::empty.default_value(),
                    combinator::empty.default_value(),
                    combinator::eof,
                ),
                (
                    AuthReasonCode::parse,
                    AuthProperties::parse(parser_settings),
                    combinator::eof,
                ),
            ))
            .map(move |(reason_code, properties, _)| Auth {
                reason_code,
                properties,
            }),
        )
    }
}

impl<'input> AuthProperties<'input> {
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
                                    Property::ReasonString(value) => {
                                        match &mut properties.reason_string {
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
