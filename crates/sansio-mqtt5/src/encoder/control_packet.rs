use super::*;

impl<E> Encodable<E> for ControlPacket<'_>
where
    E: Encoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;
    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        match self {
            // ControlPacket::Reserved(packet) => packet.encode(encoder),
            // ControlPacket::Connect(packet) => packet.encode(encoder),
            // ControlPacket::ConnAck(packet) => packet.encode(encoder),
            // ControlPacket::Publish(packet) => packet.encode(encoder),
            ControlPacket::PubAck(packet) => packet.encode(encoder),
            ControlPacket::PubRec(packet) => packet.encode(encoder),
            ControlPacket::PubRel(packet) => packet.encode(encoder),
            ControlPacket::PubComp(packet) => packet.encode(encoder),
            // ControlPacket::Subscribe(packet) => packet.encode(encoder),
            // ControlPacket::Unsubscribe(packet) => packet.encode(encoder),
            ControlPacket::SubAck(packet) => packet.encode(encoder),
            ControlPacket::UnsubAck(packet) => packet.encode(encoder),
            ControlPacket::PingReq(packet) => packet.encode(encoder),
            ControlPacket::PingResp(packet) => packet.encode(encoder),
            ControlPacket::Disconnect(packet) => packet.encode(encoder),
            ControlPacket::Auth(packet) => packet.encode(encoder),
            _ => unimplemented!(),
        }
    }
}
