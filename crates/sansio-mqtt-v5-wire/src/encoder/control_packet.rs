use super::EncodeError;
use crate::ControlPacket;
use encode::ByteEncoder;
use encode::Encodable;

impl<E> Encodable<E> for ControlPacket
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;
    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        match self {
            Self::Connect(packet) => packet.encode(encoder),
            Self::ConnAck(packet) => packet.encode(encoder),
            Self::Publish(packet) => packet.encode(encoder),
            Self::PubAck(packet) => packet.encode(encoder),
            Self::PubRec(packet) => packet.encode(encoder),
            Self::PubRel(packet) => packet.encode(encoder),
            Self::PubComp(packet) => packet.encode(encoder),
            Self::Subscribe(packet) => packet.encode(encoder),
            Self::Unsubscribe(packet) => packet.encode(encoder),
            Self::SubAck(packet) => packet.encode(encoder),
            Self::UnsubAck(packet) => packet.encode(encoder),
            Self::PingReq(packet) => packet.encode(encoder),
            Self::PingResp(packet) => packet.encode(encoder),
            Self::Disconnect(packet) => packet.encode(encoder),
            Self::Auth(packet) => packet.encode(encoder),
        }
    }
}
