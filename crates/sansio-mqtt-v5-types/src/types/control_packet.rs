use super::*;

#[derive(Debug, PartialEq, Clone, EnumDiscriminants)]
#[strum_discriminants(derive(Hash, EnumIter, Display))]
#[strum_discriminants(name(ControlPacketType))]
#[allow(clippy::large_enum_variant)]
pub enum ControlPacket {
    Reserved(Reserved),
    Connect(Connect),
    ConnAck(ConnAck),
    Publish(Publish),
    PubAck(PubAck),
    PubRec(PubRec),
    PubRel(PubRel),
    PubComp(PubComp),
    Subscribe(Subscribe),
    SubAck(SubAck),
    Unsubscribe(Unsubscribe),
    UnsubAck(UnsubAck),
    PingReq(PingReq),
    PingResp(PingResp),
    Disconnect(Disconnect),
    Auth(Auth),
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

#[cfg(test)]
mod derive_guards {
    use super::*;

    trait MustNotImplementDefault {}
    impl<T: Default> MustNotImplementDefault for T {}

    impl MustNotImplementDefault for ControlPacket {}
    impl MustNotImplementDefault for Reserved {}
    impl MustNotImplementDefault for Connect {}
    impl MustNotImplementDefault for ConnAck {}
    impl MustNotImplementDefault for Publish {}
    impl MustNotImplementDefault for PubAck {}
    impl MustNotImplementDefault for PubRec {}
    impl MustNotImplementDefault for PubRel {}
    impl MustNotImplementDefault for PubComp {}
    impl MustNotImplementDefault for Subscribe {}
    impl MustNotImplementDefault for SubAck {}
    impl MustNotImplementDefault for Unsubscribe {}
    impl MustNotImplementDefault for UnsubAck {}
    impl MustNotImplementDefault for PingReq {}
    impl MustNotImplementDefault for PingResp {}
    impl MustNotImplementDefault for Disconnect {}
    impl MustNotImplementDefault for Auth {}

    fn assert_not_default<T: MustNotImplementDefault>() {}

    #[test]
    fn control_packets_do_not_derive_default() {
        assert_not_default::<ControlPacket>();
        assert_not_default::<Reserved>();
        assert_not_default::<Connect>();
        assert_not_default::<ConnAck>();
        assert_not_default::<Publish>();
        assert_not_default::<PubAck>();
        assert_not_default::<PubRec>();
        assert_not_default::<PubRel>();
        assert_not_default::<PubComp>();
        assert_not_default::<Subscribe>();
        assert_not_default::<SubAck>();
        assert_not_default::<Unsubscribe>();
        assert_not_default::<UnsubAck>();
        assert_not_default::<PingReq>();
        assert_not_default::<PingResp>();
        assert_not_default::<Disconnect>();
        assert_not_default::<Auth>();
    }
}
