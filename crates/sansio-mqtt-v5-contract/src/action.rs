use alloc::string::String;
use alloc::vec::Vec;

use crate::timer::TimerKey;
use sansio_mqtt_v5_types::DisconnectReasonCode;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    SendBytes(Vec<u8>),
    ScheduleTimer {
        key: TimerKey,
        delay_ms: u32,
    },
    CancelTimer(TimerKey),
    Connected,
    DisconnectedByRemote {
        reason_code: DisconnectReasonCode,
    },
    DisconnectedByTimeout,
    DisconnectedByLocalRequest,
    DisconnectedByProtocolViolation,
    PublishReceived {
        topic: String,
        payload: Vec<u8>,
    },
    SubscribeAck {
        packet_id: u16,
        reason_codes: Vec<u8>,
    },
}
