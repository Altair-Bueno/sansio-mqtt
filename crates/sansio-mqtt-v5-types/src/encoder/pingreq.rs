use super::*;

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
