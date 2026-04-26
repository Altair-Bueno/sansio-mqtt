use core::num::NonZero;

use sansio_mqtt_v5_protocol::AuthPacket;
use sansio_mqtt_v5_protocol::BrokerMessage;
use sansio_mqtt_v5_protocol::DisconnectReasonCode;
use sansio_mqtt_v5_protocol::InboundMessageId;
use sansio_mqtt_v5_protocol::UserWriteOut;
use sansio_mqtt_v5_types::PubAckReasonCode;
use sansio_mqtt_v5_types::PubCompReasonCode;
use sansio_mqtt_v5_types::PubRecReasonCode;

#[derive(Debug)]
pub enum Event {
    Connected,
    /// The connection has been closed.
    ///
    /// `reason_code` is `Some` when the server initiated the DISCONNECT with a
    /// reason code, and `None` when the client disconnected or the socket
    /// was closed without a server DISCONNECT packet.
    Disconnected(Option<DisconnectReasonCode>),
    Message(BrokerMessage),
    MessageWithRequiredAcknowledgement(InboundMessageId, BrokerMessage),
    PublishAcknowledged(NonZero<u16>, PubAckReasonCode),
    PublishCompleted(NonZero<u16>, PubCompReasonCode),
    PublishDroppedDueToSessionNotResumed(NonZero<u16>),
    PublishDroppedDueToBrokerRejectedPubRec(NonZero<u16>, PubRecReasonCode),
    /// [MQTT-4.12.0-2] The server has initiated re-authentication via an AUTH
    /// packet.
    Auth(AuthPacket),
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
            UserWriteOut::Disconnected(reason_code) => Self::Disconnected(reason_code),
            UserWriteOut::Auth(auth) => Self::Auth(auth),
        }
    }
}
