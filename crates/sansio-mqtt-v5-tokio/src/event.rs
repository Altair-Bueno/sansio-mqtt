use core::num::NonZero;

use sansio_mqtt_v5_protocol::{BrokerMessage, InboundMessageId, UserWriteOut};
use sansio_mqtt_v5_types::{PubAckReasonCode, PubCompReasonCode, PubRecReasonCode};

#[derive(Debug)]
pub enum Event {
    Connected,
    Disconnected,
    Message(BrokerMessage),
    MessageWithRequiredAcknowledgement(InboundMessageId, BrokerMessage),
    PublishAcknowledged(NonZero<u16>, PubAckReasonCode),
    PublishCompleted(NonZero<u16>, PubCompReasonCode),
    PublishDroppedDueToSessionNotResumed(NonZero<u16>),
    PublishDroppedDueToBrokerRejectedPubRec(NonZero<u16>, PubRecReasonCode),
}

impl Event {
    pub fn from_protocol_output(output: UserWriteOut) -> Self {
        match output {
            UserWriteOut::ReceivedMessage(message) => Self::Message(message),
            UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(id, message) => {
                Self::MessageWithRequiredAcknowledgement(id, message)
            }
            UserWriteOut::PublishAcknowledged(packet_id, reason_code) => {
                Self::PublishAcknowledged(packet_id, reason_code)
            }
            UserWriteOut::PublishCompleted(packet_id, reason_code) => {
                Self::PublishCompleted(packet_id, reason_code)
            }
            UserWriteOut::PublishDroppedDueToSessionNotResumed(packet_id) => {
                Self::PublishDroppedDueToSessionNotResumed(packet_id)
            }
            UserWriteOut::PublishDroppedDueToBrokerRejectedPubRec(packet_id, reason_code) => {
                Self::PublishDroppedDueToBrokerRejectedPubRec(packet_id, reason_code)
            }
            UserWriteOut::Connected => Self::Connected,
            UserWriteOut::Disconnected => Self::Disconnected,
        }
    }
}
