//! MQTT v5.0 Control Packet discriminant
//! ([§2.1.2 — MQTT Control Packet type](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)).
//!
//! Wraps all v5 control packets into a single tagged enum and
//! provides conversions between the tag and its one-byte wire
//! representation.
use super::*;

/// Tagged union over every MQTT v5.0 Control Packet type
/// ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022),
/// [§3 — MQTT Control Packets](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901019)).
///
/// Variants map one-to-one to the Control Packet Types defined in the
/// spec; the companion [`ControlPacketType`] enum carries just the
/// discriminant (the high-nibble of the first byte on the wire).
/// Conformance: `[MQTT-2.1.2-1]`.
#[derive(Debug, PartialEq, Clone, EnumDiscriminants)]
#[strum_discriminants(derive(Hash, EnumIter, Display))]
#[strum_discriminants(name(ControlPacketType))]
#[strum_discriminants(doc = "Control Packet Type discriminant of [`ControlPacket`] ([§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)). Identifies a packet type without the payload.")]
#[allow(clippy::large_enum_variant)]
pub enum ControlPacket {
    /// Reserved type (`0`, [§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022)).
    Reserved(Reserved),
    /// [`Connect`] (`1`, [§3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901033)).
    Connect(Connect),
    /// [`ConnAck`] (`2`, [§3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901074)).
    ConnAck(ConnAck),
    /// [`Publish`] (`3`, [§3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901100)).
    Publish(Publish),
    /// [`PubAck`] (`4`, [§3.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901121)).
    PubAck(PubAck),
    /// [`PubRec`] (`5`, [§3.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901131)).
    PubRec(PubRec),
    /// [`PubRel`] (`6`, [§3.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901141)).
    PubRel(PubRel),
    /// [`PubComp`] (`7`, [§3.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901151)).
    PubComp(PubComp),
    /// [`Subscribe`] (`8`, [§3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901161)).
    Subscribe(Subscribe),
    /// [`SubAck`] (`9`, [§3.9](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901171)).
    SubAck(SubAck),
    /// [`Unsubscribe`] (`10`, [§3.10](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901179)).
    Unsubscribe(Unsubscribe),
    /// [`UnsubAck`] (`11`, [§3.11](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901187)).
    UnsubAck(UnsubAck),
    /// [`PingReq`] (`12`, [§3.12](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901195)).
    PingReq(PingReq),
    /// [`PingResp`] (`13`, [§3.13](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901200)).
    PingResp(PingResp),
    /// [`Disconnect`] (`14`, [§3.14](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901205)).
    Disconnect(Disconnect),
    /// [`Auth`] (`15`, [§3.15](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901217)).
    Auth(Auth),
}

/// Error returned when converting a byte into a [`ControlPacketType`]
/// fails because the byte does not correspond to any packet type
/// defined in
/// [§2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901022).
///
/// Conformance: `[MQTT-2.1.3-1]`.
#[derive(Debug, PartialEq, Clone, Copy, Error)]
#[error("Invalid control packet type: {value}")]
#[repr(transparent)]
pub struct InvalidControlPacketTypeError {
    /// Offending byte value.
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
