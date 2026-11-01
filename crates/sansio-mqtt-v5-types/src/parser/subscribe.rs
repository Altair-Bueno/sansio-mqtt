use super::BytesSource;
use super::ParserSettings;
use super::basic::two_byte_integer;
use super::basic::variable_byte_integer;
use super::properties::push_capped;
use super::properties::set_once;
use crate::BinaryDataError;
use crate::InvalidPropertyTypeError;
use crate::InvalidQosError;
use crate::InvalidRetainHandlingError;
use crate::PropertiesError;
use crate::Property;
use crate::PropertyType;
use crate::Subscribe;
use crate::SubscribeHeaderFlags;
use crate::SubscribeProperties;
use crate::Subscription;
use crate::TooManyUserPropertiesError;
use crate::TopicError;
use crate::UnknownFormatIndicatorError;
use crate::UnsupportedPropertyError;
use crate::Utf8StringError;
use alloc::vec::Vec;
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

impl SubscribeHeaderFlags {
    /// Parses the 4-bit Fixed Header flags for `SUBSCRIBE`
    /// ([§3.8.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901162),
    /// [MQTT-3.8.1-1]). The bit pattern `0b0010` is required.
    ///
    /// # Errors
    ///
    /// Fails when fewer than four bits remain or the flags are not the reserved
    /// value `0b0010` required for `SUBSCRIBE`; the caller treats that as a
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
            "SUBSCRIBE Header Flags",
        )))
        .parse_next(input)
    }
}

impl Subscribe {
    /// Returns a parser for the body of a `SUBSCRIBE` packet
    /// ([§3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901161)).
    ///
    /// The Payload MUST contain at least one Topic Filter / Subscription
    /// Options pair ([MQTT-3.8.3-2]).
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
            + FromExternalError<ByteInput, Utf8StringError>
            + FromExternalError<ByteInput, TopicError>
            + FromExternalError<ByteInput, TryFromIntError>
            + FromExternalError<ByteInput, BinaryDataError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<Bits<ByteInput>>
            + ErrorConvert<ByteError>
            + FromExternalError<Bits<ByteInput>, InvalidRetainHandlingError>
            + FromExternalError<Bits<ByteInput>, InvalidQosError>
            + AddContext<Bits<ByteInput>, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                combinator::trace("Packet ID", two_byte_integer.try_map(TryInto::try_into)),
                SubscribeProperties::parser(parser_settings),
                combinator::trace(
                    "subscriptions",
                    combinator::repeat_till(
                        1..=usize::try_from(parser_settings.max_subscriptions_len)
                            .unwrap_or(usize::MAX),
                        Subscription::parser::<_, _, BitError>(parser_settings),
                        combinator::eof,
                    ),
                ),
            )
                .verify_map(move |(packet_id, properties, (subscriptions, _eof))| {
                    // `repeat_till(1..=..)` yields at least one subscription,
                    // so `next()` always succeeds [MQTT-3.8.3-2].
                    let mut subscriptions = Vec::<Subscription>::into_iter(subscriptions);
                    let subscription = subscriptions.next()?;

                    Some(Self {
                        packet_id,
                        subscription,
                        extra_subscriptions: subscriptions.collect(),
                        properties,
                    })
                }),
        )
    }
}

impl SubscribeProperties {
    /// Returns a parser for the `SUBSCRIBE` properties section
    /// ([§3.8.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901164)).
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
            + FromExternalError<Input, TryFromIntError>
            + FromExternalError<Input, BinaryDataError>,
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
                                Property::SubscriptionIdentifier(value) => set_once(
                                    &mut properties.subscription_identifier,
                                    value,
                                    property_type,
                                )?,
                                Property::UserProperty(key, value) => push_capped(
                                    &mut properties.user_properties,
                                    (key, value),
                                    parser_settings.max_user_properties_len,
                                    PropertiesError::from(TooManyUserPropertiesError),
                                )?,
                                Property::PayloadFormatIndicator(_)
                                | Property::MessageExpiryInterval(_)
                                | Property::ContentType(_)
                                | Property::ResponseTopic(_)
                                | Property::CorrelationData(_)
                                | Property::SessionExpiryInterval(_)
                                | Property::AssignedClientIdentifier(_)
                                | Property::ServerKeepAlive(_)
                                | Property::AuthenticationMethod(_)
                                | Property::AuthenticationData(_)
                                | Property::RequestProblemInformation(_)
                                | Property::WillDelayInterval(_)
                                | Property::RequestResponseInformation(_)
                                | Property::ResponseInformation(_)
                                | Property::ServerReference(_)
                                | Property::ReasonString(_)
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
