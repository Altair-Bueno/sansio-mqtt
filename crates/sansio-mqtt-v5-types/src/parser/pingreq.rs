use super::BytesSource;
use super::ParserSettings;
use crate::PingReq;
use crate::PingReqHeaderFlags;
use core::any::type_name;
use winnow::Parser;
use winnow::binary::bits;
use winnow::binary::bits::Bits;
use winnow::combinator;
use winnow::error::AddContext;
use winnow::error::ErrorConvert;
use winnow::error::ParserError;
use winnow::error::Result;
use winnow::error::StrContext;
use winnow::error::StrContextValue;
use winnow::stream::Stream;
use winnow::stream::StreamIsPartial;
impl PingReqHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `PINGREQ`
    /// ([§3.12.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901196),
    /// [MQTT-3.12.1-1]).
    ///
    /// # Errors
    ///
    /// Fails when fewer than four bits remain or the flags are not the reserved
    /// value `0b0000` required for `PINGREQ`; the caller treats that as a
    /// Malformed Packet.
    #[inline]
    pub fn parser<Input, Error>(input: &mut Bits<Input>) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<Bits<Input>> + AddContext<Bits<Input>, StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "PINGREQ Header Flags",
            )))
            .parse_next(input)
    }
}

impl PingReq {
    /// Returns a parser for the body of a `PINGREQ` packet
    /// ([§3.12](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901195)).
    ///
    /// The packet has no Variable Header or Payload, so the parser
    /// only asserts that no bytes remain.
    #[inline]
    pub fn parser<'input, 'settings, ByteInput, ByteError, BitError>(
        _parser_settings: &'settings ParserSettings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + BytesSource + Clone,
        ByteError: ParserError<ByteInput>,
        BitError: ParserError<Bits<ByteInput>> + ErrorConvert<ByteError>,
    {
        combinator::trace(
            type_name::<Self>(),
            // The remaining length of the PINGREQ packet is always 0
            combinator::eof.value(Self {}),
        )
    }
}
