use super::*;

impl<'input> ControlPacket<'input> {
    #[inline]
    pub fn parse<'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + Clone + UpdateSlice,
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
        BitError: ParserError<(ByteInput, usize)>
            + FromExternalError<(ByteInput, usize), InvalidControlPacketTypeError>
            + FromExternalError<(ByteInput, usize), InvalidRetainHandlingError>
            + FromExternalError<(ByteInput, usize), InvalidQosError>
            + ErrorConvert<ByteError>
            + AddContext<(ByteInput, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), |input: &mut ByteInput| {
            let control_packet_type =
                combinator::peek(bits::bits(ControlPacketType::parse::<_, BitError>))
                    .parse_next(input)?;

            let remaining_len_parser = combinator::trace(
                "Remaining length",
                self::variable_byte_integer_len_with_limits(parser_settings.max_remaining_bytes),
            )
            .context(StrContext::Label("Remaining length"));

            match control_packet_type {
                ControlPacketType::Reserved => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        ReservedHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(remaining_len_parser, Reserved::parse(parser_settings))
                        .map(ControlPacket::Reserved)
                        .parse_next(input)
                }
                ControlPacketType::Connect => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        ConnectHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        Connect::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::Connect)
                    .parse_next(input)
                }
                ControlPacketType::ConnAck => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        ConnAckHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        ConnAck::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::ConnAck)
                    .parse_next(input)
                }
                ControlPacketType::Publish => {
                    let (_, header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PublishHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        Publish::parse::<_, _, BitError>(parser_settings, header_flags),
                    )
                    .map(ControlPacket::Publish)
                    .parse_next(input)
                }
                ControlPacketType::PubAck => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PubAckHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        PubAck::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::PubAck)
                    .parse_next(input)
                }
                ControlPacketType::PubRec => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PubRecHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        PubRec::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::PubRec)
                    .parse_next(input)
                }
                ControlPacketType::PubRel => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PubRelHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        PubRel::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::PubRel)
                    .parse_next(input)
                }
                ControlPacketType::PubComp => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PubCompHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        PubComp::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::PubComp)
                    .parse_next(input)
                }
                ControlPacketType::Subscribe => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        SubscribeHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        Subscribe::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::Subscribe)
                    .parse_next(input)
                }
                ControlPacketType::SubAck => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        SubAckHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        SubAck::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::SubAck)
                    .parse_next(input)
                }
                ControlPacketType::Unsubscribe => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        UnsubscribeHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        Unsubscribe::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::Unsubscribe)
                    .parse_next(input)
                }
                ControlPacketType::UnsubAck => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        UnsubAckHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        UnsubAck::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::UnsubAck)
                    .parse_next(input)
                }
                ControlPacketType::PingReq => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PingReqHeaderFlags::parse,
                    ))
                    .parse_next(input)?;

                    binary::length_and_then(
                        remaining_len_parser,
                        PingReq::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::PingReq)
                    .parse_next(input)
                }
                ControlPacketType::PingResp => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PingRespHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        PingResp::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::PingResp)
                    .parse_next(input)
                }
                ControlPacketType::Disconnect => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        DisconnectHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        Disconnect::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::Disconnect)
                    .parse_next(input)
                }
                ControlPacketType::Auth => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        AuthHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    binary::length_and_then(
                        remaining_len_parser,
                        Auth::parse::<_, _, BitError>(parser_settings),
                    )
                    .map(ControlPacket::Auth)
                    .parse_next(input)
                }
            }
        })
    }
}
