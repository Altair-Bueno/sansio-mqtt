#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    Normal,
    ProtocolError,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    DecodeError,
    UnexpectedPacket,
    Timeout,
    PacketIdExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsError {
    TopicFilterTooLong,
}
