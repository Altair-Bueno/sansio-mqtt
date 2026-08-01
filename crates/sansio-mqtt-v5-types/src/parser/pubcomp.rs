use super::*;

impl PubCompHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `PUBCOMP`
    /// ([§3.7.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901151),
    /// [MQTT-3.7.1-1]).
    #[inline]
    pub fn parser<Input, Error>(input: &mut bits::Bits<Input>) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<bits::Bits<Input>> + AddContext<bits::Bits<Input>, StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "PUBCOMP Header Flags",
            )))
            .parse_next(input)
    }
}

impl PubComp {
    /// Returns a parser for the body of a `PUBCOMP` packet
    /// ([§3.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901151)).
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
            + FromExternalError<ByteInput, TopicError>
            + FromExternalError<ByteInput, TryFromIntError>
            + FromExternalError<ByteInput, BinaryDataError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<bits::Bits<ByteInput>> + ErrorConvert<ByteError>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                combinator::trace("Packet ID", two_byte_integer.try_map(TryInto::try_into)),
                // The Reason Code and Property Length can be omitted if the Reason Code is 0x00
                // (Success) and there are no Properties. In this case the PUBREC has a Remaining
                // Length of 2.
                combinator::alt((
                    (
                        combinator::empty.default_value(),
                        combinator::empty.default_value(),
                        combinator::eof,
                    ),
                    (
                        PubCompReasonCode::parser,
                        PubCompProperties::parser(parser_settings),
                        combinator::eof,
                    ),
                )),
            )
                .map(move |(packet_id, (reason_code, properties, _))| PubComp {
                    packet_id,
                    reason_code,
                    properties,
                }),
        )
    }
}
