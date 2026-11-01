use super::EncodeError;
use super::basic::TwoByteInteger;
use super::basic::VariableByteInteger;
use super::basic::fixed_header;
use crate::ControlPacketType;
use crate::PubComp;
use crate::PubCompHeaderFlags;
use crate::PubCompReasonCode;
use encode::ByteEncoder;
use encode::Encodable;

impl<E> Encodable<E> for PubComp
where
    E: ByteEncoder,
    EncodeError: From<E::Error>,
{
    type Error = EncodeError;

    fn encode(&self, encoder: &mut E) -> Result<(), Self::Error> {
        fixed_header(ControlPacketType::PubComp, u8::from(PubCompHeaderFlags)).encode(encoder)?;

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
                    let success = **reason_code == PubCompReasonCode::Success;
                    let no_properties = properties.is_empty();

                    !(success && no_properties)
                },
            ),
        ))
        .encode(encoder)
    }
}
