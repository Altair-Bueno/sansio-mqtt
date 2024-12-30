use super::*;

impl<'input> ControlPacket<'input> {
    #[inline]
    pub fn parse<'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + Clone + UpdateSlice,
        ByteError: ParserError<ByteInput>
            + ParserError<ByteInput::Slice>
            + FromExternalError<ByteInput::Slice, Utf8Error>
            + FromExternalError<ByteInput::Slice, InvalidQosError>
            + FromExternalError<ByteInput::Slice, InvalidPropertyTypeError>
            + FromExternalError<ByteInput::Slice, UnknownFormatIndicatorError>
            + AddContext<ByteInput, StrContext>
            + AddContext<ByteInput::Slice, StrContext>,
        BitError: ParserError<(ByteInput, usize)>
            + ParserError<(ByteInput::Slice, usize)>
            + ErrorConvert<ByteError>
            + FromExternalError<(ByteInput, usize), InvalidQosError>
            + FromExternalError<(ByteInput, usize), InvalidControlPacketTypeError>
            + FromExternalError<(ByteInput::Slice, usize), InvalidRetainHandlingError>
            + FromExternalError<(ByteInput::Slice, usize), InvalidQosError>
            + AddContext<(ByteInput, usize), StrContext>
            + AddContext<(ByteInput::Slice, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), |input: &mut ByteInput| {
            let control_packet_type =
                combinator::peek(bits::bits(ControlPacketType::parse::<_, BitError>))
                    .parse_next(input)?;

            let remaining_parser = combinator::trace(
                "Remaining length",
                self::length_take_with_limits(
                    self::variable_byte_integer,
                    parser_settings.max_remaining_bytes,
                ),
            )
            .context(StrContext::Label("Remaining length"));

            match control_packet_type {
                ControlPacketType::Reserved => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        ReservedHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(Reserved::parse(parser_settings))
                        .map(ControlPacket::Reserved)
                        .parse_next(input)
                }
                ControlPacketType::Connect => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        ConnectHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(Connect::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::Connect)
                        .parse_next(input)
                }
                ControlPacketType::ConnAck => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        ConnAckHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(ConnAck::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::ConnAck)
                        .parse_next(input)
                }
                ControlPacketType::Publish => {
                    let (_, header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PublishHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(Publish::parse::<_, _, BitError>(
                            parser_settings,
                            header_flags,
                        ))
                        .map(ControlPacket::Publish)
                        .parse_next(input)
                }
                ControlPacketType::PubAck => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PubAckHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(PubAck::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::PubAck)
                        .parse_next(input)
                }
                ControlPacketType::PubRec => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PubRecHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(PubRec::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::PubRec)
                        .parse_next(input)
                }
                ControlPacketType::PubRel => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PubRelHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(PubRel::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::PubRel)
                        .parse_next(input)
                }
                ControlPacketType::PubComp => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PubCompHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(PubComp::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::PubComp)
                        .parse_next(input)
                }
                ControlPacketType::Subscribe => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        SubscribeHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(Subscribe::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::Subscribe)
                        .parse_next(input)
                }
                ControlPacketType::SubAck => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        SubAckHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(SubAck::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::SubAck)
                        .parse_next(input)
                }
                ControlPacketType::Unsubscribe => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        UnsubscribeHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(Unsubscribe::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::Unsubscribe)
                        .parse_next(input)
                }
                ControlPacketType::UnsubAck => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        UnsubAckHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(UnsubAck::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::UnsubAck)
                        .parse_next(input)
                }
                ControlPacketType::PingReq => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PingReqHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(PingReq::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::PingReq)
                        .parse_next(input)
                }
                ControlPacketType::PingResp => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        PingRespHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(PingResp::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::PingResp)
                        .parse_next(input)
                }
                ControlPacketType::Disconnect => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        DisconnectHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(Disconnect::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::Disconnect)
                        .parse_next(input)
                }
                ControlPacketType::Auth => {
                    let (_, _header_flags) = bits::bits((
                        bits::take::<_, u8, _, BitError>(4usize),
                        AuthHeaderFlags::parse,
                    ))
                    .parse_next(input)?;
                    remaining_parser
                        .and_then(Auth::parse::<_, _, BitError>(parser_settings))
                        .map(ControlPacket::Auth)
                        .parse_next(input)
                }
            }
        })
    }
}
