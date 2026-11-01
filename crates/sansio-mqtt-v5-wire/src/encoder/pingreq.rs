use super::EncodeError;
use super::basic::VariableByteInteger;
use super::basic::fixed_header;
use crate::ControlPacketType;
use crate::PingReq;
use crate::PingReqHeaderFlags;
use encode::ByteEncoder;
use encode::Encodable;

impl<E: ByteEncoder> Encodable<E> for PingReq
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(ControlPacketType::PingReq, u8::from(PingReqHeaderFlags)).encode(encoder)?;
        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new(())
            .encode(encoder)
    }
}
