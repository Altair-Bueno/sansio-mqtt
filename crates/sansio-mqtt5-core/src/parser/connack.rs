#[inline]
pub fn flags<Input, BitError, ByteError>(input: &mut Input) -> Result<(bool,), ByteError>
where
    BitError: ParserError<(Input, usize)> + ErrorConvert<ByteError>,
    ByteError: ParserError<Input>,
    (Input, usize): Stream,
    Input: Stream<Token = u8> + StreamIsPartial + Clone,
{
    let (_, session_present) =
        bits::bits::<_, _, BitError, _, _>((bits::pattern(0, 7usize), bits::bool))
            .parse_next(input)?;
    Ok((session_present,))
}

use super::*;

impl<'input> ConnAck<'input> {
    #[inline]
    pub fn parse<'settings, ByteInput, ByteError, BitError>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<ByteInput, Self, ByteError> + use<'input, 'settings, ByteInput, ByteError, BitError>
    where
        ByteInput: StreamIsPartial + Stream<Token = u8, Slice = &'input [u8]> + Clone + UpdateSlice,
        ByteError: ParserError<ByteInput>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, Utf8Error>
            + FromExternalError<ByteInput, InvalidQosError>
            + FromExternalError<ByteInput, InvalidPropertyTypeError>
            + FromExternalError<ByteInput, PropertiesError>
            + FromExternalError<ByteInput, UnknownFormatIndicatorError>
            + FromExternalError<ByteInput, InvalidReasonCode>
            + FromExternalError<ByteInput, MQTTStringError>
            + FromExternalError<ByteInput, PublishTopicError>
            + FromExternalError<ByteInput, TryFromIntError>
            + AddContext<ByteInput, StrContext>,
        BitError: ParserError<(ByteInput, usize)> + ErrorConvert<ByteError>,
    {
        combinator::trace(
            type_name::<Self>(),
            (
                flags::<_, BitError, _>,
                ConnackReasonCode::parse,
                ConnAckProperties::parse(parser_settings),
                combinator::eof,
            )
                .verify_map(move |((session_present,), reason_code, properties, _)| {
                    // If a Server sends a CONNACK packet containing a non-zero Reason Code it MUST set Session Present to 0 [MQTT-3.2.2-6].
                    let kind = match (session_present, reason_code) {
                        (true, ConnackReasonCode::Success) => ConnAckKind::ResumePreviousSession,
                        (false, reason_code) => ConnAckKind::Other { reason_code },
                        (true, _) => return None,
                    };
                    Some(ConnAck { kind, properties })
                }),
        )
    }
}

impl ConnAckHeaderFlags {
    #[inline]
    pub fn parse<Input, Error>(input: &mut (Input, usize)) -> Result<Self, Error>
    where
        Input: Stream<Token = u8> + StreamIsPartial + Clone,
        Error: ParserError<(Input, usize)> + AddContext<(Input, usize), StrContext>,
    {
        combinator::trace(type_name::<Self>(), bits::pattern(0u8, 4usize).value(Self))
            .context(StrContext::Label(type_name::<Self>()))
            .context(StrContext::Expected(StrContextValue::Description(
                "CONNACK Header Flags",
            )))
            .parse_next(input)
    }
}

impl<'input> ConnAckProperties<'input> {
    #[inline]
    pub fn parse<'settings, Input, Error>(
        parser_settings: &'settings Settings,
    ) -> impl Parser<Input, Self, Error> + use<'input, 'settings, Input, Error>
    where
        Input: Stream<Token = u8, Slice = &'input [u8]> + UpdateSlice + StreamIsPartial + Clone,
        Error: ParserError<Input>
            + AddContext<Input, StrContext>
            + FromExternalError<Input, Utf8Error>
            + FromExternalError<Input, InvalidQosError>
            + FromExternalError<Input, InvalidPropertyTypeError>
            + FromExternalError<Input, PropertiesError>
            + FromExternalError<Input, UnknownFormatIndicatorError>
            + FromExternalError<Input, MQTTStringError>
            + FromExternalError<Input, TryFromIntError>
            + FromExternalError<Input, PublishTopicError>,
    {
        combinator::trace(
            type_name::<Self>(),
            binary::length_and_then(
                variable_byte_integer,
                (
                    combinator::repeat(.., Property::parse(parser_settings))
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
                                    Property::SessionExpiryInterval(value) => {
                                        match &mut properties.session_expiry_interval {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::ReceiveMaximum(value) => {
                                        match &mut properties.receive_maximum {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::MaximumQoS(value) => {
                                        match &mut properties.maximum_qos {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::RetainAvailable(value) => {
                                        match &mut properties.retain_available {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::MaximumPacketSize(value) => {
                                        match &mut properties.maximum_packet_size {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::AssignedClientIdentifier(value) => {
                                        match &mut properties.assigned_client_identifier {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::TopicAliasMaximum(value) => {
                                        match &mut properties.topic_alias_maximum {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::ReasonString(value) => {
                                        match &mut properties.reason_string {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::UserProperty(key, value) => {
                                        if properties.user_properties.len()
                                            >= parser_settings.max_user_properties_len
                                        {
                                            return Err(PropertiesError::from(
                                                TooManyUserPropertiesError,
                                            ));
                                        }
                                        properties.user_properties.push((key, value))
                                    }
                                    Property::WildcardSubscriptionAvailable(value) => {
                                        match &mut properties.wildcard_subscription_available {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::SubscriptionIdentifiersAvailable(value) => {
                                        match &mut properties.subscription_identifiers_available {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::SharedSubscriptionAvailable(value) => {
                                        match &mut properties.shared_subscription_available {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::ServerKeepAlive(value) => {
                                        match &mut properties.server_keep_alive {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::ResponseInformation(value) => {
                                        match &mut properties.response_information {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::ServerReference(value) => {
                                        match &mut properties.server_reference {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::AuthenticationMethod(value) => {
                                        match &mut authentication_method {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    Property::AuthenticationData(value) => {
                                        match &mut authentication_data {
                                            slot @ None => *slot = Some(value),
                                            _ => {
                                                return Err(PropertiesError::from(
                                                    DuplicatedPropertyError { property_type },
                                                ))
                                            }
                                        }
                                    }
                                    _ => {
                                        return Err(PropertiesError::from(
                                            UnsupportedPropertyError { property_type },
                                        ))
                                    }
                                };
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
