use std::net::SocketAddr;
use sansio_mqtt_v5_protocol::{ClientSettings, ConnectionOptions};
use crate::backoff::Backoff;

#[derive(Clone, Debug)]
pub struct ConnectOptions {
    pub addr: SocketAddr,
    pub connection: ConnectionOptions,
    pub protocol_config: ClientSettings,
    pub max_in_queued_messages: usize,
    pub max_out_queued_messages: usize,
    pub backoff: Option<Backoff>,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:1883".parse().unwrap(),
            connection: ConnectionOptions::default(),
            protocol_config: ClientSettings::default(),
            max_in_queued_messages: 16,
            max_out_queued_messages: 16,
            backoff: None,
        }
    }
}
