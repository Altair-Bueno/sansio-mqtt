use core::num::NonZero;
use core::time::Duration;

use crate::types::*;
use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;
use bytes::Bytes;
use bytes::BytesMut;
use encode::Encodable;
use sansio::Protocol;
use sansio_mqtt_v5_types::BinaryData;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::Connect;
use sansio_mqtt_v5_types::ConnectProperties;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::Disconnect;
use sansio_mqtt_v5_types::DisconnectProperties;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::EncodeError;
use sansio_mqtt_v5_types::GuaranteedQoS;
use sansio_mqtt_v5_types::MaximumQoS;
use sansio_mqtt_v5_types::ParserSettings;
use sansio_mqtt_v5_types::PingReq;
use sansio_mqtt_v5_types::PubAck;
use sansio_mqtt_v5_types::PubAckProperties;
use sansio_mqtt_v5_types::PubAckReasonCode;
use sansio_mqtt_v5_types::PubComp;
use sansio_mqtt_v5_types::PubCompProperties;
use sansio_mqtt_v5_types::PubCompReasonCode;
use sansio_mqtt_v5_types::PubRec;
use sansio_mqtt_v5_types::PubRecProperties;
use sansio_mqtt_v5_types::PubRecReasonCode;
use sansio_mqtt_v5_types::PubRel;
use sansio_mqtt_v5_types::PubRelProperties;
use sansio_mqtt_v5_types::PubRelReasonCode;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::PublishProperties;
use sansio_mqtt_v5_types::Qos;
use sansio_mqtt_v5_types::Subscribe;
use sansio_mqtt_v5_types::SubscribeProperties;
use sansio_mqtt_v5_types::Topic;
use sansio_mqtt_v5_types::Unsubscribe;
use sansio_mqtt_v5_types::UnsubscribeProperties;
use sansio_mqtt_v5_types::Utf8String;
use sansio_mqtt_v5_types::Will as ConnectWill;
use sansio_mqtt_v5_types::WillProperties;
use winnow::error::ErrMode;
use winnow::stream::Partial;
use winnow::Parser;

#[derive(Debug, PartialEq, Default)]
enum ClientLifecycleState {
    #[default]
    Start,
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ConnectingPhase {
    AwaitConnAck,
    AuthInProgress,
}

#[derive(Debug, Clone, PartialEq)]
enum OutboundInflightState {
    Qos1AwaitPubAck { publish: Publish },
    Qos2AwaitPubRec { publish: Publish },
    Qos2AwaitPubComp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InboundInflightState {
    Qos1AwaitAppDecision,
    Qos2AwaitAppDecision,
    Qos2AwaitPubRel,
    Qos2Rejected(PubRecReasonCode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientSession {
    on_flight_sent: BTreeMap<NonZero<u16>, OutboundInflightState>,
    on_flight_received: BTreeMap<NonZero<u16>, InboundInflightState>,
    pending_subscribe: BTreeMap<NonZero<u16>, ()>,
    pending_unsubscribe: BTreeMap<NonZero<u16>, ()>,
    inbound_topic_aliases: BTreeMap<NonZero<u16>, Topic>,
    next_packet_id: u16,
}

impl ClientSession {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

impl Default for ClientSession {
    fn default() -> Self {
        Self {
            on_flight_sent: BTreeMap::new(),
            on_flight_received: BTreeMap::new(),
            pending_subscribe: BTreeMap::new(),
            pending_unsubscribe: BTreeMap::new(),
            inbound_topic_aliases: BTreeMap::new(),
            next_packet_id: 1,
        }
    }
}

#[derive(Debug)]
pub struct ClientScratchpad<Time>
where
    Time: 'static,
{
    lifecycle_state: ClientLifecycleState,
    connecting_phase: ConnectingPhase,
    pending_connect_options: ConnectionOptions,
    session_should_persist: bool,
    effective_client_max_bytes_string: u16,
    effective_client_max_bytes_binary_data: u16,
    effective_client_max_remaining_bytes: u64,
    effective_client_max_subscriptions_len: u32,
    effective_client_max_user_properties_len: usize,
    effective_client_max_subscription_identifiers_len: usize,
    effective_client_receive_maximum: NonZero<u16>,
    effective_client_maximum_packet_size: Option<NonZero<u32>>,
    effective_client_topic_alias_maximum: u16,
    effective_broker_receive_maximum: NonZero<u16>,
    effective_broker_maximum_packet_size: Option<NonZero<u32>>,
    effective_broker_topic_alias_maximum: u16,
    effective_broker_maximum_qos: Option<MaximumQoS>,
    effective_retain_available: bool,
    effective_wildcard_subscription_available: bool,
    effective_shared_subscription_available: bool,
    effective_subscription_identifiers_available: bool,
    negotiated_receive_maximum: NonZero<u16>,
    negotiated_maximum_packet_size: Option<NonZero<u32>>,
    negotiated_topic_alias_maximum: u16,
    negotiated_server_keep_alive: Option<u16>,
    negotiated_maximum_qos: Option<MaximumQoS>,
    negotiated_retain_available: bool,
    negotiated_wildcard_subscription_available: bool,
    negotiated_shared_subscription_available: bool,
    negotiated_subscription_identifiers_available: bool,
    keep_alive_interval_secs: Option<NonZero<u16>>,
    keep_alive_saw_network_activity: bool,
    keep_alive_ping_outstanding: bool,
    read_buffer: BytesMut,
    read_queue: VecDeque<UserWriteOut>,
    write_queue: VecDeque<Bytes>,
    action_queue: VecDeque<DriverEventOut>,
    next_timeout: Option<Time>,
}

impl<Time> Default for ClientScratchpad<Time>
where
    Time: 'static,
{
    fn default() -> Self {
        Self {
            lifecycle_state: ClientLifecycleState::default(),
            connecting_phase: ConnectingPhase::AwaitConnAck,
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

#[derive(Debug)]
pub struct Client<Time>
where
    Time: 'static,
{
    settings: ClientSettings,
    session: ClientSession,
    scratchpad: ClientScratchpad<Time>,
}

impl<Time> Default for Client<Time> {
    fn default() -> Self {
        Self::with_settings(Default::default())
    }
}

impl<Time> Client<Time> {
    fn min_option_nonzero_u16(
        a: Option<NonZero<u16>>,
        b: Option<NonZero<u16>>,
    ) -> Option<NonZero<u16>> {
        match (a, b) {
            (Some(a), Some(b)) => Some(if a.get() <= b.get() { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    fn min_option_nonzero_u32(
        a: Option<NonZero<u32>>,
        b: Option<NonZero<u32>>,
    ) -> Option<NonZero<u32>> {
        match (a, b) {
            (Some(a), Some(b)) => Some(if a.get() <= b.get() { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    pub fn with_settings_and_session(settings: ClientSettings, session: ClientSession) -> Self {
        let mut client = Self {
            settings,
            session,
            scratchpad: ClientScratchpad::default(),
        };
        client.recompute_effective_limits();
        client
    }

    pub fn with_settings(settings: ClientSettings) -> Self {
        Self::with_settings_and_session(settings, Default::default())
    }

    fn encode_control_packet(packet: &ControlPacket) -> Result<Bytes, Error> {
        let mut encoded = Vec::new();

        packet.encode(&mut encoded).map_err(|err| match err {
            EncodeError::PacketTooLarge(_) => Error::PacketTooLarge,
            _ => Error::EncodeFailure,
        })?;

        Ok(Bytes::from(encoded))
    }

    fn enqueue_packet(&mut self, packet: ControlPacket) -> Result<(), Error> {
        let encoded = Self::encode_control_packet(&packet)?;
        self.validate_outbound_packet_size(encoded.len())?;
        self.scratchpad.write_queue.push_back(encoded);
        self.scratchpad.keep_alive_saw_network_activity = true;
        Ok(())
    }

    fn build_connect_packet(&self, options: &ConnectionOptions) -> Result<Connect, Error> {
        let will = options
            .will
            .as_ref()
            .map(|will| {
                let payload =
                    BinaryData::try_new(will.payload.clone()).map_err(|_| Error::ProtocolError)?;
                let message_expiry_interval = will
                    .message_expiry_interval
                    .map(|interval| {
                        u32::try_from(interval.as_secs()).map_err(|_| Error::ProtocolError)
                    })
                    .transpose()?;

                Ok(ConnectWill {
                    topic: will.topic.clone(),
                    payload,
                    qos: will.qos,
                    retain: will.retain,
                    properties: WillProperties {
                        will_delay_interval: will.will_delay_interval,
                        payload_format_indicator: will.payload_format_indicator,
                        message_expiry_interval,
                        content_type: will.content_type.clone(),
                        response_topic: will.response_topic.clone(),
                        correlation_data: will.correlation_data.clone(),
                        user_properties: will.user_properties.clone(),
                    },
                })
            })
            .transpose()?;

        Ok(Connect {
            protocol_name: Utf8String::try_from("MQTT")
                .expect("MQTT protocol name is always valid UTF-8 string"),
            protocol_version: 5,
            clean_start: options.clean_start,
            client_identifier: options.client_identifier.clone(),
            will,
            user_name: options.user_name.clone(),
            password: options.password.clone(),
            keep_alive: options.keep_alive.or(self.settings.default_keep_alive),
            properties: ConnectProperties {
                session_expiry_interval: options.session_expiry_interval,
                receive_maximum: Self::min_option_nonzero_u16(
                    options.receive_maximum,
                    self.settings.max_incoming_receive_maximum,
                ),
                maximum_packet_size: Self::min_option_nonzero_u32(
                    options.maximum_packet_size,
                    self.settings.max_incoming_packet_size,
                ),
                topic_alias_maximum: options
                    .topic_alias_maximum
                    .or(self.settings.max_incoming_topic_alias_maximum)
                    .map(|topic_alias_maximum| {
                        topic_alias_maximum.min(
                            self.settings
                                .max_incoming_topic_alias_maximum
                                .unwrap_or(u16::MAX),
                        )
                    }),
                request_response_information: options
                    .request_response_information
                    .or(self.settings.default_request_response_information),
                request_problem_information: options
                    .request_problem_information
                    .or(self.settings.default_request_problem_information),
                authentication: options.authentication.clone(),
                user_properties: options.user_properties.clone(),
            },
        })
    }

    fn parser_settings(&self) -> ParserSettings {
        ParserSettings {
            max_bytes_string: self.scratchpad.effective_client_max_bytes_string,
            max_bytes_binary_data: self.scratchpad.effective_client_max_bytes_binary_data,
            max_remaining_bytes: self.scratchpad.effective_client_max_remaining_bytes,
            max_subscriptions_len: self.scratchpad.effective_client_max_subscriptions_len,
            max_user_properties_len: self.scratchpad.effective_client_max_user_properties_len,
            max_subscription_identifiers_len: self
                .scratchpad
                .effective_client_max_subscription_identifiers_len,
        }
    }

    fn recompute_effective_limits(&mut self) {
        fn min_option_maximum_qos(
            a: Option<MaximumQoS>,
            b: Option<MaximumQoS>,
        ) -> Option<MaximumQoS> {
            match (a, b) {
                (Some(MaximumQoS::AtMostOnce), _) | (_, Some(MaximumQoS::AtMostOnce)) => {
                    Some(MaximumQoS::AtMostOnce)
                }
                (Some(MaximumQoS::AtLeastOnce), Some(MaximumQoS::AtLeastOnce)) => {
                    Some(MaximumQoS::AtLeastOnce)
                }
                (Some(x), None) | (None, Some(x)) => Some(x),
                (None, None) => None,
            }
        }

        self.scratchpad.effective_client_max_bytes_string = self.settings.max_bytes_string;
        self.scratchpad.effective_client_max_bytes_binary_data =
            self.settings.max_bytes_binary_data;
        self.scratchpad.effective_client_max_remaining_bytes =
            self.settings.max_remaining_bytes.min(
                self.scratchpad
                    .effective_client_maximum_packet_size
                    .map(|x| u64::from(x.get()))
                    .unwrap_or(u64::MAX),
            );
        self.scratchpad.effective_client_max_subscriptions_len =
            self.settings.max_subscriptions_len;
        self.scratchpad.effective_client_max_user_properties_len =
            self.settings.max_user_properties_len;
        self.scratchpad
            .effective_client_max_subscription_identifiers_len =
            self.settings.max_subscription_identifiers_len;

        self.scratchpad.effective_client_receive_maximum = Self::min_option_nonzero_u16(
            self.settings.max_incoming_receive_maximum,
            self.scratchpad.pending_connect_options.receive_maximum,
        )
        .unwrap_or(NonZero::new(u16::MAX).expect("u16::MAX is always non-zero"));

        self.scratchpad.effective_client_maximum_packet_size = Self::min_option_nonzero_u32(
            self.settings.max_incoming_packet_size,
            self.scratchpad.pending_connect_options.maximum_packet_size,
        );

        self.scratchpad.effective_client_topic_alias_maximum = self
            .settings
            .max_incoming_topic_alias_maximum
            .unwrap_or(u16::MAX)
            .min(
                self.scratchpad
                    .pending_connect_options
                    .topic_alias_maximum
                    .or(self.settings.max_incoming_topic_alias_maximum)
                    .unwrap_or(0),
            );

        self.scratchpad.effective_broker_receive_maximum =
            self.scratchpad.negotiated_receive_maximum;
        self.scratchpad.effective_broker_maximum_packet_size =
            self.scratchpad.negotiated_maximum_packet_size;
        self.scratchpad.effective_broker_topic_alias_maximum =
            self.scratchpad.negotiated_topic_alias_maximum;
        self.scratchpad.effective_broker_maximum_qos = min_option_maximum_qos(
            self.settings.max_outgoing_qos,
            self.scratchpad.negotiated_maximum_qos,
        );
        self.scratchpad.effective_retain_available =
            self.settings.allow_retain && self.scratchpad.negotiated_retain_available;
        self.scratchpad.effective_wildcard_subscription_available =
            self.settings.allow_wildcard_subscriptions
                && self.scratchpad.negotiated_wildcard_subscription_available;
        self.scratchpad.effective_shared_subscription_available =
            self.settings.allow_shared_subscriptions
                && self.scratchpad.negotiated_shared_subscription_available;
        self.scratchpad.effective_subscription_identifiers_available =
            self.settings.allow_subscription_identifiers
                && self
                    .scratchpad
                    .negotiated_subscription_identifiers_available;
    }

    fn next_packet_id(&mut self) -> NonZero<u16> {
        let packet_id = self.session.next_packet_id;
        self.session.next_packet_id = if packet_id == u16::MAX {
            1
        } else {
            packet_id + 1
        };

        NonZero::new(packet_id).expect("packet identifier is always non-zero")
    }

    fn next_outbound_publish_packet_id(&mut self) -> Result<NonZero<u16>, Error> {
        for _ in 0..u16::MAX {
            let packet_id = self.next_packet_id();
            if !self.session.on_flight_sent.contains_key(&packet_id)
                && !self.session.pending_subscribe.contains_key(&packet_id)
                && !self.session.pending_unsubscribe.contains_key(&packet_id)
            {
                return Ok(packet_id);
            }
        }

        Err(Error::ReceiveMaximumExceeded)
    }

    fn ensure_outbound_receive_maximum_capacity(&self) -> Result<(), Error> {
        // [MQTT-4.9.0-2] [MQTT-4.9.0-3] Sender enforces peer Receive Maximum by limiting concurrent QoS>0 in-flight PUBLISH packets.
        if self.session.on_flight_sent.len()
            >= usize::from(self.scratchpad.effective_broker_receive_maximum.get())
        {
            return Err(Error::ReceiveMaximumExceeded);
        }

        Ok(())
    }

    fn validate_outbound_topic_alias(
        &self,
        topic_alias: Option<NonZero<u16>>,
    ) -> Result<(), Error> {
        if let Some(alias) = topic_alias {
            let topic_alias_maximum = self.scratchpad.effective_broker_topic_alias_maximum;
            if topic_alias_maximum == 0 || alias.get() > topic_alias_maximum {
                return Err(Error::ProtocolError);
            }
        }

        Ok(())
    }

    fn validate_outbound_packet_size(&self, packet_size_bytes: usize) -> Result<(), Error> {
        if let Some(maximum_packet_size) = self.scratchpad.effective_broker_maximum_packet_size {
            if packet_size_bytes > maximum_packet_size.get() as usize {
                return Err(Error::PacketTooLarge);
            }
        }

        Ok(())
    }

    fn validate_outbound_publish_capabilities(&self, msg: &ClientMessage) -> Result<(), Error> {
        if let Some(maximum_qos) = self.scratchpad.effective_broker_maximum_qos {
            let exceeds = match maximum_qos {
                MaximumQoS::AtMostOnce => !matches!(msg.qos, Qos::AtMostOnce),
                MaximumQoS::AtLeastOnce => matches!(msg.qos, Qos::ExactlyOnce),
            };

            if exceeds {
                return Err(Error::ProtocolError);
            }
        }

        if msg.retain && !self.scratchpad.effective_retain_available {
            return Err(Error::ProtocolError);
        }

        Ok(())
    }

    fn reset_negotiated_limits(&mut self) {
        self.scratchpad.negotiated_receive_maximum =
            NonZero::new(u16::MAX).expect("u16::MAX is always non-zero for receive_maximum");
        self.scratchpad.negotiated_maximum_packet_size = None;
        self.scratchpad.negotiated_topic_alias_maximum = 0;
        self.scratchpad.negotiated_server_keep_alive = None;
        self.scratchpad.negotiated_maximum_qos = None;
        self.scratchpad.negotiated_retain_available = true;
        self.scratchpad.negotiated_wildcard_subscription_available = true;
        self.scratchpad.negotiated_shared_subscription_available = true;
        self.scratchpad
            .negotiated_subscription_identifiers_available = true;
        self.session.inbound_topic_aliases.clear();
        self.recompute_effective_limits();
    }

    fn apply_inbound_publish_topic_alias(&mut self, publish: &mut Publish) -> Result<(), Error> {
        let topic: &str = publish.topic.as_ref().as_ref();
        if topic.is_empty() && publish.properties.topic_alias.is_none() {
            return Err(Error::ProtocolError);
        }

        let Some(topic_alias) = publish.properties.topic_alias else {
            return Ok(());
        };

        let topic_alias_maximum = self.scratchpad.effective_client_topic_alias_maximum;
        if topic_alias.get() > topic_alias_maximum {
            return Err(Error::ProtocolError);
        }

        if topic.is_empty() {
            publish.topic = self
                .session
                .inbound_topic_aliases
                .get(&topic_alias)
                .cloned()
                .ok_or(Error::ProtocolError)?;
        } else {
            self.session
                .inbound_topic_aliases
                .insert(topic_alias, publish.topic.clone());
        }

        Ok(())
    }

    fn reset_inflight_transactions(&mut self) {
        self.session.on_flight_sent.clear();
        self.session.on_flight_received.clear();
    }

    fn clear_pending_subscriptions(&mut self) {
        self.session.pending_subscribe.clear();
        self.session.pending_unsubscribe.clear();
    }

    fn reset_session_state(&mut self) {
        self.reset_inflight_transactions();
        self.clear_pending_subscriptions();
    }

    fn maybe_reset_session_state(&mut self) {
        // [MQTT-3.1.2-4] Clean Start controls whether prior session state is discarded.
        if !self.scratchpad.session_should_persist {
            self.reset_session_state();
        }
    }

    fn reset_keepalive(&mut self) {
        // [MQTT-3.1.2-22] [MQTT-3.1.2-23] Keep Alive tracking resets on connection lifecycle boundaries.
        self.scratchpad.keep_alive_interval_secs = None;
        self.scratchpad.keep_alive_saw_network_activity = false;
        self.scratchpad.keep_alive_ping_outstanding = false;
        self.scratchpad.next_timeout = None;
    }

    fn next_packet_id_checked(&mut self) -> Result<NonZero<u16>, Error> {
        // [MQTT-2.2.1-2] Packet Identifier MUST be unused while an exchange is in-flight.
        for _ in 0..u16::MAX {
            let packet_id = self.next_packet_id();
            if !self.session.on_flight_sent.contains_key(&packet_id)
                && !self.session.pending_subscribe.contains_key(&packet_id)
                && !self.session.pending_unsubscribe.contains_key(&packet_id)
            {
                return Ok(packet_id);
            }
        }

        Err(Error::ReceiveMaximumExceeded)
    }

    fn replay_outbound_inflight_with_dup(&mut self) -> Result<(), Error> {
        // [MQTT-4.4.0-1] [MQTT-4.4.0-2] On session resume, retransmit unacknowledged QoS1/QoS2 PUBLISH with DUP=1.
        for (packet_id, state) in self.session.on_flight_sent.clone() {
            let publish = match state {
                OutboundInflightState::Qos1AwaitPubAck { mut publish }
                | OutboundInflightState::Qos2AwaitPubRec { mut publish } => {
                    if let PublishKind::Repetible { dup, .. } = &mut publish.kind {
                        *dup = true;
                    }
                    publish
                }
                OutboundInflightState::Qos2AwaitPubComp => {
                    self.enqueue_packet(ControlPacket::PubRel(PubRel {
                        packet_id,
                        reason_code: PubRelReasonCode::Success,
                        properties: PubRelProperties::default(),
                    }))?;
                    continue;
                }
            };

            self.enqueue_packet(ControlPacket::Publish(publish.clone()))?;

            match self.session.on_flight_sent.get_mut(&packet_id) {
                Some(OutboundInflightState::Qos1AwaitPubAck {
                    publish: stored_publish,
                })
                | Some(OutboundInflightState::Qos2AwaitPubRec {
                    publish: stored_publish,
                }) => {
                    *stored_publish = publish;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn emit_publish_dropped_for_all_inflight(&mut self) {
        for packet_id in self.session.on_flight_sent.keys().copied() {
            self.scratchpad.read_queue.push_back(
                UserWriteOut::PublishDroppedDueToSessionNotResumed(packet_id),
            );
        }
    }

    fn fail_protocol_and_disconnect(&mut self, reason: DisconnectReasonCode) -> Result<(), Error> {
        // [MQTT-4.13.1-1] Protocol violations and malformed frames force DISCONNECT and connection close.
        let _ = self.enqueue_packet(ControlPacket::Disconnect(Disconnect {
            reason_code: reason,
            properties: DisconnectProperties::default(),
        }));

        self.scratchpad
            .action_queue
            .push_back(DriverEventOut::CloseSocket);
        self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
        self.scratchpad.read_buffer.clear();
        self.reset_keepalive();
        self.reset_negotiated_limits();
        self.maybe_reset_session_state();

        Ok(())
    }

    fn enqueue_pubrel_or_fail_protocol(&mut self, packet_id: NonZero<u16>) -> Result<(), Error> {
        match self.enqueue_packet(ControlPacket::PubRel(PubRel {
            packet_id,
            reason_code: PubRelReasonCode::Success,
            properties: PubRelProperties::default(),
        })) {
            Ok(()) => Ok(()),
            Err(_) => {
                self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                Err(Error::ProtocolError)
            }
        }
    }

    fn enqueue_puback_or_fail_protocol(
        &mut self,
        packet_id: NonZero<u16>,
        reason_code: PubAckReasonCode,
    ) -> Result<(), Error> {
        match self.enqueue_packet(ControlPacket::PubAck(PubAck {
            packet_id,
            reason_code,
            properties: PubAckProperties::default(),
        })) {
            Ok(()) => Ok(()),
            Err(_) => {
                self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                Err(Error::ProtocolError)
            }
        }
    }

    fn enqueue_pubrec_or_fail_protocol(
        &mut self,
        packet_id: NonZero<u16>,
        reason_code: PubRecReasonCode,
    ) -> Result<(), Error> {
        match self.enqueue_packet(ControlPacket::PubRec(PubRec {
            packet_id,
            reason_code,
            properties: PubRecProperties::default(),
        })) {
            Ok(()) => Ok(()),
            Err(_) => {
                self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                Err(Error::ProtocolError)
            }
        }
    }

    fn enqueue_pubcomp_or_fail_protocol(
        &mut self,
        packet_id: NonZero<u16>,
        reason_code: PubCompReasonCode,
    ) -> Result<(), Error> {
        match self.enqueue_packet(ControlPacket::PubComp(PubComp {
            packet_id,
            reason_code,
            properties: PubCompProperties::default(),
        })) {
            Ok(()) => Ok(()),
            Err(_) => {
                self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                Err(Error::ProtocolError)
            }
        }
    }

    fn handle_read_control_packet(&mut self, packet: ControlPacket) -> Result<(), Error> {
        match self.scratchpad.lifecycle_state {
            ClientLifecycleState::Connecting => match packet {
                ControlPacket::ConnAck(connack) => {
                    if matches!(
                        connack.kind,
                        sansio_mqtt_v5_types::ConnAckKind::ResumePreviousSession
                            | sansio_mqtt_v5_types::ConnAckKind::Other {
                                reason_code: ConnackReasonCode::Success
                            }
                    ) {
                        self.scratchpad.negotiated_receive_maximum =
                            connack.properties.receive_maximum.unwrap_or(
                                NonZero::new(u16::MAX).expect("u16::MAX is always non-zero"),
                            );
                        self.scratchpad.negotiated_maximum_packet_size =
                            connack.properties.maximum_packet_size;
                        self.scratchpad.negotiated_topic_alias_maximum =
                            connack.properties.topic_alias_maximum.unwrap_or(0);
                        self.scratchpad.negotiated_server_keep_alive =
                            connack.properties.server_keep_alive;
                        self.scratchpad.negotiated_maximum_qos = connack.properties.maximum_qos;
                        self.scratchpad.negotiated_retain_available =
                            connack.properties.retain_available.unwrap_or(true);
                        self.scratchpad.negotiated_wildcard_subscription_available = connack
                            .properties
                            .wildcard_subscription_available
                            .unwrap_or(true);
                        self.scratchpad
                            .negotiated_subscription_identifiers_available = connack
                            .properties
                            .subscription_identifiers_available
                            .unwrap_or(true);
                        self.scratchpad.negotiated_shared_subscription_available = connack
                            .properties
                            .shared_subscription_available
                            .unwrap_or(true);
                        self.recompute_effective_limits();
                        self.scratchpad.keep_alive_interval_secs =
                            match self.scratchpad.negotiated_server_keep_alive {
                                Some(server_keep_alive) => NonZero::new(server_keep_alive),
                                None => self.scratchpad.pending_connect_options.keep_alive,
                            };
                        self.scratchpad.keep_alive_saw_network_activity = false;
                        self.scratchpad.keep_alive_ping_outstanding = false;

                        let mut connected_emitted = false;

                        match connack.kind {
                            sansio_mqtt_v5_types::ConnAckKind::ResumePreviousSession => {
                                // [MQTT-3.2.2-2] Session Present=1 is only valid when CONNECT had Clean Start=0.
                                if self.scratchpad.pending_connect_options.clean_start {
                                    self.fail_protocol_and_disconnect(
                                        DisconnectReasonCode::ProtocolError,
                                    )?;
                                    return Err(Error::ProtocolError);
                                }
                                // [MQTT-4.4.0-1] [MQTT-4.4.0-2] Session Present=1 resumes in-flight QoS transactions and replay path.
                                if self.replay_outbound_inflight_with_dup().is_err() {
                                    self.fail_protocol_and_disconnect(
                                        DisconnectReasonCode::ProtocolError,
                                    )?;
                                    return Err(Error::ProtocolError);
                                }
                            }
                            sansio_mqtt_v5_types::ConnAckKind::Other {
                                reason_code: ConnackReasonCode::Success,
                            } => {
                                self.scratchpad
                                    .read_queue
                                    .push_back(UserWriteOut::Connected);
                                connected_emitted = true;
                                self.emit_publish_dropped_for_all_inflight();
                                self.reset_session_state();
                            }
                            _ => unreachable!("successful CONNACK kind already matched"),
                        }

                        self.scratchpad.lifecycle_state = ClientLifecycleState::Connected;
                        if !connected_emitted {
                            self.scratchpad
                                .read_queue
                                .push_back(UserWriteOut::Connected);
                        }

                        Ok(())
                    } else {
                        self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                        self.reset_negotiated_limits();
                        self.scratchpad
                            .action_queue
                            .push_back(DriverEventOut::CloseSocket);
                        Err(Error::ProtocolError)
                    }
                }
                ControlPacket::Auth(auth) => {
                    if self
                        .scratchpad
                        .pending_connect_options
                        .authentication
                        .is_none()
                    {
                        self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                        return Err(Error::ProtocolError);
                    }

                    if !matches!(
                        auth.reason_code,
                        sansio_mqtt_v5_types::AuthReasonCode::ContinueAuthentication
                    ) {
                        self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                        return Err(Error::ProtocolError);
                    }

                    self.scratchpad.connecting_phase = ConnectingPhase::AuthInProgress;
                    Ok(())
                }
                _ => {
                    self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                    Err(Error::ProtocolError)
                }
            },
            ClientLifecycleState::Connected => match packet {
                ControlPacket::Publish(mut publish) => {
                    if self
                        .apply_inbound_publish_topic_alias(&mut publish)
                        .is_err()
                    {
                        self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                        return Err(Error::ProtocolError);
                    }

                    match publish.kind {
                        PublishKind::FireAndForget => {
                            self.scratchpad
                                .read_queue
                                .push_back(UserWriteOut::ReceivedMessage(
                                    Self::map_inbound_publish_to_broker_message(publish),
                                ));
                            Ok(())
                        }
                        PublishKind::Repetible {
                            packet_id,
                            qos: GuaranteedQoS::AtLeastOnce,
                            ..
                        } => match self.session.on_flight_received.get(&packet_id).copied() {
                            None => {
                                self.scratchpad.read_queue.push_back(
                                    UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(
                                        InboundMessageId::new(packet_id),
                                        Self::map_inbound_publish_to_broker_message(publish),
                                    ),
                                );
                                self.session
                                    .on_flight_received
                                    .insert(packet_id, InboundInflightState::Qos1AwaitAppDecision);
                                Ok(())
                            }
                            Some(InboundInflightState::Qos1AwaitAppDecision) => Ok(()),
                            Some(
                                InboundInflightState::Qos2AwaitAppDecision
                                | InboundInflightState::Qos2AwaitPubRel
                                | InboundInflightState::Qos2Rejected(_),
                            ) => {
                                self.fail_protocol_and_disconnect(
                                    DisconnectReasonCode::ProtocolError,
                                )?;
                                Err(Error::ProtocolError)
                            }
                        },
                        PublishKind::Repetible {
                            packet_id,
                            qos: GuaranteedQoS::ExactlyOnce,
                            ..
                        } => match self.session.on_flight_received.get(&packet_id).copied() {
                            Some(InboundInflightState::Qos2AwaitPubRel) => self
                                .enqueue_pubrec_or_fail_protocol(
                                    packet_id,
                                    PubRecReasonCode::Success,
                                ),
                            Some(InboundInflightState::Qos2AwaitAppDecision) => Ok(()),
                            Some(InboundInflightState::Qos2Rejected(reason_code)) => {
                                self.enqueue_pubrec_or_fail_protocol(packet_id, reason_code)
                            }
                            Some(InboundInflightState::Qos1AwaitAppDecision) => {
                                self.fail_protocol_and_disconnect(
                                    DisconnectReasonCode::ProtocolError,
                                )?;
                                Err(Error::ProtocolError)
                            }
                            None => {
                                self.scratchpad.read_queue.push_back(
                                    UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(
                                        InboundMessageId::new(packet_id),
                                        Self::map_inbound_publish_to_broker_message(publish),
                                    ),
                                );
                                self.session
                                    .on_flight_received
                                    .insert(packet_id, InboundInflightState::Qos2AwaitAppDecision);
                                Ok(())
                            }
                        },
                    }
                }
                ControlPacket::PubRel(pubrel) => {
                    let packet_id = pubrel.packet_id;

                    match self.session.on_flight_received.get(&packet_id).copied() {
                        Some(InboundInflightState::Qos2AwaitPubRel) => {
                            let _ = self.session.on_flight_received.remove(&packet_id);
                            self.enqueue_pubcomp_or_fail_protocol(
                                packet_id,
                                PubCompReasonCode::Success,
                            )
                        }
                        Some(
                            InboundInflightState::Qos1AwaitAppDecision
                            | InboundInflightState::Qos2AwaitAppDecision,
                        ) => {
                            self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                            Err(Error::ProtocolError)
                        }
                        Some(InboundInflightState::Qos2Rejected(_)) => {
                            let _ = self.session.on_flight_received.remove(&packet_id);
                            self.enqueue_pubcomp_or_fail_protocol(
                                packet_id,
                                PubCompReasonCode::PacketIdentifierNotFound,
                            )
                        }
                        None => self.enqueue_pubcomp_or_fail_protocol(
                            packet_id,
                            PubCompReasonCode::PacketIdentifierNotFound,
                        ),
                    }
                }
                ControlPacket::PubAck(puback) => {
                    let packet_id = puback.packet_id;
                    let reason_code = puback.reason_code;

                    match self.session.on_flight_sent.get(&packet_id) {
                        Some(OutboundInflightState::Qos1AwaitPubAck { .. }) => {
                            // [MQTT-4.3.2-3] QoS1 sender keeps PUBLISH unacknowledged until matching PUBACK is received.
                            let _ = self.session.on_flight_sent.remove(&packet_id);
                            self.scratchpad.read_queue.push_back(
                                UserWriteOut::PublishAcknowledged(packet_id, reason_code),
                            );
                            Ok(())
                        }
                        _ => {
                            self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                            Err(Error::ProtocolError)
                        }
                    }
                }
                ControlPacket::PubRec(pubrec) => {
                    let packet_id = pubrec.packet_id;
                    let reason_code = pubrec.reason_code;

                    match self.session.on_flight_sent.get(&packet_id).cloned() {
                        Some(OutboundInflightState::Qos2AwaitPubRec { .. }) => {
                            // [MQTT-4.3.3-4] QoS2 sender sends PUBREL with the same Packet Identifier after PUBREC (Reason Code < 0x80).
                            if matches!(
                                reason_code,
                                PubRecReasonCode::Success | PubRecReasonCode::NoMatchingSubscribers
                            ) {
                                self.enqueue_pubrel_or_fail_protocol(packet_id)?;

                                self.session
                                    .on_flight_sent
                                    .insert(packet_id, OutboundInflightState::Qos2AwaitPubComp);
                            } else {
                                let _ = self.session.on_flight_sent.remove(&packet_id);
                                self.scratchpad.read_queue.push_back(
                                    UserWriteOut::PublishDroppedDueToBrokerRejectedPubRec(
                                        packet_id,
                                        reason_code,
                                    ),
                                );
                            }

                            Ok(())
                        }
                        Some(OutboundInflightState::Qos2AwaitPubComp) => {
                            // [MQTT-4.3.3-4] Repeated PUBREC still requires PUBREL with the same Packet Identifier.
                            self.enqueue_pubrel_or_fail_protocol(packet_id)?;
                            Ok(())
                        }
                        _ => {
                            self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                            Err(Error::ProtocolError)
                        }
                    }
                }
                ControlPacket::PubComp(pubcomp) => {
                    let packet_id = pubcomp.packet_id;
                    let reason_code = pubcomp.reason_code;

                    match self.session.on_flight_sent.get(&packet_id) {
                        Some(OutboundInflightState::Qos2AwaitPubComp) => {
                            // [MQTT-4.3.3-5] QoS2 sender treats PUBREL as unacknowledged until matching PUBCOMP is received.
                            let _ = self.session.on_flight_sent.remove(&packet_id);
                            self.scratchpad
                                .read_queue
                                .push_back(UserWriteOut::PublishCompleted(packet_id, reason_code));

                            Ok(())
                        }
                        _ => {
                            self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                            Err(Error::ProtocolError)
                        }
                    }
                }
                ControlPacket::PingResp(_) => Ok(()),
                ControlPacket::SubAck(suback) => {
                    // [MQTT-3.8.4-1] SUBACK MUST correspond to an outstanding SUBSCRIBE Packet Identifier.
                    if self
                        .session
                        .pending_subscribe
                        .remove(&suback.packet_id)
                        .is_none()
                    {
                        self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                        return Err(Error::ProtocolError);
                    }
                    Ok(())
                }
                ControlPacket::UnsubAck(unsuback) => {
                    // [MQTT-3.10.4-1] UNSUBACK MUST correspond to an outstanding UNSUBSCRIBE Packet Identifier.
                    if self
                        .session
                        .pending_unsubscribe
                        .remove(&unsuback.packet_id)
                        .is_none()
                    {
                        self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                        return Err(Error::ProtocolError);
                    }
                    Ok(())
                }
                ControlPacket::Disconnect(_) => {
                    self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                    self.reset_keepalive();
                    self.reset_negotiated_limits();
                    self.maybe_reset_session_state();
                    self.scratchpad
                        .read_queue
                        .push_back(UserWriteOut::Disconnected);
                    self.scratchpad
                        .action_queue
                        .push_back(DriverEventOut::CloseSocket);
                    Ok(())
                }
                _ => {
                    self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                    Err(Error::ProtocolError)
                }
            },
            ClientLifecycleState::Start | ClientLifecycleState::Disconnected => {
                self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                Err(Error::ProtocolError)
            }
        }
    }

    fn map_inbound_publish_to_broker_message(publish: Publish) -> BrokerMessage {
        let qos = match &publish.kind {
            PublishKind::FireAndForget => Qos::AtMostOnce,
            PublishKind::Repetible { qos, .. } => Qos::from(*qos),
        };
        let retain = publish.retain;
        let properties = publish.properties;

        BrokerMessage {
            qos,
            retain,
            topic: publish.topic,
            payload: publish.payload,
            payload_format_indicator: properties.payload_format_indicator,
            message_expiry_interval: properties
                .message_expiry_interval
                .map(|seconds| Duration::from_secs(u64::from(seconds))),
            topic_alias: properties.topic_alias,
            response_topic: properties.response_topic,
            correlation_data: properties.correlation_data,
            subscription_identifiers: properties.subscription_identifiers,
            content_type: properties.content_type,
            user_properties: properties.user_properties,
        }
    }

    fn map_incoming_reject_reason_to_puback(reason: IncomingRejectReason) -> PubAckReasonCode {
        match reason {
            IncomingRejectReason::UnspecifiedError => PubAckReasonCode::UnspecifiedError,
            IncomingRejectReason::ImplementationSpecificError => {
                PubAckReasonCode::ImplementationSpecificError
            }
            IncomingRejectReason::NotAuthorized => PubAckReasonCode::NotAuthorized,
            IncomingRejectReason::TopicNameInvalid => PubAckReasonCode::TopicNameInvalid,
            IncomingRejectReason::QuotaExceeded => PubAckReasonCode::QuotaExceeded,
            IncomingRejectReason::PayloadFormatInvalid => PubAckReasonCode::PayloadFormatInvalid,
        }
    }

    fn map_incoming_reject_reason_to_pubrec(reason: IncomingRejectReason) -> PubRecReasonCode {
        match reason {
            IncomingRejectReason::UnspecifiedError => PubRecReasonCode::UnspecifiedError,
            IncomingRejectReason::ImplementationSpecificError => {
                PubRecReasonCode::ImplementationSpecificError
            }
            IncomingRejectReason::NotAuthorized => PubRecReasonCode::NotAuthorized,
            IncomingRejectReason::TopicNameInvalid => PubRecReasonCode::TopicNameInvalid,
            IncomingRejectReason::QuotaExceeded => PubRecReasonCode::QuotaExceeded,
            IncomingRejectReason::PayloadFormatInvalid => PubRecReasonCode::PayloadFormatInvalid,
        }
    }
}

impl<Time> Protocol<Bytes, UserWriteIn, DriverEventIn> for Client<Time>
where
    Time: Copy + Ord + 'static,
{
    type Rout = UserWriteOut;
    type Wout = Bytes;
    type Eout = DriverEventOut;
    type Error = Error;
    type Time = Time;

    #[tracing::instrument(skip_all)]
    fn handle_read(&mut self, msg: Bytes) -> Result<(), Self::Error> {
        let packet_bytes = if self.scratchpad.read_buffer.is_empty() {
            msg
        } else {
            let mut combined = core::mem::take(&mut self.scratchpad.read_buffer);
            combined.extend_from_slice(&msg);
            combined.freeze()
        };

        let parser_settings = self.parser_settings();
        let mut slice: &[u8] = packet_bytes.as_ref();

        while !slice.is_empty() {
            let mut input = Partial::new(slice);

            match ControlPacket::parser::<_, ErrMode<()>, ErrMode<()>>(&parser_settings)
                .parse_next(&mut input)
            {
                Ok(packet) => {
                    slice = input.into_inner();
                    self.scratchpad.keep_alive_saw_network_activity = true;
                    if matches!(packet, ControlPacket::PingResp(_)) {
                        self.scratchpad.keep_alive_ping_outstanding = false;
                    }
                    self.handle_read_control_packet(packet)?;
                }
                Err(ErrMode::Incomplete(_)) => {
                    break;
                }
                Err(ErrMode::Backtrack(_)) | Err(ErrMode::Cut(_)) => {
                    // [MQTT-4.13.1-1] Malformed Control Packet is a protocol error and requires disconnect.
                    self.fail_protocol_and_disconnect(DisconnectReasonCode::MalformedPacket)?;

                    return Err(Error::MalformedPacket);
                }
            }
        }

        self.scratchpad.read_buffer = BytesMut::from(slice);

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn handle_write(&mut self, msg: UserWriteIn) -> Result<(), Self::Error> {
        match msg {
            UserWriteIn::Connect(options) => {
                if self.scratchpad.lifecycle_state == ClientLifecycleState::Start
                    || self.scratchpad.lifecycle_state == ClientLifecycleState::Disconnected
                {
                    self.scratchpad.pending_connect_options = options;
                    self.recompute_effective_limits();
                    if self.scratchpad.pending_connect_options.clean_start {
                        // [MQTT-3.1.2-4] Clean Start=1 starts a new Session.
                        self.session.clear();
                    }
                    self.scratchpad.session_should_persist = self
                        .scratchpad
                        .pending_connect_options
                        .session_expiry_interval
                        .unwrap_or(0)
                        > 0;

                    if !self
                        .scratchpad
                        .action_queue
                        .iter()
                        .any(|event| matches!(event, DriverEventOut::OpenSocket))
                    {
                        self.scratchpad
                            .action_queue
                            .push_back(DriverEventOut::OpenSocket);
                    }
                    Ok(())
                } else {
                    Err(Error::InvalidStateTransition)
                }
            }
            UserWriteIn::PublishMessage(msg) => {
                if self.scratchpad.lifecycle_state != ClientLifecycleState::Connected {
                    return Err(Error::InvalidStateTransition);
                }

                self.validate_outbound_topic_alias(msg.topic_alias)?;
                self.validate_outbound_publish_capabilities(&msg)?;

                if matches!(msg.qos, Qos::AtLeastOnce | Qos::ExactlyOnce) {
                    // [MQTT-4.9.0-1] Apply peer Receive Maximum before sending QoS1/QoS2 PUBLISH.
                    self.ensure_outbound_receive_maximum_capacity()?;
                }

                let message_expiry_interval = msg
                    .message_expiry_interval
                    .map(|interval| {
                        u32::try_from(interval.as_secs()).map_err(|_| Error::ProtocolError)
                    })
                    .transpose()?;
                let properties = PublishProperties {
                    payload_format_indicator: msg.payload_format_indicator,
                    message_expiry_interval,
                    topic_alias: msg.topic_alias,
                    response_topic: msg.response_topic,
                    correlation_data: msg.correlation_data,
                    user_properties: msg.user_properties,
                    subscription_identifiers: Vec::new(),
                    content_type: msg.content_type,
                };
                let kind = match msg.qos {
                    Qos::AtMostOnce => PublishKind::FireAndForget,
                    Qos::AtLeastOnce => PublishKind::Repetible {
                        packet_id: self.next_outbound_publish_packet_id()?,
                        qos: GuaranteedQoS::AtLeastOnce,
                        dup: false,
                    },
                    Qos::ExactlyOnce => PublishKind::Repetible {
                        packet_id: self.next_outbound_publish_packet_id()?,
                        qos: GuaranteedQoS::ExactlyOnce,
                        dup: false,
                    },
                };
                let inflight_state = match msg.qos {
                    Qos::AtMostOnce => None,
                    Qos::AtLeastOnce => Some(OutboundInflightState::Qos1AwaitPubAck {
                        publish: Publish {
                            kind: kind.clone(),
                            retain: msg.retain,
                            payload: msg.payload.clone(),
                            topic: msg.topic.clone(),
                            properties: properties.clone(),
                        },
                    }),
                    Qos::ExactlyOnce => Some(OutboundInflightState::Qos2AwaitPubRec {
                        publish: Publish {
                            kind: kind.clone(),
                            retain: msg.retain,
                            payload: msg.payload.clone(),
                            topic: msg.topic.clone(),
                            properties: properties.clone(),
                        },
                    }),
                };
                let packet = ControlPacket::Publish(Publish {
                    kind: kind.clone(),
                    retain: msg.retain,
                    payload: msg.payload,
                    topic: msg.topic,
                    properties,
                });

                self.enqueue_packet(packet)?;

                if let (PublishKind::Repetible { packet_id, .. }, Some(inflight_state)) =
                    (kind, inflight_state)
                {
                    self.session.on_flight_sent.insert(packet_id, inflight_state);
                }

                Ok(())
            }
            UserWriteIn::AcknowledgeMessage(inbound_message_id) => {
                let packet_id = inbound_message_id.get();
                if self.scratchpad.lifecycle_state != ClientLifecycleState::Connected {
                    return Err(Error::InvalidStateTransition);
                }

                match self.session.on_flight_received.get(&packet_id).copied() {
                    Some(InboundInflightState::Qos1AwaitAppDecision) => {
                        self.enqueue_puback_or_fail_protocol(packet_id, PubAckReasonCode::Success)?;
                        let _ = self.session.on_flight_received.remove(&packet_id);
                        Ok(())
                    }
                    Some(InboundInflightState::Qos2AwaitAppDecision) => {
                        self.enqueue_pubrec_or_fail_protocol(packet_id, PubRecReasonCode::Success)?;
                        self.session
                            .on_flight_received
                            .insert(packet_id, InboundInflightState::Qos2AwaitPubRel);
                        Ok(())
                    }
                    Some(InboundInflightState::Qos2AwaitPubRel)
                    | Some(InboundInflightState::Qos2Rejected(_))
                    | None => Err(Error::ProtocolError),
                }
            }
            UserWriteIn::RejectMessage(inbound_message_id, reason) => {
                let packet_id = inbound_message_id.get();
                if self.scratchpad.lifecycle_state != ClientLifecycleState::Connected {
                    return Err(Error::InvalidStateTransition);
                }

                match self.session.on_flight_received.get(&packet_id).copied() {
                    Some(InboundInflightState::Qos1AwaitAppDecision) => {
                        self.enqueue_puback_or_fail_protocol(
                            packet_id,
                            Self::map_incoming_reject_reason_to_puback(reason),
                        )?;
                        let _ = self.session.on_flight_received.remove(&packet_id);
                        Ok(())
                    }
                    Some(InboundInflightState::Qos2AwaitAppDecision) => {
                        let reason_code = Self::map_incoming_reject_reason_to_pubrec(reason);
                        self.enqueue_pubrec_or_fail_protocol(packet_id, reason_code)?;
                        self.session
                            .on_flight_received
                            .insert(packet_id, InboundInflightState::Qos2Rejected(reason_code));
                        Ok(())
                    }
                    Some(InboundInflightState::Qos2AwaitPubRel)
                    | Some(InboundInflightState::Qos2Rejected(_))
                    | None => Err(Error::ProtocolError),
                }
            }
            UserWriteIn::Subscribe(options) => {
                if self.scratchpad.lifecycle_state != ClientLifecycleState::Connected {
                    return Err(Error::InvalidStateTransition);
                }

                if options.subscription_identifier.is_some()
                    && !self.scratchpad.effective_subscription_identifiers_available
                {
                    return Err(Error::ProtocolError);
                }

                let subscriptions = core::iter::once(options.subscription)
                    .chain(options.extra_subscriptions)
                    .map(|subscription| {
                        let topic_filter_str: &str = subscription.topic_filter.as_ref();
                        let is_shared = topic_filter_str.starts_with("$share/");
                        let has_wildcard =
                            topic_filter_str.contains('+') || topic_filter_str.contains('#');

                        if has_wildcard
                            && !self.scratchpad.effective_wildcard_subscription_available
                        {
                            return Err(Error::ProtocolError);
                        }

                        if is_shared {
                            if !self.scratchpad.effective_shared_subscription_available {
                                return Err(Error::ProtocolError);
                            }

                            // [MQTT-3.8.3-4] A Shared Subscription cannot be used with No Local.
                            if subscription.no_local {
                                return Err(Error::ProtocolError);
                            }
                        }

                        Ok(subscription)
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                let mut subscriptions = subscriptions.into_iter();
                let subscription = subscriptions.next().ok_or(Error::ProtocolError)?;
                let packet_id = self.next_packet_id_checked()?;

                self.enqueue_packet(ControlPacket::Subscribe(Subscribe {
                    packet_id,
                    subscription,
                    extra_subscriptions: subscriptions.collect(),
                    properties: SubscribeProperties {
                        subscription_identifier: options.subscription_identifier,
                        user_properties: options.user_properties,
                    },
                }))?;
                self.session.pending_subscribe.insert(packet_id, ());

                Ok(())
            }
            UserWriteIn::Unsubscribe(options) => {
                if self.scratchpad.lifecycle_state != ClientLifecycleState::Connected {
                    return Err(Error::InvalidStateTransition);
                }
                let packet_id = self.next_packet_id_checked()?;

                self.enqueue_packet(ControlPacket::Unsubscribe(Unsubscribe {
                    packet_id,
                    properties: UnsubscribeProperties {
                        user_properties: options.user_properties,
                    },
                    filter: options.filter,
                    extra_filters: options.extra_filters,
                }))?;
                self.session.pending_unsubscribe.insert(packet_id, ());

                Ok(())
            }
            UserWriteIn::Disconnect => match self.scratchpad.lifecycle_state {
                ClientLifecycleState::Connected | ClientLifecycleState::Connecting => {
                    let _ = self.enqueue_packet(ControlPacket::Disconnect(Disconnect {
                        reason_code: DisconnectReasonCode::NormalDisconnection,
                        properties: DisconnectProperties::default(),
                    }));
                    self.scratchpad
                        .action_queue
                        .push_back(DriverEventOut::CloseSocket);
                    self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                    self.scratchpad.read_buffer.clear();
                    self.reset_keepalive();
                    self.reset_negotiated_limits();
                    self.maybe_reset_session_state();
                    self.scratchpad
                        .read_queue
                        .push_back(UserWriteOut::Disconnected);
                    Ok(())
                }
                ClientLifecycleState::Disconnected => Ok(()),
                ClientLifecycleState::Start => {
                    self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                    self.scratchpad.read_buffer.clear();
                    self.reset_keepalive();
                    self.reset_negotiated_limits();
                    self.maybe_reset_session_state();
                    Ok(())
                }
            },
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_event(&mut self, evt: DriverEventIn) -> Result<(), Self::Error> {
        match evt {
            DriverEventIn::SocketConnected => {
                if self.scratchpad.lifecycle_state != ClientLifecycleState::Start
                    && self.scratchpad.lifecycle_state != ClientLifecycleState::Disconnected
                {
                    return Err(Error::InvalidStateTransition);
                }

                self.reset_negotiated_limits();
                let connect_packet =
                    self.build_connect_packet(&self.scratchpad.pending_connect_options)?;
                self.enqueue_packet(ControlPacket::Connect(connect_packet))?;
                self.scratchpad.lifecycle_state = ClientLifecycleState::Connecting;
                self.scratchpad.connecting_phase = ConnectingPhase::AwaitConnAck;
                self.scratchpad.keep_alive_saw_network_activity = false;
                self.scratchpad.keep_alive_ping_outstanding = false;
                Ok(())
            }
            DriverEventIn::SocketClosed => {
                let was_disconnected =
                    self.scratchpad.lifecycle_state == ClientLifecycleState::Disconnected;
                self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                self.scratchpad.read_buffer.clear();
                self.reset_keepalive();
                self.reset_negotiated_limits();
                self.maybe_reset_session_state();

                if !was_disconnected {
                    self.scratchpad
                        .read_queue
                        .push_back(UserWriteOut::Disconnected);
                }

                Ok(())
            }
            DriverEventIn::SocketError => {
                self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                self.scratchpad.read_buffer.clear();
                self.reset_keepalive();
                self.reset_negotiated_limits();
                self.maybe_reset_session_state();
                self.scratchpad
                    .action_queue
                    .push_back(DriverEventOut::CloseSocket);
                Err(Error::ProtocolError)
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_timeout(&mut self, now: Self::Time) -> Result<(), Self::Error> {
        if self.scratchpad.lifecycle_state != ClientLifecycleState::Connected {
            return Ok(());
        }

        if self.scratchpad.keep_alive_interval_secs.is_none() {
            self.scratchpad.next_timeout = None;
            return Ok(());
        }

        if self.scratchpad.keep_alive_ping_outstanding {
            // [MQTT-3.1.2-24] [MQTT-4.13.1-1] Keep Alive timeout closes the network connection.
            self.fail_protocol_and_disconnect(DisconnectReasonCode::KeepAliveTimeout)?;
            return Err(Error::ProtocolError);
        }

        if self.scratchpad.keep_alive_saw_network_activity {
            // [MQTT-3.1.2-22] Any control packet traffic resets keep-alive idle detection.
            self.scratchpad.keep_alive_saw_network_activity = false;
            self.scratchpad.next_timeout = Some(now);
            return Ok(());
        }

        // [MQTT-3.1.2-22] [MQTT-3.12.4-1] Send PINGREQ when Keep Alive elapses without traffic.
        self.enqueue_packet(ControlPacket::PingReq(PingReq {}))?;
        self.scratchpad.keep_alive_ping_outstanding = true;
        self.scratchpad.next_timeout = Some(now);

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn close(&mut self) -> Result<(), Self::Error> {
        match self.scratchpad.lifecycle_state {
            ClientLifecycleState::Connected | ClientLifecycleState::Connecting => {
                let _ = self.enqueue_packet(ControlPacket::Disconnect(Disconnect {
                    reason_code: DisconnectReasonCode::NormalDisconnection,
                    properties: DisconnectProperties::default(),
                }));
                self.scratchpad
                    .action_queue
                    .push_back(DriverEventOut::CloseSocket);
                self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                self.scratchpad.read_buffer.clear();
                self.reset_keepalive();
                self.reset_negotiated_limits();
                self.maybe_reset_session_state();
                self.scratchpad
                    .read_queue
                    .push_back(UserWriteOut::Disconnected);
                Ok(())
            }
            ClientLifecycleState::Disconnected => Ok(()),
            ClientLifecycleState::Start => {
                self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                self.scratchpad.read_buffer.clear();
                self.reset_keepalive();
                self.reset_negotiated_limits();
                self.maybe_reset_session_state();
                Ok(())
            }
        }
    }

    fn poll_read(&mut self) -> Option<Self::Rout> {
        self.scratchpad.read_queue.pop_front()
    }

    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.scratchpad.write_queue.pop_front()
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.scratchpad.action_queue.pop_front()
    }

    fn poll_timeout(&mut self) -> Option<Self::Time> {
        self.scratchpad.next_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use core::time::Duration;
    use sansio_mqtt_v5_types::BinaryData;
    use sansio_mqtt_v5_types::FormatIndicator;
    use sansio_mqtt_v5_types::Payload;
    use sansio_mqtt_v5_types::PubRec;
    use sansio_mqtt_v5_types::PubRecProperties;
    use sansio_mqtt_v5_types::PubRecReasonCode;
    use sansio_mqtt_v5_types::Qos;
    use sansio_mqtt_v5_types::Topic;

    #[test]
    fn build_connect_packet_maps_will_from_connection_options() {
        let will = crate::types::Will {
            topic: Topic::try_from(Utf8String::try_from("topic/will").expect("valid utf8"))
                .expect("valid topic"),
            payload: Payload::from(&b"will payload"[..]),
            qos: Qos::AtMostOnce,
            retain: false,
            payload_format_indicator: Some(FormatIndicator::Utf8),
            message_expiry_interval: Some(Duration::from_secs(42)),
            response_topic: Some(
                Topic::try_from(Utf8String::try_from("topic/response").expect("valid utf8"))
                    .expect("valid topic"),
            ),
            correlation_data: Some(
                BinaryData::try_from(&b"correlation"[..]).expect("valid binary data"),
            ),
            content_type: Some(Utf8String::try_from("text/plain").expect("valid utf8")),
            user_properties: vec![(
                Utf8String::try_from("k").expect("valid utf8"),
                Utf8String::try_from("v").expect("valid utf8"),
            )],
            will_delay_interval: Some(7),
        };
        let options = ConnectionOptions {
            will: Some(will.clone()),
            ..ConnectionOptions::default()
        };

        let client = Client::<u64>::default();
        let connect = client
            .build_connect_packet(&options)
            .expect("connect packet should build");
        let mapped_will = connect.will.expect("will should be present");

        assert_eq!(mapped_will.topic, will.topic);
        assert_eq!(mapped_will.payload.as_ref(), will.payload.as_ref());
        assert_eq!(mapped_will.qos, Qos::AtMostOnce);
        assert!(!mapped_will.retain);
        assert_eq!(
            mapped_will.properties.will_delay_interval,
            will.will_delay_interval
        );
        assert_eq!(
            mapped_will.properties.payload_format_indicator,
            will.payload_format_indicator
        );
        assert_eq!(mapped_will.properties.message_expiry_interval, Some(42));
        assert_eq!(mapped_will.properties.response_topic, will.response_topic);
        assert_eq!(
            mapped_will.properties.correlation_data,
            will.correlation_data
        );
        assert_eq!(mapped_will.properties.content_type, will.content_type);
        assert_eq!(mapped_will.properties.user_properties, will.user_properties);
    }

    #[test]
    fn build_connect_packet_errors_on_message_expiry_interval_overflow() {
        let options = ConnectionOptions {
            will: Some(crate::types::Will {
                message_expiry_interval: Some(Duration::from_secs(u64::from(u32::MAX) + 1)),
                ..crate::types::Will::default()
            }),
            ..ConnectionOptions::default()
        };

        let client = Client::<u64>::default();

        assert_eq!(
            client.build_connect_packet(&options),
            Err(Error::ProtocolError)
        );
    }

    #[test]
    fn build_connect_packet_maps_will_qos_and_retain_from_options() {
        let will = crate::types::Will {
            qos: Qos::ExactlyOnce,
            retain: true,
            ..crate::types::Will::default()
        };
        let options = ConnectionOptions {
            will: Some(will.clone()),
            ..ConnectionOptions::default()
        };

        let client = Client::<u64>::default();
        let connect = client
            .build_connect_packet(&options)
            .expect("connect packet should build");
        let mapped_will = connect.will.expect("will should be present");

        assert_eq!(mapped_will.qos, will.qos);
        assert_eq!(mapped_will.retain, will.retain);
    }

    #[test]
    fn socket_connected_error_does_not_poison_state() {
        let mut client = Client::<u64>::default();
        client.scratchpad.pending_connect_options = ConnectionOptions {
            will: Some(crate::types::Will {
                message_expiry_interval: Some(Duration::from_secs(u64::from(u32::MAX) + 1)),
                ..crate::types::Will::default()
            }),
            ..ConnectionOptions::default()
        };

        assert_eq!(
            client.handle_event(DriverEventIn::SocketConnected),
            Err(Error::ProtocolError)
        );
        assert_eq!(
            client.scratchpad.lifecycle_state,
            ClientLifecycleState::Start
        );
        assert_eq!(client.poll_write(), None);
    }

    #[test]
    fn pubrel_enqueue_failure_forces_protocol_close() {
        let mut client = Client::<u64>::default();
        let packet_id = NonZero::new(1).expect("non-zero packet id");

        client.scratchpad.lifecycle_state = ClientLifecycleState::Connected;
        client.scratchpad.effective_broker_maximum_packet_size =
            NonZero::new(1).expect("non-zero packet size limit").into();
        client.session.on_flight_sent.insert(
            packet_id,
            OutboundInflightState::Qos2AwaitPubRec {
                publish: Publish {
                    kind: PublishKind::Repetible {
                        packet_id,
                        qos: GuaranteedQoS::ExactlyOnce,
                        dup: false,
                    },
                    retain: false,
                    payload: Payload::from(&b"test"[..]),
                    topic: Topic::try_from(Utf8String::try_from("topic/test").expect("valid utf8"))
                        .expect("valid topic"),
                    properties: PublishProperties::default(),
                },
            },
        );

        assert_eq!(
            client.handle_read_control_packet(ControlPacket::PubRec(PubRec {
                packet_id,
                reason_code: PubRecReasonCode::Success,
                properties: PubRecProperties::default(),
            })),
            Err(Error::ProtocolError)
        );

        assert_eq!(
            client.scratchpad.lifecycle_state,
            ClientLifecycleState::Disconnected
        );
        assert!(matches!(
            client.poll_event(),
            Some(DriverEventOut::CloseSocket)
        ));
        assert!(client.session.on_flight_sent.is_empty());
    }
}
