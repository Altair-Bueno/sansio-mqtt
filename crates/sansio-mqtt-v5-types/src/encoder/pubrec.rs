use super::*;

impl<E> Encodable<E> for PubRec
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(ControlPacketType::PubRec, u8::from(PubRecHeaderFlags)).encode(encoder)?;

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
                    let success = **reason_code == PubRecReasonCode::Success;
                    let no_properties = properties.is_empty();

                    !(success && no_properties)
                },
            ),
        ))
        .encode(encoder)
    }
}
