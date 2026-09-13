use alloc::collections::vec_deque::VecDeque;
use bytes::Bytes;
use bytes::BytesMut;
use core::num::NonZero;
use core::time::Duration;
use sansio_mqtt_protocol::ConnectOptions;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_v5_types::MaximumQoS;
use sansio_mqtt_v5_types::ParserSettings;

#[derive(Debug)]
pub(crate) struct ClientScratchpad<T> {
    /// The options from the most recent `Command::Connect`, retained for the
    /// whole client lifetime so a reconnect can resend CONNECT unchanged.
    ///
    /// `None` until the application issues its first `Command::Connect`
    /// (`sansio_mqtt_protocol::ConnectOptions` has no meaningful default to
    /// synthesize instead).
    pub(crate) pending_connect_options: Option<ConnectOptions>,
    pub(crate) session_should_persist: bool,
    pub(crate) effective_client_max_remaining_bytes: u64,
    pub(crate) effective_client_maximum_packet_size: Option<NonZero<u32>>,
    pub(crate) effective_client_topic_alias_maximum: u16,
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
    pub(crate) read_queue: VecDeque<Event>,
    pub(crate) write_queue: VecDeque<Bytes>,
    pub(crate) action_queue: VecDeque<DriverAction>,
    pub(crate) next_timeout: Option<T>,
}

impl<T> ClientScratchpad<T>
where
    T: Time,
{
    /// Schedules the next keep-alive deadline `secs` seconds after `from`.
    ///
    /// The only place in the crate where an instant is advanced; everything
    /// else stores and compares `T` values supplied by the driver.
    pub(crate) fn arm_keep_alive_deadline(&mut self, from: T, secs: u64) {
        self.next_timeout = Some(from + Duration::from_secs(secs));
    }
}

impl<T> Default for ClientScratchpad<T> {
    fn default() -> Self {
        Self {
            pending_connect_options: None,
            session_should_persist: false,
            effective_client_max_remaining_bytes: ParserSettings::default().max_remaining_bytes,
            effective_client_maximum_packet_size: None,
            effective_client_topic_alias_maximum: u16::MAX,
            effective_broker_maximum_qos: None,
            effective_retain_available: true,
            effective_wildcard_subscription_available: true,
            effective_shared_subscription_available: true,
            effective_subscription_identifiers_available: true,
            negotiated_receive_maximum: NonZero::<u16>::MAX,
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
