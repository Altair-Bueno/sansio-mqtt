#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    DecodeError,
    UnexpectedPacket,
    Timeout,
    PacketIdExhausted,
}
