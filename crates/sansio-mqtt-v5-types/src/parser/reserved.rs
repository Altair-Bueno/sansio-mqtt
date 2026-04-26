use super::*;

impl ReservedHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for the Reserved (0) Control Packet
    /// type ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)).
    #[inline]
    pub fn parser<Input, Error>(input: &mut (Input, usize)) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "RESERVED Header Flags",
            )))
            .parse_next(input)
    }
}

impl Reserved {
    /// Returns a parser for the body of a Reserved (0) Control Packet
    /// ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)).
    ///
    /// The packet has no Variable Header or Payload, so the parser
    /// only asserts that no bytes remain.
    #[inline]
    pub fn parser<ByteInput, ByteError>(
        _parser_settings: &ParserSettings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'_, ByteInput, ByteError>
    where
        ByteInput: StreamIsPartial + Stream + Clone + UpdateSlice,
        ByteError: ParserError<ByteInput>,
    {
        combinator::trace(type_name::<Self>(), combinator::eof.value(Reserved {}))
    }
}
