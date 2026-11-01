use super::BytesSource;
use super::ParserSettings;
use super::basic::variable_byte_integer;
use super::properties::push_capped;
use super::properties::set_once;
use crate::BinaryDataError;
use crate::Disconnect;
use crate::DisconnectHeaderFlags;
use crate::DisconnectProperties;
use crate::DisconnectReasonCode;
use crate::InvalidPropertyTypeError;
use crate::InvalidQosError;
use crate::InvalidReasonCode;
use crate::PropertiesError;
use crate::Property;
use crate::PropertyType;
use crate::TooManyUserPropertiesError;
use crate::TopicError;
use crate::UnknownFormatIndicatorError;
use crate::UnsupportedPropertyError;
use crate::Utf8StringError;
use core::any::type_name;
use core::num::TryFromIntError;
use core::str::Utf8Error;
use winnow::Parser;
use winnow::binary;
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
impl DisconnectHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `DISCONNECT`
    /// ([§3.14.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901206),
    /// [MQTT-3.14.1-1]).
    ///
    /// # Errors
    ///
    /// Fails when fewer than four bits remain or the flags are not the reserved
    /// value `0b0000` required for `DISCONNECT`; the caller treats that as a
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
                "DISCONNECT Header Flags",
            )))
            .parse_next(input)
    }
}

impl Disconnect {
    /// Returns a parser for the body of a `DISCONNECT` packet
    /// ([§3.14](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901205)).
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
            // The Reason Code and Property Length can be omitted if the Reason
            // Code is 0x00 (Normal disconnection) and there are no Properties.
            // In this case the DISCONNECT has a Remaining Length of 0
            combinator::alt((
                (
                    combinator::empty.default_value(),
                    combinator::empty.default_value(),
                    combinator::eof,
                ),
                (
                    DisconnectReasonCode::parser,
                    DisconnectProperties::parser(parser_settings),
                    combinator::eof,
                ),
            ))
            .map(move |(reason_code, properties, _)| Self {
                reason_code,
                properties,
            }),
        )
    }
}

impl DisconnectProperties {
    /// Returns a parser for the `DISCONNECT` properties section
    /// ([§3.14.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901209)).
    #[inline]
    pub fn parser<'input, 'settings, Input, Error>(
        parser_settings: &'settings ParserSettings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]>
            + BytesSource
            + UpdateSlice
            + StreamIsPartial
            + Clone,
        Error: ParserError<Input>
            + AddContext<Input, StrContext>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, PropertiesError>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + FromExternalError<Input, Utf8StringError>
            + FromExternalError<Input, TopicError>
            + FromExternalError<Input, BinaryDataError>
            + FromExternalError<Input, TryFromIntError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(
                variable_byte_integer,
                (
                    combinator::repeat(.., Property::parser(parser_settings)).try_fold(
                        Self::default,
                        |mut properties, property| {
                            let property_type = PropertyType::from(&property);
                            match property {
                                Property::SessionExpiryInterval(value) => set_once(
                                    &mut properties.session_expiry_interval,
                                    value,
                                    property_type,
                                )?,
                                Property::ReasonString(value) => {
                                    set_once(&mut properties.reason_string, value, property_type)?;
                                }
                                Property::UserProperty(key, value) => push_capped(
                                    &mut properties.user_properties,
                                    (key, value),
                                    parser_settings.max_user_properties_len,
                                    PropertiesError::from(TooManyUserPropertiesError),
                                )?,
                                Property::ServerReference(value) => set_once(
                                    &mut properties.server_reference,
                                    value,
                                    property_type,
                                )?,
                                Property::PayloadFormatIndicator(_)
                                | Property::MessageExpiryInterval(_)
                                | Property::ContentType(_)
                                | Property::ResponseTopic(_)
                                | Property::CorrelationData(_)
                                | Property::SubscriptionIdentifier(_)
                                | Property::AssignedClientIdentifier(_)
                                | Property::ServerKeepAlive(_)
                                | Property::AuthenticationMethod(_)
                                | Property::AuthenticationData(_)
                                | Property::RequestProblemInformation(_)
                                | Property::WillDelayInterval(_)
                                | Property::RequestResponseInformation(_)
                                | Property::ResponseInformation(_)
                                | Property::ReceiveMaximum(_)
                                | Property::TopicAliasMaximum(_)
                                | Property::TopicAlias(_)
                                | Property::MaximumQoS(_)
                                | Property::RetainAvailable(_)
                                | Property::MaximumPacketSize(_)
                                | Property::WildcardSubscriptionAvailable(_)
                                | Property::SubscriptionIdentifiersAvailable(_)
                                | Property::SharedSubscriptionAvailable(_) => {
                                    return Err(PropertiesError::from(UnsupportedPropertyError {
                                        property_type,
                                    }));
                                }
                            }
                            Ok(properties)
                        },
                    ),
                    combinator::eof,
                )
                    .map(|(properties, _)| properties),
            ),
        )
        .context(StrContext::Label(type_name::<Self>()))
    }
}
