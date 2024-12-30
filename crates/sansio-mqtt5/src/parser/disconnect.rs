use super::*;
impl DisconnectHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> PResult<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "DISCONNECT Header Flags",
            )))
            .parse_next(input)
    }
}

impl<'input> Disconnect<'input> {
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
            // The Reason Code and Property Length can be omitted if the Reason Code is 0x00 (Normal disconnection) and there are no Properties. In this case the DISCONNECT has a Remaining Length of 0
            combinator::alt((
                (
                    combinator::empty.value(ReasonCode::Success),
                    combinator::empty.default_value(),
                    combinator::eof,
                ),
                (
                    ReasonCode::parse_disconnect,
                    DisconnectProperties::parse(parser_settings),
                    combinator::eof,
                ),
            ))
            .map(move |(reason_code, properties, _)| Disconnect {
                reason_code,
                properties,
            }),
        )
    }
}

impl<'input> DisconnectProperties<'input> {
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

            let mut parser = combinator::alt((
                combinator::eof.value(None),
                Property::parse(parser_settings).map(Some),
            ));

            while let Some(p) = parser.parse_next(input)? {
                match p {
                    Property::SessionExpiryInterval(value) => {
                        properties.session_expiry_interval.replace(value);
                    }
                    Property::ReasonString(value) => {
                        properties.reason_string.replace(value);
                    }
                    Property::UserProperty(key, value) => {
                        properties.user_properties.push((key, value))
                    }
                    Property::ServerReference(value) => {
                        properties.server_reference.replace(value);
                    }
                    _ => return Err(ErrMode::Cut(Error::assert(input, "Invalid property type"))),
                }
            }

            Ok(properties)
        })
        .context(StrContext::Label(type_name::<Self>()))
    }
}
