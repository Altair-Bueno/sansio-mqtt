use super::*;

#[derive(Debug, PartialEq, Clone, Copy, EnumIter, Hash)]

pub enum ReasonCode {
    Success,
    NormalDisconnection,
    GrantedQoS0,
    GrantedQoS1,
    GrantedQoS2,
    DisconnectWithWillMessage,
    NoMatchingSubscribers,
    NoSubscriptionExisted,
    ContinueAuthentication,
    ReAuthenticate,
    UnspecifiedError,
    MalformedPacket,
    ProtocolError,
    ImplementationSpecificError,
    UnsupportedProtocolVersion,
    ClientIdentifierNotValid,
    BadUserNameOrPassword,
    NotAuthorized,
    ServerUnavailable,
    ServerBusy,
    Banned,
    ServerShuttingDown,
    BadAuthenticationMethod,
    KeepAliveTimeout,
    SessionTakenOver,
    TopicFilterInvalid,
    TopicNameInvalid,
    PacketIdentifierInUse,
    PacketIdentifierNotFound,
    ReceiveMaximumExceeded,
    TopicAliasInvalid,
    PacketTooLarge,
    MessageRateTooHigh,
    QuotaExceeded,
    AdministrativeAction,
    PayloadFormatInvalid,
    RetainNotSupported,
    QoSNotSupported,
    UseAnotherServer,
    ServerMoved,
    SharedSubscriptionsNotSupported,
    ConnectionRateExceeded,
    MaximumConnectTime,
    SubscriptionIdentifiersNotSupported,
    WildcardSubscriptionsNotSupported,
}
impl ReasonCode {
    #[inline]
    pub fn is_error(self) -> bool {
        !self.is_success()
    }
    #[inline]
    pub fn is_success(self) -> bool {
        u8::from(self) < 0x80
    }
    #[inline]
    pub fn from_connack(value: u8) -> Option<ReasonCode> {
        match value {
            0x00 => Some(ReasonCode::Success),
            0x80 => Some(ReasonCode::UnspecifiedError),
            0x81 => Some(ReasonCode::MalformedPacket),
            0x82 => Some(ReasonCode::ProtocolError),
            0x83 => Some(ReasonCode::ImplementationSpecificError),
            0x84 => Some(ReasonCode::UnsupportedProtocolVersion),
            0x85 => Some(ReasonCode::ClientIdentifierNotValid),
            0x86 => Some(ReasonCode::BadUserNameOrPassword),
            0x87 => Some(ReasonCode::NotAuthorized),
            0x88 => Some(ReasonCode::ServerUnavailable),
            0x89 => Some(ReasonCode::ServerBusy),
            0x8A => Some(ReasonCode::Banned),
            0x8C => Some(ReasonCode::BadAuthenticationMethod),
            0x90 => Some(ReasonCode::TopicNameInvalid),
            0x95 => Some(ReasonCode::PacketTooLarge),
            0x97 => Some(ReasonCode::QuotaExceeded),
            0x99 => Some(ReasonCode::PayloadFormatInvalid),
            0x9A => Some(ReasonCode::RetainNotSupported),
            0x9B => Some(ReasonCode::QoSNotSupported),
            0x9C => Some(ReasonCode::UseAnotherServer),
            0x9D => Some(ReasonCode::ServerMoved),
            0x9F => Some(ReasonCode::ConnectionRateExceeded),
            _ => None,
        }
    }

    #[inline]
    pub fn from_puback(value: u8) -> Option<ReasonCode> {
        match value {
            0x00 => Some(ReasonCode::Success),
            0x10 => Some(ReasonCode::NoMatchingSubscribers),
            0x80 => Some(ReasonCode::UnspecifiedError),
            0x83 => Some(ReasonCode::ImplementationSpecificError),
            0x87 => Some(ReasonCode::NotAuthorized),
            0x90 => Some(ReasonCode::TopicNameInvalid),
            0x91 => Some(ReasonCode::PacketIdentifierInUse),
            0x97 => Some(ReasonCode::QuotaExceeded),
            0x99 => Some(ReasonCode::PayloadFormatInvalid),
            _ => None,
        }
    }

    #[inline]
    pub fn from_pubrec(value: u8) -> Option<ReasonCode> {
        match value {
            0x00 => Some(ReasonCode::Success),
            0x10 => Some(ReasonCode::NoMatchingSubscribers),
            0x80 => Some(ReasonCode::UnspecifiedError),
            0x83 => Some(ReasonCode::ImplementationSpecificError),
            0x87 => Some(ReasonCode::NotAuthorized),
            0x90 => Some(ReasonCode::TopicNameInvalid),
            0x91 => Some(ReasonCode::PacketIdentifierInUse),
            0x97 => Some(ReasonCode::QuotaExceeded),
            0x99 => Some(ReasonCode::PayloadFormatInvalid),
            _ => None,
        }
    }

    #[inline]
    pub fn from_pubrel(value: u8) -> Option<ReasonCode> {
        match value {
            0x00 => Some(ReasonCode::Success),
            0x92 => Some(ReasonCode::PacketIdentifierNotFound),
            _ => None,
        }
    }

    #[inline]
    pub fn from_pubcomp(value: u8) -> Option<ReasonCode> {
        match value {
            0x00 => Some(ReasonCode::Success),
            0x92 => Some(ReasonCode::PacketIdentifierNotFound),
            _ => None,
        }
    }

    #[inline]
    pub fn from_disconnect(value: u8) -> Option<ReasonCode> {
        match value {
            0x00 => Some(ReasonCode::NormalDisconnection),
            0x04 => Some(ReasonCode::DisconnectWithWillMessage),
            0x80 => Some(ReasonCode::UnspecifiedError),
            0x81 => Some(ReasonCode::MalformedPacket),
            0x82 => Some(ReasonCode::ProtocolError),
            0x83 => Some(ReasonCode::ImplementationSpecificError),
            0x87 => Some(ReasonCode::NotAuthorized),
            0x89 => Some(ReasonCode::ServerBusy),
            0x8B => Some(ReasonCode::ServerShuttingDown),
            0x8D => Some(ReasonCode::KeepAliveTimeout),
            0x8E => Some(ReasonCode::SessionTakenOver),
            0x8F => Some(ReasonCode::TopicFilterInvalid),
            0x90 => Some(ReasonCode::TopicNameInvalid),
            0x93 => Some(ReasonCode::ReceiveMaximumExceeded),
            0x94 => Some(ReasonCode::TopicAliasInvalid),
            0x95 => Some(ReasonCode::PacketTooLarge),
            0x96 => Some(ReasonCode::MessageRateTooHigh),
            0x97 => Some(ReasonCode::QuotaExceeded),
            0x98 => Some(ReasonCode::AdministrativeAction),
            0x9A => Some(ReasonCode::RetainNotSupported),
            0x9B => Some(ReasonCode::QoSNotSupported),
            0x9C => Some(ReasonCode::UseAnotherServer),
            0x9D => Some(ReasonCode::ServerMoved),
            0x9E => Some(ReasonCode::SharedSubscriptionsNotSupported),
            0x9F => Some(ReasonCode::ConnectionRateExceeded),
            0xA0 => Some(ReasonCode::MaximumConnectTime),
            0xA1 => Some(ReasonCode::SubscriptionIdentifiersNotSupported),
            0xA2 => Some(ReasonCode::WildcardSubscriptionsNotSupported),
            _ => None,
        }
    }

    #[inline]
    pub fn from_auth(value: u8) -> Option<ReasonCode> {
        match value {
            0x18 => Some(ReasonCode::ContinueAuthentication),
            0x19 => Some(ReasonCode::ReAuthenticate),
            0x00 => Some(ReasonCode::Success),
            0x80 => Some(ReasonCode::UnspecifiedError),
            _ => None,
        }
    }
    #[inline]
    pub fn from_suback(value: u8) -> Option<ReasonCode> {
        match value {
            0x00 => Some(ReasonCode::GrantedQoS0),
            0x01 => Some(ReasonCode::GrantedQoS1),
            0x02 => Some(ReasonCode::GrantedQoS2),
            0x80 => Some(ReasonCode::UnspecifiedError),
            0x83 => Some(ReasonCode::ImplementationSpecificError),
            0x87 => Some(ReasonCode::NotAuthorized),
            0x8F => Some(ReasonCode::TopicFilterInvalid),
            0x91 => Some(ReasonCode::PacketIdentifierInUse),
            0x97 => Some(ReasonCode::QuotaExceeded),
            0x9E => Some(ReasonCode::SharedSubscriptionsNotSupported),
            0xA1 => Some(ReasonCode::SubscriptionIdentifiersNotSupported),
            0xA2 => Some(ReasonCode::WildcardSubscriptionsNotSupported),

            _ => None,
        }
    }
    #[inline]
    pub fn from_unsuback(value: u8) -> Option<ReasonCode> {
        match value {
            0x00 => Some(ReasonCode::Success),
            0x11 => Some(ReasonCode::NoSubscriptionExisted),
            0x80 => Some(ReasonCode::UnspecifiedError),
            0x83 => Some(ReasonCode::ImplementationSpecificError),
            0x87 => Some(ReasonCode::NotAuthorized),
            0x8F => Some(ReasonCode::TopicFilterInvalid),
            0x91 => Some(ReasonCode::PacketIdentifierInUse),
            _ => None,
        }
    }
}

impl From<ReasonCode> for u8 {
    #[inline]
    fn from(value: ReasonCode) -> Self {
        match value {
            ReasonCode::Success => 0x00,
            ReasonCode::NormalDisconnection => 0x00,
            ReasonCode::GrantedQoS0 => 0x00,
            ReasonCode::GrantedQoS1 => 0x01,
            ReasonCode::GrantedQoS2 => 0x02,
            ReasonCode::DisconnectWithWillMessage => 0x04,
            ReasonCode::NoMatchingSubscribers => 0x10,
            ReasonCode::NoSubscriptionExisted => 0x11,
            ReasonCode::ContinueAuthentication => 0x18,
            ReasonCode::ReAuthenticate => 0x19,
            ReasonCode::UnspecifiedError => 0x80,
            ReasonCode::MalformedPacket => 0x81,
            ReasonCode::ProtocolError => 0x82,
            ReasonCode::ImplementationSpecificError => 0x83,
            ReasonCode::UnsupportedProtocolVersion => 0x84,
            ReasonCode::ClientIdentifierNotValid => 0x85,
            ReasonCode::BadUserNameOrPassword => 0x86,
            ReasonCode::NotAuthorized => 0x87,
            ReasonCode::ServerUnavailable => 0x88,
            ReasonCode::ServerBusy => 0x89,
            ReasonCode::Banned => 0x8A,
            ReasonCode::ServerShuttingDown => 0x8B,
            ReasonCode::BadAuthenticationMethod => 0x8C,
            ReasonCode::KeepAliveTimeout => 0x8D,
            ReasonCode::SessionTakenOver => 0x8E,
            ReasonCode::TopicFilterInvalid => 0x8F,
            ReasonCode::TopicNameInvalid => 0x90,
            ReasonCode::PacketIdentifierInUse => 0x91,
            ReasonCode::PacketIdentifierNotFound => 0x92,
            ReasonCode::ReceiveMaximumExceeded => 0x93,
            ReasonCode::TopicAliasInvalid => 0x94,
            ReasonCode::PacketTooLarge => 0x95,
            ReasonCode::MessageRateTooHigh => 0x96,
            ReasonCode::QuotaExceeded => 0x97,
            ReasonCode::AdministrativeAction => 0x98,
            ReasonCode::PayloadFormatInvalid => 0x99,
            ReasonCode::RetainNotSupported => 0x9A,
            ReasonCode::QoSNotSupported => 0x9B,
            ReasonCode::UseAnotherServer => 0x9C,
            ReasonCode::ServerMoved => 0x9D,
            ReasonCode::SharedSubscriptionsNotSupported => 0x9E,
            ReasonCode::ConnectionRateExceeded => 0x9F,
            ReasonCode::MaximumConnectTime => 0xA0,
            ReasonCode::SubscriptionIdentifiersNotSupported => 0xA1,
            ReasonCode::WildcardSubscriptionsNotSupported => 0xA2,
        }
    }
}
