//! MQTT v5.0 `PUBLISH` packet
//! ([§3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901100)).
//!
//! Transports an Application Message. Conformance: `[MQTT-3.3.1-1]`,
//! `[MQTT-3.3.1-2]`, `[MQTT-3.3.1-3]`, `[MQTT-3.3.1-4]`.
use super::*;

/// MQTT v5.0 `PUBLISH` packet
/// ([§3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901100)).
///
/// Publishes an Application Message to a Topic; may be sent from
/// Client to Server or Server to Client. Conformance:
/// `[MQTT-3.3.1-1]`, `[MQTT-3.3.1-2]`, `[MQTT-3.3.1-3]`,
/// `[MQTT-3.3.1-4]`, `[MQTT-3.3.1-5]`, `[MQTT-3.3.2-1]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Publish {
    /// QoS-dependent delivery metadata (packet identifier, QoS,
    /// duplicate flag).
    pub kind: PublishKind,
    /// Retain flag; when true the Server stores the message as the
    /// retained message for the topic
    /// ([§3.3.1.3 — RETAIN](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901104),
    /// [MQTT-3.3.1-5], [MQTT-3.3.1-6], [MQTT-3.3.1-7],
    /// [MQTT-3.3.1-8], [MQTT-3.3.1-9], [MQTT-3.3.1-10],
    /// [MQTT-3.3.1-11], [MQTT-3.3.1-12], [MQTT-3.3.1-13]).
    pub retain: bool,
    /// Application Message payload
    /// ([§3.3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901119)).
    pub payload: Payload,
    /// Topic Name the message is published to
    /// ([§3.3.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901107),
    /// [MQTT-3.3.2-1], [MQTT-3.3.2-2]).
    pub topic: Topic,
    /// `PUBLISH` Properties
    /// ([§3.3.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901109)).
    pub properties: PublishProperties,
}

/// Discriminates between QoS 0 (no acknowledgement) and QoS 1/2
/// (identified, guaranteed-delivery) `PUBLISH` packets
/// ([§3.3.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901103)).
///
/// The Packet Identifier is only present for QoS > 0
/// ([MQTT-2.2.1-2], [MQTT-2.2.1-3]). Models the two flavours with
/// distinct variants to keep the invariant at the type level.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PublishKind {
    /// QoS 0 PUBLISH: no Packet Identifier, no acknowledgement
    /// ([§4.3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901235),
    /// [MQTT-3.3.1-2]).
    FireAndForget,
    /// QoS 1 or 2 PUBLISH: carries a Packet Identifier and may be a
    /// duplicate retransmission
    /// ([§4.3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901236),
    /// [§4.3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901237),
    /// [MQTT-2.2.1-3], [MQTT-3.3.1-1]).
    Repetible {
        /// Packet Identifier; non-zero per [MQTT-2.2.1-3].
        packet_id: NonZero<u16>,
        /// Delivery QoS (1 or 2 only; QoS 0 uses
        /// [`PublishKind::FireAndForget`]).
        qos: GuaranteedQoS,
        /// Duplicate delivery flag
        /// ([§3.3.1.1 DUP](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901102),
        /// [MQTT-3.3.1-1]).
        dup: bool,
    },
}

/// Fixed-header flag byte decomposition for `PUBLISH`
/// ([§3.3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901101)).
///
/// Unlike other packets, `PUBLISH` header flags are not fixed: they
/// carry DUP, QoS and RETAIN. Conformance: `[MQTT-3.3.1-1]`,
/// `[MQTT-3.3.1-2]`, `[MQTT-3.3.1-3]`, `[MQTT-3.3.1-4]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PublishHeaderFlags {
    /// Split-out DUP and QoS fields.
    pub kind: PublishHeaderFlagsKind,
    /// RETAIN flag; see [`Publish::retain`].
    pub retain: bool,
}

impl From<PublishHeaderFlags> for u8 {
    fn from(flags: PublishHeaderFlags) -> u8 {
        let mut byte = 0u8;

        byte |= u8::from(flags.retain);
        match flags.kind {
            PublishHeaderFlagsKind::Simple => (),
            PublishHeaderFlagsKind::Advanced { qos, dup } => {
                byte |= u8::from(qos) << 1;
                byte |= u8::from(dup) << 3;
            }
        };

        byte
    }
}

/// DUP and QoS portion of the `PUBLISH` fixed-header flag byte
/// ([§3.3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901101)).
///
/// DUP MUST be 0 for QoS 0 messages ([MQTT-3.3.1-2]); modelled with
/// separate `Simple` / `Advanced` variants.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PublishHeaderFlagsKind {
    /// QoS 0 PUBLISH; DUP MUST be 0 ([MQTT-3.3.1-2]).
    Simple,
    /// QoS 1 or 2 PUBLISH; carries DUP and QoS ([MQTT-3.3.1-1]).
    Advanced {
        /// Delivery QoS (1 or 2 only).
        qos: GuaranteedQoS,
        /// DUP flag.
        dup: bool,
    },
}

/// `PUBLISH` Properties
/// ([§3.3.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901109)).
///
/// All fields are optional. Subscription Identifiers are forwarded
/// by a Server to matching subscribers
/// ([§3.3.2.3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901117)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct PublishProperties {
    /// Payload Format Indicator
    /// ([§3.3.2.3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901111),
    /// [MQTT-3.3.2-4], [MQTT-3.3.2-5]).
    pub payload_format_indicator: Option<FormatIndicator>,
    /// Message Expiry Interval in seconds
    /// ([§3.3.2.3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901112)).
    pub message_expiry_interval: Option<u32>,
    /// Topic Alias
    /// ([§3.3.2.3.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901113),
    /// [MQTT-3.3.2-6], [MQTT-3.3.2-8]).
    pub topic_alias: Option<NonZero<u16>>,
    /// Response Topic for request/response
    /// ([§3.3.2.3.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901114),
    /// [MQTT-3.3.2-13]).
    pub response_topic: Option<Topic>,
    /// Correlation Data
    /// ([§3.3.2.3.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901115),
    /// [MQTT-3.3.2-14], [MQTT-3.3.2-15]).
    pub correlation_data: Option<BinaryData>,
    /// User Properties
    /// ([§3.3.2.3.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901116)).
    /// Order is preserved on the wire.
    pub user_properties: Vec<(Utf8String, Utf8String)>,
    /// Subscription Identifiers forwarded by a Server to matching
    /// subscribers
    /// ([§3.3.2.3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901117),
    /// [MQTT-3.3.4-1], [MQTT-3.8.2-2]).
    ///
    /// An empty `Vec` means the property was absent on the wire.
    pub subscription_identifiers: Vec<NonZero<u64>>,
    /// Content Type describing the payload
    /// ([§3.3.2.3.9](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901118)).
    pub content_type: Option<Utf8String>,
}
