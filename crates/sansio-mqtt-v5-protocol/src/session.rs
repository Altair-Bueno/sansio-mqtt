use alloc::collections::btree_map::BTreeMap;
use core::num::NonZero;
use sansio_mqtt_v5_types::{PubRecReasonCode, Publish, Topic};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OutboundInflightState {
    Qos1AwaitPubAck { publish: Publish },
    Qos2AwaitPubRec { publish: Publish },
    Qos2AwaitPubComp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InboundInflightState {
    Qos1AwaitAppDecision,
    Qos2AwaitAppDecision,
    Qos2AwaitPubRel,
    Qos2Rejected(PubRecReasonCode),
}

/// Persistent per-connection MQTT session state.
///
/// # Message ordering
///
/// [MQTT-4.6.0-2] Per-topic ordering is preserved implicitly by the single-threaded
/// FSM: messages on the same topic are processed in the order they arrive from the
/// network. Cross-topic ordering is intentionally not guaranteed and is not required
/// by the spec; the `on_flight_sent` map preserves per-stream QoS ordering but makes
/// no promises across distinct topics.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientSession {
    pub(crate) on_flight_sent: BTreeMap<NonZero<u16>, OutboundInflightState>,
    pub(crate) on_flight_received: BTreeMap<NonZero<u16>, InboundInflightState>,
    pub(crate) pending_subscribe: BTreeMap<NonZero<u16>, ()>,
    pub(crate) pending_unsubscribe: BTreeMap<NonZero<u16>, ()>,
    pub(crate) inbound_topic_aliases: BTreeMap<NonZero<u16>, Topic>,
    pub(crate) next_packet_id: u16,
}

impl ClientSession {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

impl Default for ClientSession {
    fn default() -> Self {
        Self {
            on_flight_sent: BTreeMap::new(),
            on_flight_received: BTreeMap::new(),
            pending_subscribe: BTreeMap::new(),
            pending_unsubscribe: BTreeMap::new(),
            inbound_topic_aliases: BTreeMap::new(),
            next_packet_id: 1,
        }
    }
}
