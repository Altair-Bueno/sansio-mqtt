use alloc::collections::btree_map::BTreeMap;
use alloc::collections::btree_set::BTreeSet;
use core::num::NonZero;
use sansio_mqtt_v5_types::PubRecReasonCode;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::Topic;

/// Where an outbound QoS1/QoS2 PUBLISH is in its acknowledgement exchange.
///
/// The originating PUBLISH is retained until the exchange completes so it can
/// be retransmitted with DUP=1 on session resume ([MQTT-4.4.0-1]). Each
/// variant also carries the app-supplied correlation `token` from the
/// originating `Command::Publish`, echoed back in
/// `Event::PublishAcknowledged`/`PublishCompleted`/`PublishDropped`.
#[derive(Debug)]
pub(crate) enum OutboundInflightState {
    /// QoS1: sent, awaiting PUBACK ([MQTT-4.3.2-3]).
    Qos1AwaitPubAck { token: u64, publish: Publish },
    /// QoS2: sent, awaiting PUBREC ([MQTT-4.3.3-3]).
    Qos2AwaitPubRec { token: u64, publish: Publish },
    /// QoS2: PUBREL sent, awaiting PUBCOMP ([MQTT-4.3.3-5]). The PUBLISH is no
    /// longer retained, since only the PUBREL is retransmitted from here.
    Qos2AwaitPubComp { token: u64 },
}

impl OutboundInflightState {
    /// The app-supplied correlation token stored with this exchange.
    pub(crate) fn token(&self) -> u64 {
        match self {
            Self::Qos1AwaitPubAck { token, .. }
            | Self::Qos2AwaitPubRec { token, .. }
            | Self::Qos2AwaitPubComp { token } => *token,
        }
    }
}

/// Where an inbound QoS1/QoS2 PUBLISH is in its acknowledgement exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InboundInflightState {
    /// QoS1: delivered to the application, awaiting its accept/reject decision.
    Qos1AwaitAppDecision,
    /// QoS2: delivered to the application, awaiting its accept/reject decision.
    Qos2AwaitAppDecision,
    /// QoS2: PUBREC sent, awaiting PUBREL ([MQTT-4.3.3-2]).
    Qos2AwaitPubRel,
    /// QoS2: the application rejected the message with this Reason Code, which
    /// is repeated if the server redelivers the PUBLISH.
    Qos2Rejected(PubRecReasonCode),
}

/// Persistent per-connection MQTT session state.
///
/// # Message ordering
///
/// [MQTT-4.6.0-2] Per-topic ordering is preserved implicitly by the
/// single-threaded FSM: messages on the same topic are processed in the order
/// they arrive from the network. Cross-topic ordering is intentionally not
/// guaranteed and is not required by the spec; the `on_flight_sent` map
/// preserves per-stream QoS ordering but makes no promises across distinct
/// topics.
#[derive(Debug)]
pub(crate) struct ClientSession {
    /// Outbound QoS1/QoS2 exchanges awaiting acknowledgement, by Packet
    /// Identifier.
    pub(crate) on_flight_sent: BTreeMap<NonZero<u16>, OutboundInflightState>,
    /// Inbound QoS1/QoS2 exchanges awaiting acknowledgement, by Packet
    /// Identifier.
    pub(crate) on_flight_received: BTreeMap<NonZero<u16>, InboundInflightState>,
    /// Packet Identifiers of SUBSCRIBE packets awaiting SUBACK.
    pub(crate) pending_subscribe: BTreeSet<NonZero<u16>>,
    /// Packet Identifiers of UNSUBSCRIBE packets awaiting UNSUBACK.
    pub(crate) pending_unsubscribe: BTreeSet<NonZero<u16>>,
    /// Topic Aliases the server has assigned on the current connection.
    ///
    /// Connection-scoped, not session-scoped: cleared on every (re)connection
    /// per [MQTT-3.8.2-1].
    pub(crate) inbound_topic_aliases: BTreeMap<NonZero<u16>, Topic>,
    /// The next Packet Identifier to hand out, wrapping from `u16::MAX` to 1.
    ///
    /// [MQTT-2.2.1-3] A Packet Identifier of 0 is invalid, which the type rules
    /// out rather than a runtime check.
    pub(crate) next_packet_id: NonZero<u16>,
}

impl Default for ClientSession {
    fn default() -> Self {
        Self {
            on_flight_sent: BTreeMap::new(),
            on_flight_received: BTreeMap::new(),
            pending_subscribe: BTreeSet::new(),
            pending_unsubscribe: BTreeSet::new(),
            inbound_topic_aliases: BTreeMap::new(),
            next_packet_id: NonZero::<u16>::MIN,
        }
    }
}
