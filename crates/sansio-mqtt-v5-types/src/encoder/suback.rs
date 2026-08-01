use super::*;

impl<E> Encodable<E> for SubAck
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(ControlPacketType::SubAck, u8::from(SubAckHeaderFlags)).encode(encoder)?;

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
