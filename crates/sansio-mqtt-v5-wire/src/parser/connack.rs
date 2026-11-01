#[inline]
pub(super) fn flags<Input, BitError, ByteError>(input: &mut Input) -> Result<(bool,), ByteError>
where
    BitError: ParserError<Bits<Input>> + ErrorConvert<ByteError>,
    ByteError: ParserError<Input>,
    Bits<Input>: Stream,
    Input: Stream<Token = u8> + StreamIsPartial + Clone,
{
    let (_, session_present) =
        bits::bits::<_, _, BitError, _, _>((bits::pattern(0, 7usize), bits::bool))
            .parse_next(input)?;
    Ok((session_present,))
}

use super::BytesSource;
use super::ParserSettings;
use super::basic::variable_byte_integer;
use super::properties::push_capped;
use super::properties::set_once;
use crate::AuthenticationKind;
use crate::BinaryDataError;
use crate::ConnAck;
use crate::ConnAckHeaderFlags;
use crate::ConnAckKind;
use crate::ConnAckProperties;
use crate::ConnackReasonCode;
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

impl ConnAck {
    /// Returns a parser for the body of a `CONNACK` packet
    /// ([§3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901074)).
    #[inline]
    #[must_use]
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
                flags::<_, BitError, _>,
                ConnackReasonCode::parser,
                ConnAckProperties::parser(parser_settings),
                combinator::eof,
            )
                .verify_map(move |((session_present,), reason_code, properties, _)| {
                    // If a Server sends a CONNACK packet containing a non-zero
                    // Reason Code it MUST set Session
                    // Present to 0 [MQTT-3.2.2-6].
                    let kind = match (session_present, reason_code) {
                        (true, ConnackReasonCode::Success) => ConnAckKind::ResumePreviousSession,
                        (false, reason_code) => ConnAckKind::Other { reason_code },
                        (true, _) => return None,
                    };
                    Some(Self { kind, properties })
                }),
        )
    }
}

impl ConnAckHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `CONNACK`
    /// ([§3.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901075)).
    ///
    /// # Errors
    ///
    /// Fails when fewer than four bits remain or the flags are not the reserved
    /// value `0b0000` required for `CONNACK`; the caller treats that as a
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
                "CONNACK Header Flags",
            )))
            .parse_next(input)
    }
}

impl ConnAckProperties {
    /// Returns a parser for the `CONNACK` properties section
    /// ([§3.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901080)).
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
            + FromExternalError<Input, TryFromIntError>
            + FromExternalError<Input, TopicError>
            + FromExternalError<Input, BinaryDataError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(
                variable_byte_integer,
                (
                    combinator::repeat(.., Property::parser(parser_settings))
                        .try_fold(
                            Default::default,
                            |(
                                mut properties,
                                mut authentication_data,
                                mut authentication_method,
                            ): (Self, Option<_>, Option<_>),
                             property| {
                                let property_type = PropertyType::from(&property);
                                match property {
                                    Property::SessionExpiryInterval(value) => set_once(&mut properties.session_expiry_interval, value, property_type)?,
                                    Property::ReceiveMaximum(value) => set_once(&mut properties.receive_maximum, value, property_type)?,
                                    Property::MaximumQoS(value) => set_once(&mut properties.maximum_qos, value, property_type)?,
                                    Property::RetainAvailable(value) => set_once(&mut properties.retain_available, value, property_type)?,
                                    Property::MaximumPacketSize(value) => set_once(&mut properties.maximum_packet_size, value, property_type)?,
                                    Property::AssignedClientIdentifier(value) => set_once(&mut properties.assigned_client_identifier, value, property_type)?,
                                    Property::TopicAliasMaximum(value) => set_once(&mut properties.topic_alias_maximum, value, property_type)?,
                                    Property::ReasonString(value) => set_once(&mut properties.reason_string, value, property_type)?,
                                    Property::UserProperty(key, value) => push_capped(&mut properties.user_properties, (key, value), parser_settings.max_user_properties_len, PropertiesError::from(TooManyUserPropertiesError))?,
                                    Property::WildcardSubscriptionAvailable(value) => set_once(&mut properties.wildcard_subscription_available, value, property_type)?,
                                    Property::SubscriptionIdentifiersAvailable(value) => set_once(&mut properties.subscription_identifiers_available, value, property_type)?,
                                    Property::SharedSubscriptionAvailable(value) => set_once(&mut properties.shared_subscription_available, value, property_type)?,
                                    Property::ServerKeepAlive(value) => set_once(&mut properties.server_keep_alive, value, property_type)?,
                                    Property::ResponseInformation(value) => set_once(&mut properties.response_information, value, property_type)?,
                                    Property::ServerReference(value) => set_once(&mut properties.server_reference, value, property_type)?,
                                    Property::AuthenticationMethod(value) => set_once(&mut authentication_method, value, property_type)?,
                                    Property::AuthenticationData(value) => set_once(&mut authentication_data, value, property_type)?,
                                    Property::PayloadFormatIndicator(_)
                                    | Property::MessageExpiryInterval(_)
                                    | Property::ContentType(_)
                                    | Property::ResponseTopic(_)
                                    | Property::CorrelationData(_)
                                    | Property::SubscriptionIdentifier(_)
                                    | Property::RequestProblemInformation(_)
                                    | Property::WillDelayInterval(_)
                                    | Property::RequestResponseInformation(_)
                                    | Property::TopicAlias(_) => {
                                        return Err(PropertiesError::from(
                                            UnsupportedPropertyError { property_type },
                                        ))
                                    }
                                }
                                Ok((properties, authentication_data, authentication_method))
                            },
                        )
                        .try_map(
                            |(mut properties, authentication_data, authentication_method)| -> Result<_, PropertiesError> {
                                // It is a Protocol Error to include Authentication Data if there is no Authentication Method
                                properties.authentication = AuthenticationKind::try_from_parts((
                                    authentication_method,
                                    authentication_data,
                                ))?;
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
