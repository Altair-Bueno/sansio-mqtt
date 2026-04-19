use super::*;
impl PingRespHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `PINGRESP`
    /// ([§3.13.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901201),
    /// [MQTT-3.13.1-1]).
    #[inline]
    pub fn parser<Input, Error>(input: &mut (Input, usize)) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "PINGRESP Header Flags",
            )))
            .parse_next(input)
    }
}

impl PingResp {
    /// Returns a parser for the body of a `PINGRESP` packet
    /// ([§3.13](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901200)).
    ///
    /// The packet has no Variable Header or Payload, so the parser
    /// only asserts that no bytes remain.
    #[inline]
    pub fn parser<'input, 'settings, ByteInput, ByteError, BitError>(
        _parser_settings: &'settings ParserSettings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + Clone + UpdateSlice,
        ByteError: ParserError<ByteInput>,
        BitError: ParserError<(ByteInput, usize)> + ErrorConvert<ByteError>,
    {
        combinator::trace(type_name::<Self>(), combinator::eof.value(PingResp {}))
    }
}
