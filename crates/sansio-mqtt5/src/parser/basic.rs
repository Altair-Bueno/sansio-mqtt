use super::*;
pub use binary::be_u16 as two_byte_integer;
pub use binary::be_u32 as four_byte_integer;

#[inline]
pub fn variable_byte_integer<'input, Input, Error>(input: &mut Input) -> PResult<u64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8> + 'input,
    Error: ParserError<Input> + AddContext<Input, StrContext>,
{
    combinator::trace("variable_byte_integer", move |input: &mut Input| {
        let mut multiplier = 1;
        let mut value = 0;
        loop {
            let encoded_byte = token::any.parse_next(input)?;
            value += (encoded_byte & 127) as u64 * multiplier;
            if multiplier > 128 * 128 * 128 {
                return Err(ErrMode::Cut(Error::from_error_kind(
                    input,
                    ErrorKind::Verify,
                )));
            }
            multiplier *= 128;
            if encoded_byte & 128 == 0 {
                break;
            }
        }
        Ok(value)
    })
    .context(StrContext::Label("variable_byte_integer"))
    .context(StrContext::Expected(StrContextValue::Description(
        "a Variable Byte Integer",
    )))
    .parse_next(input)
}

#[inline]
pub fn binary_data<'settings, 'input, Input, Error>(
    parser_settings: &'settings Settings,
) -> impl Parser<Input, Input::Slice, Error> + use<'input, 'settings, Input, Error>
where
    Input: StreamIsPartial + Stream<Token = u8> + 'input,
    Error: ParserError<Input> + AddContext<Input, StrContext>,
{
    combinator::trace(
        "binary_data",
        self::length_take_with_limits(two_byte_integer, parser_settings.max_bytes_binary_data),
    )
    .context(StrContext::Label("binary_data"))
    .context(StrContext::Expected(StrContextValue::Description(
        "a length prefixed slice of binary data",
    )))
}

#[inline]
pub fn string_pair<'settings, 'input, Input, Error>(
    parser_settings: &'settings Settings,
) -> impl Parser<Input, (MQTTString<'input>, MQTTString<'input>), Error>
       + use<'input, 'settings, Input, Error>
where
    Input: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + 'input,
    Error: ParserError<Input> + FromExternalError<Input, Utf8Error> + AddContext<Input, StrContext>,
{
    combinator::trace(
        "string_pair",
        (
            MQTTString::parse(parser_settings),
            MQTTString::parse(parser_settings),
        ),
    )
    .context(StrContext::Label("string_pair"))
}

#[inline]
pub fn length_take_with_limits<Input, Count, Error, CountParser>(
    mut count: CountParser,
    limit: impl ToUsize,
) -> impl Parser<Input, <Input as Stream>::Slice, Error>
where
    Input: StreamIsPartial + Stream,
    Count: ToUsize,
    CountParser: Parser<Input, Count, Error>,
    Error: ParserError<Input> + AddContext<Input, StrContext>,
{
    combinator::trace("length_take_with_limits", move |i: &mut Input| {
        let length = count.parse_next(i)?;
        let length = length.to_usize();
        if length > limit.to_usize() {
            // TODO: improve errors
            return Err(ErrMode::Cut(Error::from_error_kind(i, ErrorKind::Verify)));
        }
        token::take(length).parse_next(i)
    })
    .context(StrContext::Label("length_take_with_limits"))
    .context(StrContext::Expected(StrContextValue::Description(
        "a length prefixed slice",
    )))
}

impl<'input> MQTTString<'input> {
    #[inline]
    pub fn parse<Input, Error>(parser_settings: &Settings) -> impl Parser<Input, Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + 'input,
        Error: ParserError<Input>
            + FromExternalError<Input, Utf8Error>
            + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            self::length_take_with_limits(two_byte_integer, parser_settings.max_bytes_string)
                .try_map(str::from_utf8)
                .verify_map(Self::new),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a length prefixed MQTT string",
        )))
    }
}

impl<'input> PublishTopic<'input> {
    #[inline]
    pub fn parse<Input, Error>(parser_settings: &Settings) -> impl Parser<Input, Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + 'input,
        Error: ParserError<Input>
            + FromExternalError<Input, Utf8Error>
            + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            MQTTString::parse(parser_settings).verify_map(|s| s.try_into().ok()),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a topic string",
        )))
    }
}

impl ControlPacketType {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> PResult<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)>
            + FromExternalError<(Input, usize), InvalidControlPacketTypeError>
            + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            bits::take::<_, u8, _, _>(4usize).try_map(TryInto::try_into),
        )
        .context(StrContext::Label("control_packet_type"))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Control Packet Type",
        )))
        .parse_next(input)
    }
}

impl Qos {
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
            bits::take::<_, u8, _, _>(2usize).try_map(TryInto::try_into),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a QoS Level",
        )))
        .parse_next(input)
    }
}
impl FormatIndicator {
    #[inline]
    pub fn parse<'input, Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + 'input,
        Error: ParserError<Input>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + AddContext<Input, StrContext>,
    {
        combinator::trace(type_name::<Self>(), token::any.try_map(TryInto::try_into))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "a Format Indicator property value",
            )))
            .parse_next(input)
    }
}
impl RetainHandling {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> PResult<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)>
            + FromExternalError<(Input, usize), InvalidRetainHandlingError>
            + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            bits::take::<_, u8, _, _>(2usize).try_map(TryInto::try_into),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Retain Handling property value",
        )))
        .parse_next(input)
    }
}

impl<'input> Subscription<'input> {
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
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<(ByteInput, usize)>
            + ErrorConvert<ByteError>
            + FromExternalError<(ByteInput, usize), InvalidQosError>
            + FromExternalError<(ByteInput, usize), InvalidRetainHandlingError>
            + AddContext<(ByteInput, usize), StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                MQTTString::parse(parser_settings),
                bits::bits::<_, _, BitError, _, _>((
                    bits::pattern(0u8, 2usize),
                    RetainHandling::parse,
                    bits::bool,
                    bits::bool,
                    Qos::parse,
                )),
            )
                .map(
                    |(topic, (_, retain_handling, retain_as_published, no_local, qos))| {
                        Subscription {
                            topic,
                            qos,
                            no_local,
                            retain_as_published,
                            retain_handling,
                        }
                    },
                ),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Subscription",
        )))
    }
}

impl ReasonCode {
    #[inline(always)]
    pub fn parse_auth<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_auth),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for an AUTH packet",
        )))
        .parse_next(input)
    }

    #[inline(always)]
    pub fn parse_disconnect<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_disconnect),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for a DISCONNECT packet",
        )))
        .parse_next(input)
    }

    #[inline(always)]
    pub fn parse_pubcomp<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_pubcomp),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for a PUBCOMP packet",
        )))
        .parse_next(input)
    }

    #[inline(always)]
    pub fn parse_pubrel<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_pubrel),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for a PUBREL packet",
        )))
        .parse_next(input)
    }

    #[inline(always)]
    pub fn parse_pubrec<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_pubrec),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for a PUBREC packet",
        )))
        .parse_next(input)
    }

    #[inline(always)]
    pub fn parse_puback<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_puback),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for a PUBACK packet",
        )))
        .parse_next(input)
    }

    #[inline(always)]
    pub fn parse_connack<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_connack),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for a CONNACK packet",
        )))
        .parse_next(input)
    }
    #[inline(always)]
    pub fn parse_suback<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_suback),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for a SUBACK packet",
        )))
        .parse_next(input)
    }
    #[inline(always)]
    pub fn parse_unsuback<Input, Error>(input: &mut Input) -> PResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8>,
        Error: ParserError<Input> + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            token::any.verify_map(ReasonCode::from_unsuback),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Reason Code for an UNSUBACK packet",
        )))
        .parse_next(input)
    }
}