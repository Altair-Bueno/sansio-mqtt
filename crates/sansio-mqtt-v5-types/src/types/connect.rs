//! MQTT v5.0 `CONNECT` packet
//! ([§3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901033)).
//!
//! The first packet sent from Client to Server after the Network
//! Connection is established. Conformance: `[MQTT-3.1.0-1]`,
//! `[MQTT-3.1.0-2]`.
use super::*;

/// MQTT v5.0 `CONNECT` packet
/// ([§3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901033)).
///
/// The first packet a Client sends to the Server; it requests the
/// creation or resumption of an MQTT session. Conformance:
/// `[MQTT-3.1.0-1]`, `[MQTT-3.1.0-2]`, `[MQTT-3.1.2-1]`,
/// `[MQTT-3.1.2-2]`, `[MQTT-3.1.2-3]`, `[MQTT-3.1.2-4]`,
/// `[MQTT-3.1.2-5]`, `[MQTT-3.1.2-6]`, `[MQTT-3.1.2-7]`,
/// `[MQTT-3.1.2-8]`, `[MQTT-3.1.2-9]`, `[MQTT-3.1.3-1]`,
/// `[MQTT-3.1.3-3]`, `[MQTT-3.1.3-4]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Connect {
    /// Protocol Name; MUST equal `"MQTT"` for MQTT v5.0
    /// ([§3.1.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901036),
    /// [MQTT-3.1.2-1]).
    pub protocol_name: Utf8String,
    /// Protocol Level; MUST be `5` for MQTT v5.0
    /// ([§3.1.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901037),
    /// [MQTT-3.1.2-2]).
    pub protocol_version: u8,
    /// Clean Start
    /// ([§3.1.2.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901039),
    /// [MQTT-3.1.2-4], [MQTT-3.1.2-5], [MQTT-3.1.2-6]).
    pub clean_start: bool,
    /// Client Identifier
    /// ([§3.1.3.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901059),
    /// [MQTT-3.1.3-3], [MQTT-3.1.3-4], [MQTT-3.1.3-5]).
    pub client_identifier: Utf8String,
    /// Will Message to publish on ungraceful disconnect, or `None`.
    /// Presence corresponds to the Will Flag
    /// ([§3.1.2.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901040),
    /// [MQTT-3.1.2-7], [MQTT-3.1.2-8], [MQTT-3.1.2-9]).
    pub will: Option<Will>,
    /// Optional User Name
    /// ([§3.1.3.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901071),
    /// [MQTT-3.1.2-17], [MQTT-3.1.2-18]).
    pub user_name: Option<Utf8String>,
    /// Optional Password
    /// ([§3.1.3.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901072),
    /// [MQTT-3.1.2-19]).
    pub password: Option<BinaryData>,
    /// Optional Keep Alive interval, in seconds; `None` disables the
    /// mechanism
    /// ([§3.1.2.10](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901045),
    /// [MQTT-3.1.2-22], [MQTT-3.1.2-23], [MQTT-3.1.2-24]).
    pub keep_alive: Option<NonZero<u16>>,
    /// `CONNECT` properties
    /// ([§3.1.2.11](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901046)).
    pub properties: ConnectProperties,
}

/// Fixed-header flags byte for `CONNECT`
/// ([§3.1.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901034)).
///
/// MUST be `0b0000` for `CONNECT`; any other value is a Malformed
/// Packet ([MQTT-3.1.1-1]).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ConnectHeaderFlags;

impl From<ConnectHeaderFlags> for u8 {
    fn from(_: ConnectHeaderFlags) -> u8 {
        0b0000_0000
    }
}

/// Will Message carried inside a [`Connect`] packet
/// ([§3.1.3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901060)).
///
/// Published by the Server on behalf of the Client if the Network
/// Connection is lost without a clean `DISCONNECT`. Conformance:
/// `[MQTT-3.1.2-7]`, `[MQTT-3.1.2-8]`, `[MQTT-3.1.2-9]`,
/// `[MQTT-3.1.2-10]`, `[MQTT-3.1.2-11]`, `[MQTT-3.1.2-12]`,
/// `[MQTT-3.1.2-13]`, `[MQTT-3.1.2-14]`, `[MQTT-3.1.2-15]`,
/// `[MQTT-3.1.2-16]`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Will {
    /// Will Topic — topic to which the Will Message is published.
    pub topic: Topic,
    /// Will Payload — opaque bytes delivered as the Will Message.
    pub payload: BinaryData,
    /// Will QoS ([MQTT-3.1.2-11], [MQTT-3.1.2-12]).
    pub qos: Qos,
    /// Will Retain flag ([MQTT-3.1.2-14], [MQTT-3.1.2-15],
    /// [MQTT-3.1.2-16]).
    pub retain: bool,
    /// Will Properties
    /// ([§3.1.3.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901060)).
    pub properties: WillProperties,
}

/// Will Properties attached to the [`Will`] payload
/// ([§3.1.3.2.1](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901061)).
///
/// All fields are optional. `None` means "property absent on the
/// wire".
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct WillProperties {
    /// Will Delay Interval in seconds
    /// ([§3.1.3.2.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901062)).
    pub will_delay_interval: Option<u32>,
    /// Payload Format Indicator
    /// ([§3.1.3.2.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901063)).
    pub payload_format_indicator: Option<FormatIndicator>,
    /// Message Expiry Interval in seconds
    /// ([§3.1.3.2.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901064)).
    pub message_expiry_interval: Option<u32>,
    /// Content Type
    /// ([§3.1.3.2.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901065)).
    pub content_type: Option<Utf8String>,
    /// Response Topic for request/response interactions
    /// ([§3.1.3.2.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901066)).
    pub response_topic: Option<Topic>,
    /// Correlation Data
    /// ([§3.1.3.2.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901067)).
    pub correlation_data: Option<BinaryData>,
    /// User Properties
    /// ([§3.1.3.2.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901068)).
    /// Order of user properties is preserved ([MQTT-3.1.3-10]).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

/// `CONNECT` Properties attached to the [`Connect`] packet
/// ([§3.1.2.11](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901046)).
///
/// All fields are optional. A `None` value indicates the property was
/// absent on the wire; the Server then applies the spec-mandated
/// default.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct ConnectProperties {
    /// Session Expiry Interval in seconds
    /// ([§3.1.2.11.2](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901048)).
    pub session_expiry_interval: Option<u32>,
    /// Receive Maximum for in-flight QoS 1/2 PUBLISH packets
    /// ([§3.1.2.11.3](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901049),
    /// [MQTT-3.1.2-23]).
    pub receive_maximum: Option<NonZero<u16>>,
    /// Maximum Packet Size the Client is willing to accept
    /// ([§3.1.2.11.4](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901050),
    /// [MQTT-3.1.2-25]).
    pub maximum_packet_size: Option<NonZero<u32>>,
    /// Topic Alias Maximum the Client is willing to accept
    /// ([§3.1.2.11.5](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901051)).
    pub topic_alias_maximum: Option<u16>,
    /// Request Response Information flag
    /// ([§3.1.2.11.6](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901052),
    /// [MQTT-3.1.2-28]).
    pub request_response_information: Option<bool>,
    /// Request Problem Information flag
    /// ([§3.1.2.11.7](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901053),
    /// [MQTT-3.1.2-29]).
    pub request_problem_information: Option<bool>,
    /// Enhanced Authentication state (method and optional data)
    /// ([§3.1.2.11.9](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901055),
    /// [§3.1.2.11.10](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901056),
    /// [MQTT-3.1.2-20], [MQTT-3.1.2-21]).
    pub authentication: Option<AuthenticationKind>,
    /// User Properties
    /// ([§3.1.2.11.8](https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html#_Toc3901054),
    /// [MQTT-3.1.2-19]).
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}
