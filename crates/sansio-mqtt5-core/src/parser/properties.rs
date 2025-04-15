use super::*;

impl PropertyType {
    #[inline]
    pub fn parse<Input, Error>(input: &mut Input) -> Result<Self, Error>
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

impl<'input> Property<'input> {
    #[inline]
    pub fn parse<'settings, Input, Error>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]> + StreamIsPartial + Clone,
        Error: ParserError<Input>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + FromExternalError<Input, MQTTStringError>
            + FromExternalError<Input, PublishTopicError>
            + FromExternalError<Input, TryFromIntError>
            + AddContext<Input, StrContext>,
    {
        combinator::trace(type_name::<Self>(), move |input: &mut Input| {
            let property_type = PropertyType::parse.parse_next(input)?;
            match property_type {
                PropertyType::PayloadFormatIndicator => combinator::trace(
                    "PayloadFormatIndicator",
                    FormatIndicator::parse.map(Property::PayloadFormatIndicator),
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
                    MQTTString::parse(parser_settings).map(Property::ContentType),
                )
                .context(StrContext::Label("ContentType"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Content Type value",
                )))
                .parse_next(input),
                PropertyType::ResponseTopic => combinator::trace(
                    "ResponseTopic",
                    PublishTopic::parse(parser_settings).map(Property::ResponseTopic),
                )
                .context(StrContext::Label("ResponseTopic"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Response Topic value",
                )))
                .parse_next(input),
                PropertyType::CorrelationData => combinator::trace(
                    "CorrelationData",
                    binary_data(parser_settings)
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
                    "assignedClientIdentifier",
                    MQTTString::parse(parser_settings).map(Property::AssignedClientIdentifier),
                )
                .context(StrContext::Label("assignedClientIdentifier"))
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
                    "authenticationMethod",
                    MQTTString::parse(parser_settings).map(Property::AuthenticationMethod),
                )
                .context(StrContext::Label("authenticationMethod"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "an Authentication Method value",
                )))
                .parse_next(input),
                PropertyType::AuthenticationData => combinator::trace(
                    "authenticationData",
                    binary_data(parser_settings)
                        .output_into()
                        .map(Property::AuthenticationData),
                )
                .context(StrContext::Label("authenticationData"))
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
                    MQTTString::parse(parser_settings).map(Property::ResponseInformation),
                )
                .context(StrContext::Label("ResponseInformation"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Response Information value",
                )))
                .parse_next(input),
                PropertyType::ServerReference => combinator::trace(
                    "ServerReference",
                    MQTTString::parse(parser_settings).map(Property::ServerReference),
                )
                .context(StrContext::Label("ServerReference"))
                .context(StrContext::Expected(StrContextValue::Description(
                    "a Server Reference value",
                )))
                .parse_next(input),
                PropertyType::ReasonString => combinator::trace(
                    "ReasonString",
                    MQTTString::parse(parser_settings).map(Property::ReasonString),
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
