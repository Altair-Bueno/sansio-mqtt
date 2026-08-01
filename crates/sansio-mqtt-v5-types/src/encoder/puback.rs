use super::*;

impl<E> Encodable<E> for PubAck
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(ControlPacketType::PubAck, u8::from(PubAckHeaderFlags)).encode(encoder)?;

        encode::combinators::LengthPrefix::<_, VariableByteInteger, Self::Error>::new((
            encode::combinators::FromError::<_, Self::Error>::new(TwoByteInteger::new(
                self.packet_id.get(),
            )),
            encode::combinators::Cond::new(
                (
                    encode::combinators::FromError::new(self.reason_code),
                    &self.properties,
                ),
                |(reason_code, properties)| {
                    let success = **reason_code == PubAckReasonCode::Success;
                    let no_properties = properties.is_empty();

                    !(success && no_properties)
                },
            ),
        ))
        .encode(encoder)
    }
}
