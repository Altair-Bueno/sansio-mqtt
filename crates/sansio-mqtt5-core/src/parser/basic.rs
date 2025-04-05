use super::*;
pub use binary::be_u16 as two_byte_integer;
pub use binary::be_u32 as four_byte_integer;

#[inline]
pub fn two_byte_integer_len_with_limits<'input, Input, Error>(
    limit: u16,
) -> impl ModalParser<Input, u16, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input> + AddContext<Input, StrContext>,
{
    two_byte_integer
        .verify(move |len| *len <= limit)
        .context(StrContext::Label("two_byte_integer_len_with_limits"))
}

#[inline]
pub fn variable_byte_integer_len_with_limits<'input, Input, Error>(
    limit: u64,
) -> impl ModalParser<Input, u64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input> + AddContext<Input, StrContext>,
{
    variable_byte_integer
        .verify(move |len| *len <= limit)
        .context(StrContext::Label("variable_byte_integer_len_with_limits"))
}

#[inline]
pub fn variable_byte_integer<'input, Input, Error>(input: &mut Input) -> ModalResult<u64, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input> + AddContext<Input, StrContext>,
{
    combinator::trace("variable_byte_integer", move |input: &mut Input| {
        let mut multiplier = 1;
        let mut value = 0;
        loop {
            let encoded_byte = token::any.parse_next(input)?;
            value += (encoded_byte & 127) as u64 * multiplier;
            if multiplier > 128 * 128 * 128 {
                return Err(ErrMode::Cut(Error::from_input(input)));
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
) -> impl ModalParser<Input, Input::Slice, Error> + use<'input, 'settings, Input, Error>
where
    Input: StreamIsPartial + Stream<Token = u8>,
    Error: ParserError<Input> + AddContext<Input, StrContext>,
{
    combinator::trace(
        "binary_data",
        binary::length_take(two_byte_integer_len_with_limits(
            parser_settings.max_bytes_binary_data,
        )),
    )
    .context(StrContext::Label("binary_data"))
    .context(StrContext::Expected(StrContextValue::Description(
        "a length prefixed slice of binary data",
    )))
}

#[inline]
pub fn string_pair<'settings, 'input, Input, Error>(
    parser_settings: &'settings Settings,
) -> impl ModalParser<Input, (MQTTString<'input>, MQTTString<'input>), Error>
       + use<'input, 'settings, Input, Error>
where
    Input: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]>,
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

impl<'input> MQTTString<'input> {
    #[inline]
    pub fn parse<Input, Error>(parser_settings: &Settings) -> impl ModalParser<Input, Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]>,
        Error: ParserError<Input>
            + FromExternalError<Input, Utf8Error>
            + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_take(two_byte_integer_len_with_limits(
                parser_settings.max_bytes_string,
            ))
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
    pub fn parse<Input, Error>(parser_settings: &Settings) -> impl ModalParser<Input, Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]>,
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
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> ModalResult<Self, Error>
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
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> ModalResult<Self, Error>
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
    pub fn parse<'input, Input, Error>(input: &mut Input) -> ModalResult<Self, Error>
    where
        Input: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]>,
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
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> ModalResult<Self, Error>
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
    ) -> impl ModalParser<ByteInput, Self, ByteError>
           + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + Clone + UpdateSlice,
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
                    |(topic_filter, (_, retain_handling, retain_as_published, no_local, qos))| {
                        Subscription {
                            topic_filter,
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

macro_rules! impl_parser_for_reason_code {
    ($name:ty) => {
        impl $name {
            #[inline]
            pub fn parse<Input, Error>(input: &mut Input) -> ModalResult<Self, Error>
            where
                Input: Stream<Token = u8> + StreamIsPartial + Clone,
                Error: ParserError<Input>
                    + FromExternalError<Input, InvalidReasonCode>
                    + AddContext<Input, StrContext>,
            {
                combinator::trace(type_name::<Self>(), token::any.try_map(Self::try_from))
                    .context(StrContext::Label(type_name::<Self>()))
                    .parse_next(input)
            }
        }
    };
}

impl_parser_for_reason_code!(ConnectReasonCode);
impl_parser_for_reason_code!(ConnackReasonCode);
impl_parser_for_reason_code!(PublishReasonCode);
impl_parser_for_reason_code!(PubAckReasonCode);
impl_parser_for_reason_code!(PubRecReasonCode);
impl_parser_for_reason_code!(PubRelReasonCode);
impl_parser_for_reason_code!(PubCompReasonCode);
impl_parser_for_reason_code!(SubAckReasonCode);
impl_parser_for_reason_code!(UnsubAckReasonCode);
impl_parser_for_reason_code!(DisconnectReasonCode);
impl_parser_for_reason_code!(AuthReasonCode);
