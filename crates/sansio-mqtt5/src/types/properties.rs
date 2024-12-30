use super::*;

#[derive(Debug, PartialEq, Clone, EnumDiscriminants)]
#[strum_discriminants(derive(Hash, EnumIter))]
#[strum_discriminants(name(PropertyType))]

pub enum Property<'input> {
    PayloadFormatIndicator(FormatIndicator),
    MessageExpiryInterval(u32),

    ContentType(MQTTString<'input>),

    ResponseTopic(PublishTopic<'input>),

    CorrelationData(Cow<'input, [u8]>),
    // It is a Protocol Error if the Subscription Identifier has a value of 0.
    SubscriptionIdentifier(NonZero<u64>),
    SessionExpiryInterval(u32),

    AssignedClientIdentifier(MQTTString<'input>),
    ServerKeepAlive(u16),

    AuthenticationMethod(MQTTString<'input>),

    AuthenticationData(Cow<'input, [u8]>),
    RequestProblemInformation(bool),
    WillDelayInterval(u32),
    RequestResponseInformation(bool),

    ResponseInformation(MQTTString<'input>),

    ServerReference(MQTTString<'input>),

    ReasonString(MQTTString<'input>),
    // It is a Protocol Error to include the Receive Maximum value more than once or for it to have the value 0.
    ReceiveMaximum(NonZero<u16>),
    TopicAliasMaximum(u16),
    TopicAlias(NonZero<u16>),
    MaximumQoS(MaximumQoS),
    RetainAvailable(bool),

    UserProperty(MQTTString<'input>, MQTTString<'input>),
    MaximumPacketSize(NonZero<u32>),
    WildcardSubscriptionAvailable(bool),
    SubscriptionIdentifiersAvailable(bool),
    SharedSubscriptionAvailable(bool),
}

#[derive(Debug, PartialEq, Clone)]

pub enum AuthenticationKind<'input> {
    WithoutData {
        method: MQTTString<'input>,
    },
    WithData {
        method: MQTTString<'input>,
        data: Cow<'input, [u8]>,
    },
}

#[derive(Debug, PartialEq, Clone, Copy, Error)]
#[error("Invalid property type: {value}")]
#[repr(transparent)]
pub struct InvalidPropertyTypeError {
    pub value: u64,
}

impl TryFrom<u64> for PropertyType {
    type Error = InvalidPropertyTypeError;

    #[inline]
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(PropertyType::PayloadFormatIndicator),
            0x02 => Ok(PropertyType::MessageExpiryInterval),
            0x03 => Ok(PropertyType::ContentType),
            0x08 => Ok(PropertyType::ResponseTopic),
            0x09 => Ok(PropertyType::CorrelationData),
            0x0B => Ok(PropertyType::SubscriptionIdentifier),
            0x11 => Ok(PropertyType::SessionExpiryInterval),
            0x12 => Ok(PropertyType::AssignedClientIdentifier),
            0x13 => Ok(PropertyType::ServerKeepAlive),
            0x15 => Ok(PropertyType::AuthenticationMethod),
            0x16 => Ok(PropertyType::AuthenticationData),
            0x17 => Ok(PropertyType::RequestProblemInformation),
            0x18 => Ok(PropertyType::WillDelayInterval),
            0x19 => Ok(PropertyType::RequestResponseInformation),
            0x1A => Ok(PropertyType::ResponseInformation),
            0x1C => Ok(PropertyType::ServerReference),
            0x1F => Ok(PropertyType::ReasonString),
            0x21 => Ok(PropertyType::ReceiveMaximum),
            0x22 => Ok(PropertyType::TopicAliasMaximum),
            0x23 => Ok(PropertyType::TopicAlias),
            0x24 => Ok(PropertyType::MaximumQoS),
            0x25 => Ok(PropertyType::RetainAvailable),
            0x26 => Ok(PropertyType::UserProperty),
            0x27 => Ok(PropertyType::MaximumPacketSize),
            0x28 => Ok(PropertyType::WildcardSubscriptionAvailable),
            0x29 => Ok(PropertyType::SubscriptionIdentifiersAvailable),
            0x2A => Ok(PropertyType::SharedSubscriptionAvailable),
            value => Err(InvalidPropertyTypeError { value }),
        }
    }
}

impl From<PropertyType> for u64 {
    fn from(value: PropertyType) -> Self {
        match value {
            PropertyType::PayloadFormatIndicator => 0x01,
            PropertyType::MessageExpiryInterval => 0x02,
            PropertyType::ContentType => 0x03,
            PropertyType::ResponseTopic => 0x08,
            PropertyType::CorrelationData => 0x09,
            PropertyType::SubscriptionIdentifier => 0x0B,
            PropertyType::SessionExpiryInterval => 0x11,
            PropertyType::AssignedClientIdentifier => 0x12,
            PropertyType::ServerKeepAlive => 0x13,
            PropertyType::AuthenticationMethod => 0x15,
            PropertyType::AuthenticationData => 0x16,
            PropertyType::RequestProblemInformation => 0x17,
            PropertyType::WillDelayInterval => 0x18,
            PropertyType::RequestResponseInformation => 0x19,
            PropertyType::ResponseInformation => 0x1A,
            PropertyType::ServerReference => 0x1C,
            PropertyType::ReasonString => 0x1F,
            PropertyType::ReceiveMaximum => 0x21,
            PropertyType::TopicAliasMaximum => 0x22,
            PropertyType::TopicAlias => 0x23,
            PropertyType::MaximumQoS => 0x24,
            PropertyType::RetainAvailable => 0x25,
            PropertyType::UserProperty => 0x26,
            PropertyType::MaximumPacketSize => 0x27,
            PropertyType::WildcardSubscriptionAvailable => 0x28,
            PropertyType::SubscriptionIdentifiersAvailable => 0x29,
            PropertyType::SharedSubscriptionAvailable => 0x2A,
        }
    }
}
