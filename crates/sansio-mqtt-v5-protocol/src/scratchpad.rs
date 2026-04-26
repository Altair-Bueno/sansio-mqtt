use crate::types::ConnectionOptions;
use crate::types::DriverEventOut;
use crate::types::UserWriteOut;
use alloc::collections::vec_deque::VecDeque;
use bytes::Bytes;
use bytes::BytesMut;
use core::num::NonZero;
use sansio_mqtt_v5_types::MaximumQoS;

#[derive(Debug)]
pub struct ClientScratchpad<Time>
where
    Time: 'static,
{
    pub(crate) pending_connect_options: ConnectionOptions,
    pub(crate) session_should_persist: bool,
    pub(crate) effective_client_max_bytes_string: u16,
    pub(crate) effective_client_max_bytes_binary_data: u16,
    pub(crate) effective_client_max_remaining_bytes: u64,
    pub(crate) effective_client_max_subscriptions_len: u32,
    pub(crate) effective_client_max_user_properties_len: usize,
    pub(crate) effective_client_max_subscription_identifiers_len: usize,
    pub(crate) effective_client_receive_maximum: NonZero<u16>,
    pub(crate) effective_client_maximum_packet_size: Option<NonZero<u32>>,
    pub(crate) effective_client_topic_alias_maximum: u16,
    pub(crate) effective_broker_receive_maximum: NonZero<u16>,
    pub(crate) effective_broker_maximum_packet_size: Option<NonZero<u32>>,
    pub(crate) effective_broker_topic_alias_maximum: u16,
    pub(crate) effective_broker_maximum_qos: Option<MaximumQoS>,
    pub(crate) effective_retain_available: bool,
    pub(crate) effective_wildcard_subscription_available: bool,
    pub(crate) effective_shared_subscription_available: bool,
    pub(crate) effective_subscription_identifiers_available: bool,
    pub(crate) negotiated_receive_maximum: NonZero<u16>,
    pub(crate) negotiated_maximum_packet_size: Option<NonZero<u32>>,
    pub(crate) negotiated_topic_alias_maximum: u16,
    pub(crate) negotiated_server_keep_alive: Option<u16>,
    pub(crate) negotiated_maximum_qos: Option<MaximumQoS>,
    pub(crate) negotiated_retain_available: bool,
    pub(crate) negotiated_wildcard_subscription_available: bool,
    pub(crate) negotiated_shared_subscription_available: bool,
    pub(crate) negotiated_subscription_identifiers_available: bool,
    pub(crate) keep_alive_interval_secs: Option<NonZero<u16>>,
    pub(crate) keep_alive_saw_network_activity: bool,
    pub(crate) keep_alive_ping_outstanding: bool,
    pub(crate) read_buffer: BytesMut,
    pub(crate) read_queue: VecDeque<UserWriteOut>,
    pub(crate) write_queue: VecDeque<Bytes>,
    pub(crate) action_queue: VecDeque<DriverEventOut>,
    pub(crate) next_timeout: Option<Time>,
}

impl<Time> Default for ClientScratchpad<Time>
where
    Time: 'static,
{
    fn default() -> Self {
        Self {
            pending_connect_options: ConnectionOptions::default(),
            session_should_persist: false,
            effective_client_max_bytes_string: u16::MAX,
            effective_client_max_bytes_binary_data: u16::MAX,
            effective_client_max_remaining_bytes: u64::MAX,
            effective_client_max_subscriptions_len: u32::MAX,
            effective_client_max_user_properties_len: usize::MAX,
            effective_client_max_subscription_identifiers_len: usize::MAX,
            effective_client_receive_maximum: NonZero::new(u16::MAX)
                .expect("u16::MAX is always non-zero for receive_maximum"),
            effective_client_maximum_packet_size: None,
            effective_client_topic_alias_maximum: u16::MAX,
            effective_broker_receive_maximum: NonZero::new(u16::MAX)
                .expect("u16::MAX is always non-zero for receive_maximum"),
            effective_broker_maximum_packet_size: None,
            effective_broker_topic_alias_maximum: u16::MAX,
            effective_broker_maximum_qos: None,
            effective_retain_available: true,
            effective_wildcard_subscription_available: true,
            effective_shared_subscription_available: true,
            effective_subscription_identifiers_available: true,
            negotiated_receive_maximum: NonZero::new(u16::MAX)
                .expect("u16::MAX is always non-zero for receive_maximum"),
            negotiated_maximum_packet_size: None,
            negotiated_topic_alias_maximum: 0,
            negotiated_server_keep_alive: None,
            negotiated_maximum_qos: None,
            negotiated_retain_available: true,
            negotiated_wildcard_subscription_available: true,
            negotiated_shared_subscription_available: true,
            negotiated_subscription_identifiers_available: true,
            keep_alive_interval_secs: None,
            keep_alive_saw_network_activity: false,
            keep_alive_ping_outstanding: false,
            read_buffer: BytesMut::new(),
            read_queue: VecDeque::new(),
            write_queue: VecDeque::new(),
            action_queue: VecDeque::new(),
            next_timeout: None,
        }
    }
}
