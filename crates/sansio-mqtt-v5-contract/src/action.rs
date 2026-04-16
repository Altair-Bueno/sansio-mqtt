use alloc::string::String;
use alloc::vec::Vec;

use crate::error::DisconnectReason;
use crate::timer::TimerKey;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // Required Task 2 public API keeps this enum payload inline.
pub enum Action {
    SendBytes(Vec<u8>),
    ScheduleTimer { key: TimerKey, delay_ms: u32 },
    CancelTimer(TimerKey),
    SessionAction(SessionAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // Required Task 2 public API keeps publish data inline.
pub enum SessionAction {
    Connected,
    Disconnected {
        reason: DisconnectReason,
    },
    PublishReceived {
        topic: String,
        payload: Vec<u8>,
    },
    SubscribeAck {
        packet_id: u16,
        reason_codes: Vec<u8>,
    },
}
