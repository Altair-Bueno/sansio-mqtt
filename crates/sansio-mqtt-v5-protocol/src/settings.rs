//! [`ClientSettings`], the configuration of a [`Client`](crate::Client).

use core::num::NonZero;
use core::time::Duration;
use sansio_mqtt_protocol::Qos;

/// Configuration for a [`Client`](crate::Client).
///
/// Every limit here is the single source of truth: the values that end up in
/// the CONNECT packet, the caps enforced on outbound traffic, and the caps fed
/// to the inbound parser are all derived from this struct alone (there is no
/// longer a separate per-connect override).
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the four `allow_*` flags are independent public settings; restructuring them would break the public API"
)]
pub struct ClientSettings {
    /// Receive Maximum advertised in CONNECT
    /// ([MQTT-3.1.2-23]).
    pub receive_maximum: Option<NonZero<u16>>,
    /// Maximum Packet Size, counting the whole packet, that the client
    /// accepts from the server.
    ///
    /// Always advertised in CONNECT and enforced on every inbound packet, so
    /// a compliant server never sends a packet the client cannot process
    /// [MQTT-3.1.2-24]. A larger packet is answered with DISCONNECT 0x95
    /// (Packet too large). Defaults to
    /// [`DEFAULT_MAXIMUM_PACKET_SIZE`](Self::DEFAULT_MAXIMUM_PACKET_SIZE).
    #[builder(default = ClientSettings::DEFAULT_MAXIMUM_PACKET_SIZE)]
    pub maximum_packet_size: NonZero<u32>,
    /// Topic Alias Maximum advertised in CONNECT, bounding the Topic Aliases
    /// the client will accept from the server.
    pub topic_alias_maximum: Option<u16>,
    /// Request Response Information flag sent in CONNECT.
    pub request_response_information: Option<bool>,
    /// Request Problem Information flag sent in CONNECT.
    pub request_problem_information: Option<bool>,
    /// Keep Alive, in seconds, sent in every CONNECT and enforced by the
    /// client for the whole connection.
    ///
    /// Keep-alive is what detects a connection that died without being
    /// closed (lost network, idle NAT entry dropped, crashed peer): the client
    /// pings when idle and gives up when no answer arrives [MQTT-3.1.2-22].
    /// The server may still impose its own value in CONNACK. Defaults to
    /// [`DEFAULT_KEEP_ALIVE`](Self::DEFAULT_KEEP_ALIVE); `None` turns
    /// keep-alive off (sent as 0), which leaves such dead connections
    /// undetected.
    #[builder(required, default = Some(ClientSettings::DEFAULT_KEEP_ALIVE))]
    pub keep_alive: Option<NonZero<u16>>,
    /// Local cap on the `QoS` of outbound PUBLISH packets.
    ///
    /// `Some(Qos::ExactlyOnce)` and `None` both mean "no local cap"; the
    /// server-advertised Maximum `QoS` is still enforced regardless.
    pub max_outgoing_qos: Option<Qos>,
    /// Whether outbound PUBLISH may set the RETAIN flag.
    #[builder(default = true)]
    pub allow_retain: bool,
    /// Whether outbound SUBSCRIBE may use wildcard Topic Filters.
    #[builder(default = true)]
    pub allow_wildcard_subscriptions: bool,
    /// Whether outbound SUBSCRIBE may use `$share/` Topic Filters.
    #[builder(default = true)]
    pub allow_shared_subscriptions: bool,
    /// Whether outbound SUBSCRIBE may carry a Subscription Identifier.
    #[builder(default = true)]
    pub allow_subscription_identifiers: bool,
    /// Maximum number of User Property entries accepted in any inbound
    /// property section.
    #[builder(default = 32)]
    pub max_user_properties: usize,
    /// Maximum number of Subscription Identifiers accepted in a single
    /// inbound PUBLISH.
    #[builder(default = 32)]
    pub max_subscription_identifiers: usize,
    /// Maximum number of Topic Filters accepted in a single inbound
    /// SUBSCRIBE.
    #[builder(default = 32)]
    pub max_subscriptions: u32,
    /// How long a connection attempt may take, from the
    /// [`Command::Connect`](sansio_mqtt_protocol::Command::Connect) that
    /// starts it until CONNACK arrives.
    ///
    /// The deadline is measured from the instant the `Connect` command is
    /// stamped with and covers both opening the socket (including any TLS
    /// handshake the driver performs) and waiting for CONNACK; it is not
    /// restarted when the socket connects. When it elapses the client gives
    /// up on the attempt: it withdraws or cancels the socket request, or
    /// closes the socket without sending DISCONNECT, and reports
    /// [`Event::Disconnected(None)`](sansio_mqtt_protocol::Event::Disconnected).
    /// "If the Client does not receive a CONNACK packet from the Server within
    /// a reasonable amount of time, the Client SHOULD close the Network
    /// Connection" (MQTT v5.0 §3.2). Once CONNACK arrives, the keep-alive
    /// deadline (if any) takes its place. Defaults to
    /// [`DEFAULT_CONNECT_TIMEOUT`](Self::DEFAULT_CONNECT_TIMEOUT).
    #[builder(default = ClientSettings::DEFAULT_CONNECT_TIMEOUT)]
    pub connect_timeout: Duration,
}

impl ClientSettings {
    /// Default [`maximum_packet_size`](Self::maximum_packet_size): 1 MiB.
    pub const DEFAULT_MAXIMUM_PACKET_SIZE: NonZero<u32> = NonZero::new(1024 * 1024).unwrap();
    /// Default [`connect_timeout`](Self::connect_timeout): 30 seconds.
    pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
    /// Default [`keep_alive`](Self::keep_alive): 60 seconds.
    pub const DEFAULT_KEEP_ALIVE: NonZero<u16> = NonZero::new(60).unwrap();
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self::builder().build()
    }
}
