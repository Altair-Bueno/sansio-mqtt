use super::BytesSource;
use super::ParserSettings;
use super::basic::two_byte_integer;
use crate::BinaryDataError;
use crate::InvalidPropertyTypeError;
use crate::InvalidQosError;
use crate::InvalidReasonCode;
use crate::PropertiesError;
use crate::PubRel;
use crate::PubRelHeaderFlags;
use crate::PubRelProperties;
use crate::PubRelReasonCode;
use crate::TopicError;
use crate::UnknownFormatIndicatorError;
use crate::Utf8StringError;
use core::any::type_name;
use core::num::TryFromIntError;
use core::str::Utf8Error;
use winnow::Parser;
use winnow::binary::bits;
use winnow::binary::bits::Bits;
use winnow::combinator;
use winnow::error::AddContext;
use winnow::error::ErrorConvert;
use winnow::error::FromExternalError;
use winnow::error::ParserError;
use winnow::error::Result;
use winnow::error::StrContext;
use winnow::error::StrContextValue;
use winnow::stream::Stream;
use winnow::stream::StreamIsPartial;
use winnow::stream::UpdateSlice;
impl PubRelHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `PUBREL`
    /// ([§3.6.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901142),
    /// [MQTT-3.6.1-1]). The bit pattern `0b0010` is required.
    ///
    /// # Errors
    ///
    /// Fails when fewer than four bits remain or the flags are not the reserved
    /// value `0b0010` required for `PUBREL`; the caller treats that as a
    /// Malformed Packet.
    #[inline]
    pub fn parser<Input, Error>(input: &mut Bits<Input>) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<Bits<Input>> + AddContext<Bits<Input>, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            bits::pattern(0b0000_0010, 4usize).value(Self),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "PUBREL Header Flags",
        )))
        .parse_next(input)
    }
}

impl PubRel {
    /// Returns a parser for the body of a `PUBREL` packet
    /// ([§3.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901141)).
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
        BitError: ParserError<Bits<ByteInput>> + ErrorConvert<ByteError>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                combinator::trace("Packet ID", two_byte_integer.try_map(TryInto::try_into)),
                // The Reason Code and Property Length can be omitted if the
                // Reason Code is 0x00 (Success) and there are no Properties. In
                // this case the PUBREC has a Remaining Length of 2.
                combinator::alt((
                    (
                        combinator::empty.default_value(),
                        combinator::empty.default_value(),
                        combinator::eof,
                    ),
                    (
                        PubRelReasonCode::parser,
                        PubRelProperties::parser(parser_settings),
                        combinator::eof,
                    ),
                )),
            )
                .map(move |(packet_id, (reason_code, properties, _))| Self {
                    packet_id,
                    reason_code,
                    properties,
                }),
        )
    }
}
