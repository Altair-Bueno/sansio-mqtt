use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;

use sansio_mqtt_v5_protocol::ClientSettings;
use sansio_mqtt_v5_protocol::ConnectionOptions;

use crate::backoff::Backoff;

#[derive(Clone, Debug)]
pub struct ConnectOptions {
    pub addr: SocketAddr,
    pub connection: ConnectionOptions,
    pub protocol_config: ClientSettings,
    /// Maximum number of concurrent inbound QoS 1/2 messages (in-flight).
    ///
    /// This value is applied as the MQTT `Receive Maximum` property in the
    /// `CONNECT` packet, capped to `u16::MAX`.  A value of `0` means the broker
    /// is not permitted to send any QoS 1/2 `PUBLISH` packets, which will
    /// effectively stall all inbound delivery.
    pub max_in_queued_messages: usize,
    /// Maximum number of concurrent outbound QoS 1/2 messages (in-flight).
    ///
    /// Once the count of unacknowledged outbound messages reaches this limit,
    /// [`Connection::publish`] returns
    /// [`ConnectionError::QueueFull`](crate::error::ConnectionError::QueueFull).
    /// A value of `0` means every QoS 1/2 publish is rejected immediately.
    pub max_out_queued_messages: usize,
    pub backoff: Option<Backoff>,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1883),
            connection: ConnectionOptions::default(),
            protocol_config: ClientSettings::default(),
            max_in_queued_messages: 16,
            max_out_queued_messages: 16,
            backoff: None,
        }
    }
}
