use super::*;

impl<E: ByteEncoder> Encodable<E> for Reserved
where
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        let mut header_flags = 0u8;
        header_flags |= u8::from(ControlPacketType::Reserved) << 4;
        header_flags |= u8::from(ReservedHeaderFlags);

        header_flags.encode(encoder)?;
        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new(())
            .encode(encoder)
    }
}
