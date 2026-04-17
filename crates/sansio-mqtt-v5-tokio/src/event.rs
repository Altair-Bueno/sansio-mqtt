use core::num::NonZero;

use sansio_mqtt_v5_protocol::{BrokerMessage, PublishDroppedReason, UserWriteOut};
use sansio_mqtt_v5_types::{PubAckReasonCode, PubCompReasonCode};

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Connected,
    Disconnected,
    Message(BrokerMessage),
    PublishAcknowledged {
        packet_id: NonZero<u16>,
        reason_code: PubAckReasonCode,
    },
    PublishCompleted {
        packet_id: NonZero<u16>,
        reason_code: PubCompReasonCode,
    },
    PublishDropped {
        packet_id: NonZero<u16>,
        reason: PublishDroppedReason,
    },
}

impl Event {
    pub fn from_protocol_output(output: UserWriteOut) -> Self {
        match output {
            UserWriteOut::ReceivedMessage(message) => Self::Message(message),
            UserWriteOut::PublishAcknowledged {
                packet_id,
                reason_code,
            } => Self::PublishAcknowledged {
                packet_id,
                reason_code,
            },
            UserWriteOut::PublishCompleted {
                packet_id,
                reason_code,
            } => Self::PublishCompleted {
                packet_id,
                reason_code,
            },
            UserWriteOut::PublishDropped { packet_id, reason } => {
                Self::PublishDropped { packet_id, reason }
            }
            UserWriteOut::Connected => Self::Connected,
            UserWriteOut::Disconnected => Self::Disconnected,
        }
    }
}
