#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Disconnected,
    Connecting,
    Idle,
    WaitingForPingResp,
    WaitingForPubAck { packet_id: u16 },
    WaitingForPubRec { packet_id: u16 },
    WaitingForPubComp { packet_id: u16 },
    WaitingForSubAck { packet_id: u16 },
}
