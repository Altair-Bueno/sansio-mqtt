use alloc::string::String;
use alloc::vec::Vec;
use sansio_mqtt_v5_types::Qos;

use crate::timer::TimerKey;
use crate::{ConnectOptions, PublishRequest, SubscribeRequest};

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
        topic: String,
        payload: Vec<u8>,
        qos: Qos,
        packet_id: Option<u16>,
    },
    PacketSubAck {
        packet_id: u16,
        reason_codes: Vec<u8>,
    },
    TimerFired(TimerKey),
    UserConnect(ConnectOptions),
    UserPublish(PublishRequest),
    UserSubscribe(SubscribeRequest),
    UserDisconnect,
}
