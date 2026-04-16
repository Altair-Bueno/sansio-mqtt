use heapless::{String, Vec};

use crate::error::OptionsError;

pub const CLIENT_ID_CAPACITY: usize = 128;
pub const TOPIC_CAPACITY: usize = 256;
pub const PAYLOAD_CAPACITY: usize = 1024;
pub const TOPIC_FILTER_CAPACITY: usize = 256;
pub const SUBACK_REASON_CODES_CAPACITY: usize = 8;
pub const SESSION_ACTION_TOPIC_CAPACITY: usize = 256;
pub const SESSION_ACTION_PAYLOAD_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Qos {
    #[default]
    AtMost,
    AtLeast,
    Exactly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectOptions {
    pub connect_timeout_ms: u32,
    pub clean_start: bool,
    pub keep_alive_secs: Option<u16>,
    pub client_id: String<CLIENT_ID_CAPACITY>,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 10_000,
            clean_start: false,
            keep_alive_secs: None,
            client_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishRequest {
    pub topic: String<TOPIC_CAPACITY>,
    pub payload: Vec<u8, PAYLOAD_CAPACITY>,
    pub qos: Qos,
    pub retain: bool,
}

impl PublishRequest {
    pub fn qos1(mut self) -> Self {
        self.qos = Qos::AtLeast;
        self
    }

    pub fn qos2(mut self) -> Self {
        self.qos = Qos::Exactly;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscribeRequest {
    pub topic_filter: String<TOPIC_FILTER_CAPACITY>,
    pub qos: Qos,
}

impl SubscribeRequest {
    pub fn single(topic_filter: &str) -> Result<Self, OptionsError> {
        let mut value = Self::default();
        value
            .topic_filter
            .push_str(topic_filter)
            .map_err(|_| OptionsError::TopicFilterTooLong)?;
        Ok(value)
    }
}
