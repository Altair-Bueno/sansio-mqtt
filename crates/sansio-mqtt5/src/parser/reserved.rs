use super::*;

impl ReservedHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> ModalResult<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "RESEVED Header Flags",
            )))
            .parse_next(input)
    }
}

impl Reserved {
    #[inline]
    pub fn parse<ByteInput, ByteError>(
        _parser_settings: &Settings,
    ) -> impl ModalParser<ByteInput, Self, ByteError> + use<'_, ByteInput, ByteError>
    where
        ByteInput: StreamIsPartial + Stream + Clone + UpdateSlice,
        ByteError: ParserError<ByteInput>,
    {
        combinator::trace(type_name::<Self>(), combinator::eof.value(Reserved {}))
    }
}
