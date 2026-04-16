use crate::error::DisconnectReason;
use crate::options::{SESSION_ACTION_PAYLOAD_CAPACITY, SESSION_ACTION_TOPIC_CAPACITY};
use crate::timer::TimerKey;
use crate::SUBACK_REASON_CODES_CAPACITY;
use heapless::{String, Vec};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // Required Task 2 public API keeps this enum payload inline.
pub enum Action {
    SendBytes(Vec<u8, 256>),
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
        topic: String<SESSION_ACTION_TOPIC_CAPACITY>,
        payload: Vec<u8, SESSION_ACTION_PAYLOAD_CAPACITY>,
    },
    SubscribeAck {
        packet_id: u16,
        reason_codes: Vec<u8, SUBACK_REASON_CODES_CAPACITY>,
    },
}
