use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use sansio_mqtt_v5_contract::{PublishRequest, Qos, SubscribeRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub next_packet_id: u16,
    pub keepalive_delay_ms: u32,
    pub pingresp_timeout_ms: u32,
    pub ack_timeout_ms: u32,
    pub pending_qos1: Option<PendingQos1Publish>,
    pub pending_qos2: Option<PendingQos2Publish>,
    pub pending_subscribe: Option<PendingSubscribe>,
    pub pending_inbound_qos2: Option<PendingInboundQos2Publish>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingQos1Publish {
    pub packet_id: u16,
    pub publish: PublishRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingQos2Publish {
    pub packet_id: u16,
    pub publish: PublishRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSubscribe {
    pub packet_id: u16,
    pub request: SubscribeRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInboundQos2Publish {
    pub packet_id: u16,
    pub topic: String,
    pub payload: Vec<u8>,
}

impl Context {
    pub fn allocate_packet_id(&mut self) -> u16 {
        let packet_id = self.next_packet_id;
        self.next_packet_id = if self.next_packet_id == u16::MAX {
            1
        } else {
            self.next_packet_id + 1
        };
        packet_id
    }

    pub fn set_keepalive_from_connect(&mut self, keep_alive_secs: Option<u16>) {
        self.keepalive_delay_ms = keep_alive_secs.map_or(0, |seconds| u32::from(seconds) * 1_000);
    }

    pub fn store_pending_qos1(&mut self, packet_id: u16, publish: &PublishRequest) {
        let mut pending = publish.clone();
        pending.qos = Qos::AtLeast;

        self.pending_qos1 = Some(PendingQos1Publish {
            packet_id,
            publish: pending,
        });
    }

    pub fn store_pending_qos2(&mut self, packet_id: u16, publish: &PublishRequest) {
        let mut pending = publish.clone();
        pending.qos = Qos::Exactly;

        self.pending_qos2 = Some(PendingQos2Publish {
            packet_id,
            publish: pending,
        });
    }

    pub fn store_pending_subscribe(&mut self, packet_id: u16, request: &SubscribeRequest) {
        self.pending_subscribe = Some(PendingSubscribe {
            packet_id,
            request: request.clone(),
        });
    }

    pub fn store_pending_inbound_qos2(&mut self, packet_id: u16, topic: &str, payload: &[u8]) {
        self.pending_inbound_qos2 = Some(PendingInboundQos2Publish {
            packet_id,
            topic: topic.to_owned(),
            payload: payload.to_vec(),
        });
    }
}

impl Default for Context {
    fn default() -> Self {
        Self {
            next_packet_id: 1,
            keepalive_delay_ms: 60_000,
            pingresp_timeout_ms: 10_000,
            ack_timeout_ms: 5_000,
            pending_qos1: None,
            pending_qos2: None,
            pending_subscribe: None,
            pending_inbound_qos2: None,
        }
    }
}
