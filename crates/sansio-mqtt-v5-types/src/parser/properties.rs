use super::*;

/// Stores `value` in `slot`, rejecting a property that is only allowed
/// to appear once but was seen again
/// ([§2.2.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901028),
/// [MQTT-2.2.2-2]).
#[inline]
pub(crate) fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    property_type: PropertyType,
) -> Result<(), PropertiesError> {
    match slot {
        None => {
            *slot = Some(value);
            Ok(())
        }
        Some(_) => Err(DuplicatedPropertyError { property_type }.into()),
    }
}

/// Appends `value` to `dst` unless that would exceed `max_len`,
/// guarding against resource-exhaustion from a repeated property.
#[inline]
pub(crate) fn push_capped<T>(
    dst: &mut Vec<T>,
    value: T,
    max_len: usize,
    on_overflow: PropertiesError,
) -> Result<(), PropertiesError> {
    if dst.len() >= max_len {
        return Err(on_overflow);
    }
    dst.push(value);
    Ok(())
}

impl PropertyType {
    /// Parses a property identifier (Variable Byte Integer)
    /// ([§2.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901029),
    /// [MQTT-2.2.2-1]).
    #[inline]
    pub fn parser<Input, Error>(input: &mut Input) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial,
        Error: ParserError<Input>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + AddContext<Input, StrContext>,
    {
        combinator::trace(
            type_name::<Self>(),
            self::variable_byte_integer.try_map(TryInto::try_into),
        )
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Property Type",
        )))
        .parse_next(input)
    }
}

impl Property {
    /// Returns a parser for a single MQTT v5.0 [`Property`]
    /// ([§2.2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901029)).
    #[inline]
    pub fn parser<'input, 'settings, Input, Error>(
        parser_settings: &'settings ParserSettings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]> + BytesSource + StreamIsPartial + Clone,
        Error: ParserError<Input>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + FromExternalError<Input, Utf8StringError>
            + FromExternalError<Input, TopicError>
            + FromExternalError<Input, TryFromIntError>
            + FromExternalError<Input, BinaryDataError>
            + AddContext<Input, StrContext>,
    {
        combinator::trace(type_name::<Self>(), move |input: &mut Input| {
            let property_type = PropertyType::parser.parse_next(input)?;
            match property_type {
                PropertyType::PayloadFormatIndicator => combinator::trace(
                    "PayloadFormatIndicator",
                    FormatIndicator::parser.map(Property::PayloadFormatIndicator),
                )
                .context(StrContext::Label("PayloadFormatIndicator"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Payload Format Indicator value",
                )))
                .parse_next(input),
                PropertyType::MessageExpiryInterval => combinator::trace(
                    "MessageExpiryInterval",
                    four_byte_integer.map(Property::MessageExpiryInterval),
                )
                .context(StrContext::Label("MessageExpiryInterval"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Message Expiry Interval value",
                )))
                .parse_next(input),
                PropertyType::ContentType => combinator::trace(
                    "ContentType",
                    Utf8String::parser(parser_settings).map(Property::ContentType),
                )
                .context(StrContext::Label("ContentType"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Content Type value",
                )))
                .parse_next(input),
                PropertyType::ResponseTopic => combinator::trace(
                    "ResponseTopic",
                    Topic::parser(parser_settings).map(Property::ResponseTopic),
                )
                .context(StrContext::Label("ResponseTopic"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Response Topic value",
                )))
                .parse_next(input),
                PropertyType::CorrelationData => combinator::trace(
                    "CorrelationData",
                    BinaryData::parser(parser_settings)
                        .output_into()
                        .map(Property::CorrelationData),
                )
                .context(StrContext::Label("CorrelationData"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Correlation Data value",
                )))
                .parse_next(input),
                PropertyType::SubscriptionIdentifier => combinator::trace(
                    "SubscriptionIdentifier",
                    self::variable_byte_integer
                        .try_map(TryInto::try_into)
                        .map(Property::SubscriptionIdentifier),
                )
                .context(StrContext::Label("SubscriptionIdentifier"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Subscription Identifier value",
                )))
                .parse_next(input),
                PropertyType::SessionExpiryInterval => combinator::trace(
                    "SessionExpiryInterval",
                    four_byte_integer.map(Property::SessionExpiryInterval),
                )
                .context(StrContext::Label("SessionExpiryInterval"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Session Expiry Interval value",
                )))
                .parse_next(input),
                PropertyType::AssignedClientIdentifier => combinator::trace(
                    "AssignedClientIdentifier",
                    Utf8String::parser(parser_settings).map(Property::AssignedClientIdentifier),
                )
                .context(StrContext::Label("AssignedClientIdentifier"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "an Assigned Client Identifier value",
                )))
                .parse_next(input),
                PropertyType::ServerKeepAlive => combinator::trace(
                    "ServerKeepAlive",
                    two_byte_integer.map(Property::ServerKeepAlive),
                )
                .context(StrContext::Label("ServerKeepAlive"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Server Keep Alive value",
                )))
                .parse_next(input),
                PropertyType::AuthenticationMethod => combinator::trace(
                    "AuthenticationMethod",
                    Utf8String::parser(parser_settings).map(Property::AuthenticationMethod),
                )
                .context(StrContext::Label("AuthenticationMethod"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "an Authentication Method value",
                )))
                .parse_next(input),
                PropertyType::AuthenticationData => combinator::trace(
                    "AuthenticationData",
                    BinaryData::parser(parser_settings)
                        .output_into()
                        .map(Property::AuthenticationData),
                )
                .context(StrContext::Label("AuthenticationData"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "an Authentication Data value",
                )))
                .parse_next(input),
                PropertyType::RequestProblemInformation => combinator::trace(
                    "RequestProblemInformation",
                    token::any
                        .map(|x| x != 0)
                        .map(Property::RequestProblemInformation),
                )
                .context(StrContext::Label("RequestProblemInformation"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Request Problem Information value",
                )))
                .parse_next(input),
                PropertyType::WillDelayInterval => combinator::trace(
                    "WillDelayInterval",
                    four_byte_integer.map(Property::WillDelayInterval),
                )
                .context(StrContext::Label("WillDelayInterval"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Will Delay Interval value",
                )))
                .parse_next(input),
                PropertyType::RequestResponseInformation => combinator::trace(
                    "RequestResponseInformation",
                    token::any
                        .map(|x| x != 0)
                        .map(Property::RequestResponseInformation),
                )
                .context(StrContext::Label("RequestResponseInformation"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Request Response Information value",
                )))
                .parse_next(input),
                PropertyType::ResponseInformation => combinator::trace(
                    "ResponseInformation",
                    Utf8String::parser(parser_settings).map(Property::ResponseInformation),
                )
                .context(StrContext::Label("ResponseInformation"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Response Information value",
                )))
                .parse_next(input),
                PropertyType::ServerReference => combinator::trace(
                    "ServerReference",
                    Utf8String::parser(parser_settings).map(Property::ServerReference),
                )
                .context(StrContext::Label("ServerReference"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Server Reference value",
                )))
                .parse_next(input),
                PropertyType::ReasonString => combinator::trace(
                    "ReasonString",
                    Utf8String::parser(parser_settings).map(Property::ReasonString),
                )
                .context(StrContext::Label("ReasonString"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Reason String value",
                )))
                .parse_next(input),
                PropertyType::ReceiveMaximum => combinator::trace(
                    "ReceiveMaximum",
                    two_byte_integer
                        .try_map(TryInto::try_into)
                        .map(Property::ReceiveMaximum),
                )
                .context(StrContext::Label("ReceiveMaximum"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Receive Maximum value",
                )))
                .parse_next(input),
                PropertyType::TopicAliasMaximum => combinator::trace(
                    "TopicAliasMaximum",
                    two_byte_integer.map(Property::TopicAliasMaximum),
                )
                .context(StrContext::Label("TopicAliasMaximum"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Topic Alias Maximum value",
                )))
                .parse_next(input),
                PropertyType::TopicAlias => combinator::trace(
                    "TopicAlias",
                    two_byte_integer
                        .try_map(TryInto::try_into)
                        .map(Property::TopicAlias),
                )
                .context(StrContext::Label("TopicAlias"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Topic Alias value",
                )))
                .parse_next(input),
                PropertyType::MaximumQoS => combinator::trace(
                    "MaximumQoS",
                    token::any
                        .try_map(TryInto::try_into)
                        .map(Property::MaximumQoS),
                )
                .context(StrContext::Label("MaximumQoS"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Maximum QoS value",
                )))
                .parse_next(input),
                PropertyType::RetainAvailable => combinator::trace(
                    "RetainAvailable",
                    token::any.map(|x| x != 0).map(Property::RetainAvailable),
                )
                .context(StrContext::Label("RetainAvailable"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Retain Available value",
                )))
                .parse_next(input),
                PropertyType::UserProperty => combinator::trace(
                    "UserProperty",
                    self::string_pair(parser_settings)
                        .map(|(key, value)| Property::UserProperty(key, value)),
                )
                .context(StrContext::Label("UserProperty"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a User Property entry",
                )))
                .parse_next(input),
                PropertyType::MaximumPacketSize => combinator::trace(
                    "MaximumPacketSize",
                    four_byte_integer
                        .try_map(TryInto::try_into)
                        .map(Property::MaximumPacketSize),
                )
                .context(StrContext::Label("MaximumPacketSize"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Maximum Packet Size value",
                )))
                .parse_next(input),
                PropertyType::WildcardSubscriptionAvailable => combinator::trace(
                    "WildcardSubscriptionAvailable",
                    token::any
                        .map(|x| x != 0)
                        .map(Property::WildcardSubscriptionAvailable),
                )
                .context(StrContext::Label("WildcardSubscriptionAvailable"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Wildcard Subscription Available value",
                )))
                .parse_next(input),
                PropertyType::SubscriptionIdentifiersAvailable => combinator::trace(
                    "SubscriptionIdentifiersAvailable",
                    token::any
                        .map(|x| x != 0)
                        .map(Property::SubscriptionIdentifiersAvailable),
                )
                .context(StrContext::Label("SubscriptionIdentifiersAvailable"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Subscription Identifiers Available value",
                )))
                .parse_next(input),
                PropertyType::SharedSubscriptionAvailable => combinator::trace(
                    "SharedSubscriptionAvailable",
                    token::any
                        .map(|x| x != 0)
                        .map(Property::SharedSubscriptionAvailable),
                )
                .context(StrContext::Label("SharedSubscriptionAvailable"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Shared Subscription Available value",
                )))
                .parse_next(input),
            }
        })
        .context(StrContext::Label(type_name::<Self>()))
        .context(StrContext::Expected(StrContextValue::Description(
            "a Property entry",
        )))
    }
}

/// Generates the properties-section parser shared by the acknowledgement
/// packets whose only permitted properties are Reason String and User
/// Property ([§2.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901027)).
macro_rules! impl_ack_properties_parser {
    ($name:ty, $doc:expr) => {
        impl $name {
            #[doc = $doc]
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
                                        Property::ReasonString(value) => set_once(
                                            &mut properties.reason_string,
                                            value,
                                            property_type,
                                        )?,
                                        Property::UserProperty(key, value) => push_capped(
                                            &mut properties.user_properties,
                                            (key, value),
                                            parser_settings.max_user_properties_len,
                                            PropertiesError::from(TooManyUserPropertiesError),
                                        )?,
                                        _ => {
                                            return Err(PropertiesError::from(
                                                UnsupportedPropertyError { property_type },
                                            ));
                                        }
                                    };
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
    };
}

impl_ack_properties_parser!(
    PubAckProperties,
    "Returns a parser for the `PUBACK` properties section\n([§3.4.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901125))."
);
impl_ack_properties_parser!(
    PubRecProperties,
    "Returns a parser for the `PUBREC` properties section\n([§3.5.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901135))."
);
impl_ack_properties_parser!(
    PubRelProperties,
    "Returns a parser for the `PUBREL` properties section\n([§3.6.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901145))."
);
impl_ack_properties_parser!(
    PubCompProperties,
    "Returns a parser for the `PUBCOMP` properties section\n([§3.7.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901155))."
);
impl_ack_properties_parser!(
    SubAckProperties,
    "Returns a parser for the `SUBACK` properties section\n([§3.9.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901174))."
);
impl_ack_properties_parser!(
    UnsubAckProperties,
    "Returns a parser for the `UNSUBACK` properties section\n([§3.11.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901189))."
);
