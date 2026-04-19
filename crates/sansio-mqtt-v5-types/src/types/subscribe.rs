//! MQTT v5.0 `SUBSCRIBE` packet
//! ([§3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901161)).
//!
//! Sent from Client to Server to create or modify subscriptions.
//! Conformance: `[MQTT-3.8.0-1]`, `[MQTT-3.8.1-1]`, `[MQTT-3.8.3-1]`,
//! `[MQTT-3.8.3-2]`, `[MQTT-3.8.3-3]`.
use super::*;

/// MQTT v5.0 `SUBSCRIBE` packet
/// ([§3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901161)).
///
/// At least one [`Subscription`] is required
/// ([MQTT-3.8.3-3]); extra ones are modelled via
/// [`Subscribe::extra_subscriptions`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Subscribe {
    /// Packet Identifier
    /// ([§3.8.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901163),
    /// [MQTT-2.2.1-3]).
    pub packet_id: NonZero<u16>,
    /// First subscription carried by the packet
    /// ([MQTT-3.8.3-3]).
    pub subscription: Subscription,
    /// Additional subscriptions beyond the first, in wire order.
    pub extra_subscriptions: Vec<Subscription>,
    /// `SUBSCRIBE` Properties
    /// ([§3.8.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901164)).
    pub properties: SubscribeProperties,
}

/// Fixed-header flags byte for `SUBSCRIBE`
/// ([§3.8.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901162)).
///
/// MUST be `0b0010`; any other value is Malformed Packet
/// ([MQTT-3.8.1-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SubscribeHeaderFlags;

impl From<SubscribeHeaderFlags> for u8 {
    fn from(_: SubscribeHeaderFlags) -> u8 {
        0b0000_0010
    }
}

/// One Topic Filter and its Subscription Options
/// ([§3.8.3.1 Subscription Options](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901169)).
///
/// Conformance: `[MQTT-3.8.3-2]`, `[MQTT-3.8.3-4]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Subscription {
    /// Topic Filter, which may contain wildcards; MUST be a valid
    /// UTF-8 string ([MQTT-3.8.3-1], [MQTT-4.7.1-1],
    /// [MQTT-4.7.1-2]).
    pub topic_filter: Utf8String,
    /// Maximum QoS the subscriber wishes to receive.
    pub qos: Qos,
    /// No Local option: if true, Application Messages MUST NOT be
    /// forwarded back to the Client that sent them
    /// ([MQTT-3.8.3-3]).
    pub no_local: bool,
    /// Retain As Published option
    /// ([§3.8.3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901169)).
    pub retain_as_published: bool,
    /// Retain Handling option
    /// ([§3.8.3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901169),
    /// [MQTT-3.8.3-4]).
    pub retain_handling: RetainHandling,
}

/// `SUBSCRIBE` Properties
/// ([§3.8.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901164)).
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct SubscribeProperties {
    /// Subscription Identifier
    /// ([§3.8.2.1.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901166),
    /// [MQTT-3.8.2-1], [MQTT-3.8.2-2]).
    ///
    /// A value of `0` is a Protocol Error, hence [`NonZero`].
    pub subscription_identifier: Option<NonZero<u64>>,
    /// User Properties
    /// ([§3.8.2.1.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901167)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
