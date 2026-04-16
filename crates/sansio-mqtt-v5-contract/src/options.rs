use alloc::string::String;
use alloc::vec::Vec;
use core::time::Duration;

use sansio_mqtt_v5_types::Qos;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectOptions {
    pub connect_timeout: Duration,
    pub clean_start: bool,
    pub keep_alive: Option<Duration>,
    pub client_id: String,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            clean_start: false,
            keep_alive: None,
            client_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishRequest {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: Qos,
    pub retain: bool,
}

impl PublishRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscribeRequest {
    pub topic_filter: String,
    pub qos: Qos,
}
