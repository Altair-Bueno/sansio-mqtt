use super::*;
impl SubAckHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `SUBACK`
    /// ([§3.9.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901172),
    /// [MQTT-3.9.1-1]).
    #[inline]
    pub fn parser<Input, Error>(input: &mut Bits<Input>) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<Bits<Input>> + AddContext<Bits<Input>, StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "SUBACK Header Flags",
            )))
            .parse_next(input)
    }
}

impl SubAck {
    /// Returns a parser for the body of a `SUBACK` packet
    /// ([§3.9](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901171)).
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
            + FromExternalError<ByteInput, InvalidReasonCode>
            + FromExternalError<ByteInput, Utf8StringError>
            + FromExternalError<ByteInput, TryFromIntError>
            + FromExternalError<ByteInput, TopicError>
            + FromExternalError<ByteInput, BinaryDataError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<Bits<ByteInput>> + ErrorConvert<ByteError>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                combinator::trace("Packet ID", two_byte_integer.try_map(TryInto::try_into)),
                SubAckProperties::parser(parser_settings),
                combinator::trace(
                    "reason codes",
                    combinator::repeat_till(
                        ..=parser_settings.max_subscriptions_len as usize,
                        SubAckReasonCode::parser,
                        combinator::eof,
                    ),
                ),
            )
                .map(move |(packet_id, properties, (reason_codes, _))| SubAck {
                    packet_id,
                    properties,
                    reason_codes,
                }),
        )
    }
}
