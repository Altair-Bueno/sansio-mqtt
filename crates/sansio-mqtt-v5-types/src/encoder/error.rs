use core::convert::Infallible;
use core::num::TryFromIntError;

use encode::encoders::InsufficientSpace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EncodeError {
    #[error("{0}. MQTT packet must be less than {variable_len} bytes in size with strings/byte arrays of at most {two_len} bytes long", two_len = u16::MAX, variable_len = 268_435_455)]
    PacketTooLarge(#[from] TryFromIntError),
    #[error(transparent)]
    InsufficientSpace(#[from] InsufficientSpace),
}

impl From<Infallible> for EncodeError {
    fn from(_: Infallible) -> Self {
        unreachable!("Infallible cannot be constructed")
    }
}
