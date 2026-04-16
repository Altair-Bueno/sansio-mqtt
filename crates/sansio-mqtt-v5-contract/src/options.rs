use alloc::vec::Vec;
use core::num::NonZero;
use core::time::Duration;

use sansio_mqtt_v5_types::{
    BinaryData, FormatIndicator, Payload, Qos, RetainHandling, Topic, Utf8String,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectOptions {
    pub connect_timeout: Duration,
    pub clean_start: bool,
    pub keep_alive: Option<Duration>,
    pub client_id: Utf8String,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            clean_start: false,
            keep_alive: None,
            client_id: Utf8String::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishRequestProperties {
    pub payload_format_indicator: Option<FormatIndicator>,
    pub message_expiry_interval: Option<u32>,
    pub response_topic: Option<Topic>,
    pub correlation_data: Option<BinaryData>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
    pub content_type: Option<Utf8String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishRequest {
    pub topic: Topic,
    pub payload: Payload,
    pub qos: Qos,
    pub retain: bool,
    pub properties: PublishRequestProperties,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscribeRequestProperties {
    pub subscription_identifier: Option<NonZero<u64>>,
    pub user_properties: Vec<(Utf8String, Utf8String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscribeRequest {
    pub topic_filter: Utf8String,
    pub qos: Qos,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: RetainHandling,
    pub properties: SubscribeRequestProperties,
}
