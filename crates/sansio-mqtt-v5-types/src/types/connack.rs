//! MQTT v5.0 `CONNACK` packet
//! ([§3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901074)).
//!
//! The Server's response to a [`Connect`] packet. Conformance:
//! `[MQTT-3.2.0-1]`, `[MQTT-3.2.0-2]`.
use super::*;

/// MQTT v5.0 `CONNACK` packet
/// ([§3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901074)).
///
/// Sent by the Server in response to a `CONNECT` packet; carries the
/// acknowledgement flags (split into [`ConnAckKind`]) and the
/// server-negotiated `CONNACK` properties. Conformance:
/// `[MQTT-3.2.0-1]`, `[MQTT-3.2.0-2]`, `[MQTT-3.2.2-1]`,
/// `[MQTT-3.2.2-2]`, `[MQTT-3.2.2-7]`, `[MQTT-3.2.2-8]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ConnAck {
    /// Acknowledge Flags and Reason Code
    /// ([§3.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901077)).
    pub kind: ConnAckKind,
    /// `CONNACK` Properties
    /// ([§3.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901080)).
    pub properties: ConnAckProperties,
}

/// Acknowledge Flags and Reason Code as a single invariant-preserving
/// enum
/// ([§3.2.2.1 Session Present](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901078),
/// [§3.2.2.2 Connect Reason Code](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901079)).
///
/// A Session Present flag of 1 is only permitted together with a
/// Reason Code of Success ([MQTT-3.2.2-4], [MQTT-3.2.2-5],
/// [MQTT-3.2.2-6]); modelling this with an enum makes the invalid
/// combination unrepresentable.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ConnAckKind {
    /// Session Present is 1; Reason Code MUST be Success
    /// ([MQTT-3.2.2-4]).
    ResumePreviousSession,
    /// Session Present is 0; Reason Code is as given
    /// ([MQTT-3.2.2-6], [MQTT-3.2.2-7]).
    Other {
        /// Connect Reason Code returned by the Server.
        reason_code: ConnackReasonCode,
    },
}

/// Fixed-header flags byte for `CONNACK`
/// ([§3.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901075)).
///
/// MUST be `0b0000`; any other value is Malformed Packet
/// ([MQTT-3.2.1-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ConnAckHeaderFlags;

impl From<ConnAckHeaderFlags> for u8 {
    fn from(_: ConnAckHeaderFlags) -> u8 {
        0
    }
}

/// `CONNACK` Properties
/// ([§3.2.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901080)).
///
/// Set by the Server to negotiate session parameters; all fields are
/// optional and use spec-mandated defaults when absent.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct ConnAckProperties {
    /// Session Expiry Interval in seconds
    /// ([§3.2.2.3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901082),
    /// [MQTT-3.2.2-10]).
    pub session_expiry_interval: Option<u32>,
    /// Receive Maximum
    /// ([§3.2.2.3.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901083),
    /// [MQTT-3.2.2-11]).
    pub receive_maximum: Option<NonZero<u16>>,
    /// Maximum QoS supported by the Server
    /// ([§3.2.2.3.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901084),
    /// [MQTT-3.2.2-12]).
    pub maximum_qos: Option<MaximumQoS>,
    /// Retain Available
    /// ([§3.2.2.3.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901085),
    /// [MQTT-3.2.2-13], [MQTT-3.2.2-14]).
    pub retain_available: Option<bool>,
    /// Maximum Packet Size the Server is willing to accept
    /// ([§3.2.2.3.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901086),
    /// [MQTT-3.2.2-15]).
    pub maximum_packet_size: Option<NonZero<u32>>,
    /// Client Identifier assigned by the Server when the Client sent
    /// an empty Client Identifier
    /// ([§3.2.2.3.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901087),
    /// [MQTT-3.2.2-16]).
    pub assigned_client_identifier: Option<Utf8String>,
    /// Topic Alias Maximum the Server is willing to accept
    /// ([§3.2.2.3.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901088)).
    pub topic_alias_maximum: Option<u16>,
    /// Reason String — optional human-readable diagnostic
    /// ([§3.2.2.3.9](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901089)).
    pub reason_string: Option<Utf8String>,
    /// Wildcard Subscription Available
    /// ([§3.2.2.3.11](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901091)).
    pub wildcard_subscription_available: Option<bool>,
    /// Subscription Identifiers Available
    /// ([§3.2.2.3.12](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901092)).
    pub subscription_identifiers_available: Option<bool>,
    /// Shared Subscription Available
    /// ([§3.2.2.3.13](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901093)).
    pub shared_subscription_available: Option<bool>,
    /// Server Keep Alive in seconds; overrides the client's request
    /// ([§3.2.2.3.14](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901094),
    /// [MQTT-3.2.2-21]).
    pub server_keep_alive: Option<u16>,
    /// Response Information used to construct a Response Topic
    /// ([§3.2.2.3.15](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901095),
    /// [MQTT-3.2.2-22]).
    pub response_information: Option<Utf8String>,
    /// Server Reference for UseAnotherServer / ServerMoved
    /// disconnects
    /// ([§3.2.2.3.16](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901096)).
    pub server_reference: Option<Utf8String>,
    /// Enhanced Authentication state echoed by the Server
    /// ([§3.2.2.3.17](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901097),
    /// [MQTT-3.2.2-23]).
    pub authentication: Option<AuthenticationKind>,
    /// User Properties
    /// ([§3.2.2.3.10](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901090)).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
