use crate::timer::TimerKey;
use crate::{
    ConnectOptions, PublishRequest, Qos, SubscribeRequest, SESSION_ACTION_PAYLOAD_CAPACITY,
    SESSION_ACTION_TOPIC_CAPACITY, SUBACK_REASON_CODES_CAPACITY,
};
use heapless::{String, Vec};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // Required Task 2 public API keeps request payloads inline.
pub enum Input<'a> {
    BytesReceived(&'a [u8]),
    PacketConnAck,
    PacketPingResp,
    PacketPubAck {
        packet_id: u16,
    },
    PacketPubRec {
        packet_id: u16,
    },
    PacketPubRel {
        packet_id: u16,
    },
    PacketPubComp {
        packet_id: u16,
    },
    PacketPublish {
        topic: String<SESSION_ACTION_TOPIC_CAPACITY>,
        payload: Vec<u8, SESSION_ACTION_PAYLOAD_CAPACITY>,
        qos: Qos,
        packet_id: Option<u16>,
    },
    PacketSubAck {
        packet_id: u16,
        reason_codes: Vec<u8, SUBACK_REASON_CODES_CAPACITY>,
    },
    TimerFired(TimerKey),
    UserConnect(ConnectOptions),
    UserPublish(PublishRequest),
    UserSubscribe(SubscribeRequest),
    UserDisconnect,
}
