use super::EncodeError;
use super::basic::TwoByteInteger;
use super::basic::VariableByteInteger;
use super::basic::fixed_header;
use crate::ControlPacketType;
use crate::UnsubAck;
use crate::UnsubAckHeaderFlags;
use encode::ByteEncoder;
use encode::Encodable;

impl<E> Encodable<E> for UnsubAck
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(ControlPacketType::UnsubAck, u8::from(UnsubAckHeaderFlags)).encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::new(TwoByteInteger::new(self.packet_id.get())),
            &self.properties,
            encode::combinators::FromError::new(encode::combinators::Iter::new(
                self.reason_codes.iter(),
            )),
        ))
        .encode(encoder)
    }
}
