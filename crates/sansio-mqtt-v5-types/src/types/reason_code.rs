use super::*;

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum ConnectReasonCode {
    #[default]
    Success = 0x00,
    UnspecifiedError = 0x80,
    MalformedPacket = 0x81,
    ProtocolError = 0x82,
    ImplementationSpecificError = 0x83,
    UnsupportedProtocolVersion = 0x84,
    ClientIdentifierNotValid = 0x85,
    BadUserNameOrPassword = 0x86,
    NotAuthorized = 0x87,
    ServerUnavailable = 0x88,
    ServerBusy = 0x89,
    Banned = 0x8A,
    BadAuthenticationMethod = 0x8C,
    TopicNameInvalid = 0x90,
    PacketTooLarge = 0x95,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
    RetainNotSupported = 0x9A,
    QoSNotSupported = 0x9B,
    UseAnotherServer = 0x9C,
    ServerMoved = 0x9D,
    ConnectionRateExceeded = 0x9F,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum ConnackReasonCode {
    #[default]
    Success = 0x00,
    UnspecifiedError = 0x80,
    MalformedPacket = 0x81,
    ProtocolError = 0x82,
    ImplementationSpecificError = 0x83,
    UnsupportedProtocolVersion = 0x84,
    ClientIdentifierNotValid = 0x85,
    BadUserNameOrPassword = 0x86,
    NotAuthorized = 0x87,
    ServerUnavailable = 0x88,
    ServerBusy = 0x89,
    Banned = 0x8A,
    BadAuthenticationMethod = 0x8C,
    TopicNameInvalid = 0x90,
    PacketTooLarge = 0x95,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
    RetainNotSupported = 0x9A,
    QoSNotSupported = 0x9B,
    UseAnotherServer = 0x9C,
    ServerMoved = 0x9D,
    ConnectionRateExceeded = 0x9F,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum PublishReasonCode {
    #[default]
    Success = 0x00,
    NoMatchingSubscribers = 0x10,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicNameInvalid = 0x90,
    PacketIdentifierInUse = 0x91,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum PubAckReasonCode {
    #[default]
    Success = 0x00,
    NoMatchingSubscribers = 0x10,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicNameInvalid = 0x90,
    PacketIdentifierInUse = 0x91,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum PubRecReasonCode {
    #[default]
    Success = 0x00,
    NoMatchingSubscribers = 0x10,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicNameInvalid = 0x90,
    PacketIdentifierInUse = 0x91,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum PubRelReasonCode {
    #[default]
    Success = 0x00,
    PacketIdentifierNotFound = 0x92,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum PubCompReasonCode {
    #[default]
    Success = 0x00,
    PacketIdentifierNotFound = 0x92,
}

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Display)]
pub enum SubAckReasonCode {
    SuccessQoS0 = 0x00,
    SuccessQoS1 = 0x01,
    SuccessQoS2 = 0x02,
    NoSubscriptionExisted = 0x11,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicFilterInvalid = 0x8F,
    PacketIdentifierInUse = 0x91,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
    RetainNotSupported = 0x9A,
    QoSNotSupported = 0x9B,
    UseAnotherServer = 0x9C,
    ServerMoved = 0x9D,
    ConnectionRateExceeded = 0x9F,
    SubscriptionIdentifiersNotSupported = 0xA1,
    WildcardSubscriptionsNotSupported = 0xA2,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum UnsubAckReasonCode {
    #[default]
    Success = 0x00,
    NoSubscriptionExisted = 0x11,
    UnspecifiedError = 0x80,
    ImplementationSpecificError = 0x83,
    NotAuthorized = 0x87,
    TopicFilterInvalid = 0x8F,
    PacketIdentifierInUse = 0x91,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum DisconnectReasonCode {
    #[default]
    NormalDisconnection = 0x00,
    DisconnectWithWillMessage = 0x04,
    UnspecifiedError = 0x80,
    MalformedPacket = 0x81,
    ProtocolError = 0x82,
    ImplementationSpecificError = 0x83,
    UnsupportedProtocolVersion = 0x84,
    ClientIdentifierNotValid = 0x85,
    BadUserNameOrPassword = 0x86,
    NotAuthorized = 0x87,
    ServerUnavailable = 0x88,
    ServerBusy = 0x89,
    Banned = 0x8A,
    BadAuthenticationMethod = 0x8C,
    ServerShuttingDown = 0x8B,
    KeepAliveTimeout = 0x8D,
    SessionTakenOver = 0x8E,
    TopicFilterInvalid = 0x8F,
    PacketIdentifierInUse = 0x91,
    PacketIdentifierNotFound = 0x92,
    ReceiveMaximumExceeded = 0x93,
    TopicAliasInvalid = 0x94,
    PacketTooLarge = 0x95,
    MessageRateTooHigh = 0x96,
    AdministrativeAction = 0x98,
    PayloadFormatInvalid = 0x99,
    RetainNotSupported = 0x9A,
    QoSNotSupported = 0x9B,
    UseAnotherServer = 0x9C,
    ServerMoved = 0x9D,
    SharedSubscriptionsNotSupported = 0x9E,
    ConnectionRateExceeded = 0x9F,
    MaximumConnectTime = 0xA0,
    SubscriptionIdentifiersNotSupported = 0xA1,
    WildcardSubscriptionsNotSupported = 0xA2,
}

#[derive(Debug, PartialEq, Clone, Copy, Default, EnumIter, Display)]
pub enum AuthReasonCode {
    #[default]
    Success = 0x00,
    ContinueAuthentication = 0x18,
    ReAuthenticate = 0x19,
}

#[derive(Debug, PartialEq, Clone, thiserror::Error)]
#[error("Invalid reason code {value}")]
pub struct InvalidReasonCode {
    value: u8,
}

macro_rules! impl_reason_code {
    ($name:ty) => {
        impl From<$name> for u8 {
            #[inline]
            fn from(value: $name) -> Self {
                value as u8
            }
        }

        impl TryFrom<u8> for $name {
            type Error = InvalidReasonCode;

            #[inline]
            fn try_from(value: u8) -> Result<Self, Self::Error> {
                Self::iter()
                    .find(|v| *v as u8 == value)
                    .ok_or(InvalidReasonCode { value })
            }
        }
    };
}

impl_reason_code!(ConnectReasonCode);
impl_reason_code!(ConnackReasonCode);
impl_reason_code!(PublishReasonCode);
impl_reason_code!(PubAckReasonCode);
impl_reason_code!(PubRecReasonCode);
impl_reason_code!(PubRelReasonCode);
impl_reason_code!(PubCompReasonCode);
impl_reason_code!(SubAckReasonCode);
impl_reason_code!(UnsubAckReasonCode);
impl_reason_code!(DisconnectReasonCode);
impl_reason_code!(AuthReasonCode);
