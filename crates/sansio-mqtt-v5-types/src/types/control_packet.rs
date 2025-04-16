use super::*;

#[derive(Debug, PartialEq, Clone, EnumDiscriminants)]
#[strum_discriminants(derive(Hash, EnumIter, Display))]
#[strum_discriminants(name(ControlPacketType))]
pub enum ControlPacket<'input> {
    Reserved(Reserved),

    Connect(Connect<'input>),

    ConnAck(ConnAck<'input>),

    Publish(Publish<'input>),

    PubAck(PubAck<'input>),

    PubRec(PubRec<'input>),

    PubRel(PubRel<'input>),

    PubComp(PubComp<'input>),

    Subscribe(Subscribe<'input>),

    SubAck(SubAck<'input>),

    Unsubscribe(Unsubscribe<'input>),

    UnsubAck(UnsubAck<'input>),
    PingReq(PingReq),
    PingResp(PingResp),

    Disconnect(Disconnect<'input>),

    Auth(Auth<'input>),
}

#[derive(Debug, PartialEq, Clone, Copy, Error)]
#[error("Invalid control packet type: {value}")]
#[repr(transparent)]
pub struct InvalidControlPacketTypeError {
    pub value: u8,
}

impl TryFrom<u8> for ControlPacketType {
    type Error = InvalidControlPacketTypeError;

    #[inline]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        ControlPacketType::iter()
            .find(|v| *v as u8 == value)
            .ok_or(InvalidControlPacketTypeError { value })
    }
}

impl From<ControlPacketType> for u8 {
    #[inline]
    fn from(value: ControlPacketType) -> Self {
        value as u8
    }
}
