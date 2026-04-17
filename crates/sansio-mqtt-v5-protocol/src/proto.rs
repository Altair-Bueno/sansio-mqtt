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
use sansio_mqtt_v5_types::PingReq;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::PublishProperties;
use sansio_mqtt_v5_types::Qos;
use sansio_mqtt_v5_types::RetainHandling;
use sansio_mqtt_v5_types::Settings;
use sansio_mqtt_v5_types::Subscribe;
use sansio_mqtt_v5_types::SubscribeProperties;
use sansio_mqtt_v5_types::Subscription;
use sansio_mqtt_v5_types::Unsubscribe;
use sansio_mqtt_v5_types::UnsubscribeProperties;
use sansio_mqtt_v5_types::Utf8String;
use sansio_mqtt_v5_types::Will as ConnectWill;
use sansio_mqtt_v5_types::WillProperties;
use winnow::error::ErrMode;
use winnow::stream::Partial;
use winnow::Parser;

#[derive(Debug, Clone, PartialEq, Eq)]
struct NegotiatedLimits {
    receive_maximum: NonZero<u16>,
    maximum_packet_size: Option<NonZero<u32>>,
    topic_alias_maximum: u16,
    server_keep_alive: Option<u16>,
}

impl Default for NegotiatedLimits {
    fn default() -> Self {
        Self {
            receive_maximum: NonZero::new(u16::MAX)
                .expect("u16::MAX is always non-zero for receive_maximum"),
            maximum_packet_size: None,
            topic_alias_maximum: 0,
            server_keep_alive: None,
        }
    }
}

#[derive(Debug, PartialEq, Default)]
enum ClientState {
    #[default]
    Start,
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, PartialEq)]
pub struct Client<Time>
where
    Time: 'static,
{
    config: Config,
    pending_connect_options: ConnectionOptions,
    state: ClientState,
    negotiated_limits: NegotiatedLimits,

    // Buffer for accumulating incoming bytes until a full control packet can be parsed
    read_buffer: BytesMut,

    // Pending packages to be acknowledged indexed by packet identifier
    on_flight_sent: BTreeMap<NonZero<u16>, ClientMessage>,
    on_flight_received: BTreeMap<NonZero<u16>, ClientMessage>,

    // Output queues
    read_queue: VecDeque<UserWriteOut>,
    write_queue: VecDeque<Bytes>,
    action_queue: VecDeque<DriverEventOut>,
    next_timeout: Option<Time>,
    next_packet_id: u16,
}

impl<Time> Default for Client<Time> {
    fn default() -> Self {
        Self {
            config: Config::default(),
            pending_connect_options: ConnectionOptions::default(),
            state: ClientState::default(),
            negotiated_limits: NegotiatedLimits::default(),
            read_buffer: BytesMut::new(),
            on_flight_sent: BTreeMap::new(),
            on_flight_received: BTreeMap::new(),
            read_queue: VecDeque::new(),
            write_queue: VecDeque::new(),
            action_queue: VecDeque::new(),
            next_timeout: None,
            next_packet_id: 1,
        }
    }
}

impl<Time> Client<Time> {
    pub fn with_config(config: Config) -> Self {
        Self {
            config,
            ..Self::default()
        }
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
        self.write_queue.push_back(encoded);
        Ok(())
    }

    fn build_connect_packet(&self, options: &ConnectionOptions) -> Result<Connect, Error> {
        let will = options
            .will
            .as_ref()
            .map(|will| {
                let payload = BinaryData::try_new(will.payload.clone().into())
                    .map_err(|_| Error::ProtocolError)?;
                let message_expiry_interval = will
                    .message_expiry_interval
                    .map(|interval| {
                        u32::try_from(interval.as_secs()).map_err(|_| Error::ProtocolError)
                    })
                    .transpose()?;

                Ok(ConnectWill {
                    topic: will.topic.clone(),
                    payload,
                    qos: Qos::AtMostOnce,
                    retain: false,
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
            keep_alive: options.keep_alive,
            properties: ConnectProperties {
                session_expiry_interval: options.session_expiry_interval,
                receive_maximum: None,
                maximum_packet_size: None,
                topic_alias_maximum: options.topic_alias_maximum,
                request_response_information: options.request_response_information,
                request_problem_information: options.request_problem_information,
                authentication: options.authentication.clone(),
                user_properties: options.user_properties.clone(),
            },
        })
    }

    fn parser_settings(&self) -> Settings {
        Settings {
            max_bytes_string: self.config.parser_max_bytes_string,
            max_bytes_binary_data: self.config.parser_max_bytes_binary_data,
            max_remaining_bytes: self.config.parser_max_remaining_bytes,
            max_subscriptions_len: self.config.parser_max_subscriptions_len,
            max_user_properties_len: self.config.parser_max_user_properties_len,
        }
    }

    fn next_packet_id(&mut self) -> NonZero<u16> {
        let packet_id = self.next_packet_id;
        self.next_packet_id = if packet_id == u16::MAX {
            1
        } else {
            packet_id + 1
        };

        NonZero::new(packet_id).expect("packet identifier is always non-zero")
    }

    fn validate_outbound_topic_alias(
        &self,
        topic_alias: Option<NonZero<u16>>,
    ) -> Result<(), Error> {
        if let Some(alias) = topic_alias {
            let topic_alias_maximum = self.negotiated_limits.topic_alias_maximum;
            if topic_alias_maximum == 0 || alias.get() > topic_alias_maximum {
                return Err(Error::ProtocolError);
            }
        }

        Ok(())
    }

    fn validate_outbound_packet_size(&self, packet_size_bytes: usize) -> Result<(), Error> {
        if let Some(maximum_packet_size) = self.negotiated_limits.maximum_packet_size {
            if packet_size_bytes > maximum_packet_size.get() as usize {
                return Err(Error::PacketTooLarge);
            }
        }

        Ok(())
    }

    fn reset_negotiated_limits(&mut self) {
        self.negotiated_limits = NegotiatedLimits::default();
    }

    fn fail_protocol_and_disconnect(&mut self, reason: DisconnectReasonCode) -> Result<(), Error> {
        let _ = self.enqueue_packet(ControlPacket::Disconnect(Disconnect {
            reason_code: reason,
            properties: DisconnectProperties::default(),
        }));

        self.action_queue.push_back(DriverEventOut::CloseSocket);
        self.state = ClientState::Disconnected;
        self.next_timeout = None;
        self.reset_negotiated_limits();

        Ok(())
    }

    fn handle_read_control_packet(&mut self, packet: ControlPacket) -> Result<(), Error> {
        match self.state {
            ClientState::Connecting => match packet {
                ControlPacket::ConnAck(connack) => {
                    let connack_is_success = matches!(
                        connack.kind,
                        sansio_mqtt_v5_types::ConnAckKind::ResumePreviousSession
                            | sansio_mqtt_v5_types::ConnAckKind::Other {
                                reason_code: ConnackReasonCode::Success
                            }
                    );

                    if connack_is_success {
                        self.negotiated_limits.receive_maximum =
                            connack.properties.receive_maximum.unwrap_or(
                                NonZero::new(u16::MAX).expect("u16::MAX is always non-zero"),
                            );
                        self.negotiated_limits.maximum_packet_size =
                            connack.properties.maximum_packet_size;
                        self.negotiated_limits.topic_alias_maximum =
                            connack.properties.topic_alias_maximum.unwrap_or(0);
                        self.negotiated_limits.server_keep_alive =
                            connack.properties.server_keep_alive;
                        self.state = ClientState::Connected;
                        self.read_queue.push_back(UserWriteOut::Connected);
                        Ok(())
                    } else {
                        self.state = ClientState::Disconnected;
                        self.reset_negotiated_limits();
                        self.action_queue.push_back(DriverEventOut::CloseSocket);
                        Err(Error::ProtocolError)
                    }
                }
                _ => {
                    self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                    Err(Error::ProtocolError)
                }
            },
            ClientState::Connected => match packet {
                ControlPacket::Publish(publish) => match publish.kind {
                    PublishKind::FireAndForget => {
                        self.read_queue.push_back(UserWriteOut::ReceivedMessage(
                            Self::map_inbound_publish_to_broker_message(publish),
                        ));
                        Ok(())
                    }
                    _ => {
                        self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                        Err(Error::ProtocolError)
                    }
                },
                ControlPacket::PingResp(_) => Ok(()),
                ControlPacket::SubAck(_) => Ok(()),
                ControlPacket::UnsubAck(_) => Ok(()),
                ControlPacket::Disconnect(_) => {
                    self.state = ClientState::Disconnected;
                    self.next_timeout = None;
                    self.reset_negotiated_limits();
                    self.read_queue.push_back(UserWriteOut::Disconnected);
                    self.action_queue.push_back(DriverEventOut::CloseSocket);
                    Ok(())
                }
                _ => {
                    self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                    Err(Error::ProtocolError)
                }
            },
            ClientState::Start | ClientState::Disconnected => {
                self.fail_protocol_and_disconnect(DisconnectReasonCode::ProtocolError)?;
                Err(Error::ProtocolError)
            }
        }
    }

    fn map_inbound_publish_to_broker_message(publish: Publish) -> BrokerMessage {
        let properties = publish.properties;

        BrokerMessage {
            topic: publish.topic,
            payload: publish.payload,
            payload_format_indicator: properties.payload_format_indicator,
            message_expiry_interval: properties
                .message_expiry_interval
                .map(|seconds| Duration::from_secs(u64::from(seconds))),
            topic_alias: properties.topic_alias,
            response_topic: properties.response_topic,
            correlation_data: properties.correlation_data,
            subscription_identifier: properties.subscription_identifier,
            content_type: properties.content_type,
            user_properties: properties.user_properties,
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
        let packet_bytes = if self.read_buffer.is_empty() {
            msg
        } else {
            let mut combined = core::mem::take(&mut self.read_buffer);
            combined.extend_from_slice(&msg);
            combined.freeze()
        };

        let parser_settings = self.parser_settings();
        let mut slice: &[u8] = packet_bytes.as_ref();

        while !slice.is_empty() {
            let mut input = Partial::new(slice);

            match ControlPacket::parse::<_, ErrMode<()>, ErrMode<()>>(&parser_settings)
                .parse_next(&mut input)
            {
                Ok(packet) => {
                    slice = input.into_inner();
                    self.handle_read_control_packet(packet)?;
                }
                Err(ErrMode::Incomplete(_)) => {
                    break;
                }
                Err(ErrMode::Backtrack(_)) | Err(ErrMode::Cut(_)) => {
                    self.fail_protocol_and_disconnect(DisconnectReasonCode::MalformedPacket)?;

                    return Err(Error::MalformedPacket);
                }
            }
        }

        self.read_buffer = BytesMut::from(slice);

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn handle_write(&mut self, msg: UserWriteIn) -> Result<(), Self::Error> {
        match msg {
            UserWriteIn::Connect(options) => {
                if self.state == ClientState::Start || self.state == ClientState::Disconnected {
                    self.pending_connect_options = options;

                    if !self.action_queue.contains(&DriverEventOut::OpenSocket) {
                        self.action_queue.push_back(DriverEventOut::OpenSocket);
                    }
                    Ok(())
                } else {
                    Err(Error::InvalidStateTransition)
                }
            }
            UserWriteIn::PublishMessage(msg) => {
                if self.state != ClientState::Connected {
                    return Err(Error::InvalidStateTransition);
                }

                if msg.qos != Qos::AtMostOnce {
                    return Err(Error::UnsupportedQosForMvp { qos: msg.qos });
                }

                self.validate_outbound_topic_alias(msg.topic_alias)?;

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
                    subscription_identifier: None,
                    content_type: msg.content_type,
                };
                let packet = ControlPacket::Publish(Publish {
                    kind: PublishKind::FireAndForget,
                    retain: false,
                    payload: msg.payload,
                    topic: msg.topic,
                    properties,
                });

                self.enqueue_packet(packet)?;

                Ok(())
            }
            UserWriteIn::Subscribe(options) => {
                if self.state != ClientState::Connected {
                    return Err(Error::InvalidStateTransition);
                }

                let retain_handling = RetainHandling::try_from(options.retain_handling)
                    .map_err(|_| Error::ProtocolError)?;
                let subscriptions = options
                    .subscriptions
                    .into_iter()
                    .map(|topic_filter| Subscription {
                        topic_filter,
                        qos: options.qos,
                        no_local: options.no_local,
                        retain_as_published: options.retain_as_published,
                        retain_handling,
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .map_err(|_| Error::ProtocolError)?;
                let packet_id = self.next_packet_id();

                self.enqueue_packet(ControlPacket::Subscribe(Subscribe {
                    packet_id,
                    subscriptions,
                    properties: SubscribeProperties {
                        subscription_identifier: options.subscription_identifier,
                        user_properties: options.user_properties,
                    },
                }))?;

                Ok(())
            }
            UserWriteIn::Unsubscribe(options) => {
                if self.state != ClientState::Connected {
                    return Err(Error::InvalidStateTransition);
                }
                let packet_id = self.next_packet_id();

                self.enqueue_packet(ControlPacket::Unsubscribe(Unsubscribe {
                    packet_id,
                    properties: UnsubscribeProperties {
                        user_properties: options.user_properties,
                    },
                    topics: options.subscriptions,
                }))?;

                Ok(())
            }
            UserWriteIn::Disconnect => match self.state {
                ClientState::Connected | ClientState::Connecting => {
                    let _ = self.enqueue_packet(ControlPacket::Disconnect(Disconnect {
                        reason_code: DisconnectReasonCode::NormalDisconnection,
                        properties: DisconnectProperties::default(),
                    }));
                    self.action_queue.push_back(DriverEventOut::CloseSocket);
                    self.state = ClientState::Disconnected;
                    self.next_timeout = None;
                    self.reset_negotiated_limits();
                    self.read_queue.push_back(UserWriteOut::Disconnected);
                    Ok(())
                }
                ClientState::Disconnected => Ok(()),
                ClientState::Start => {
                    self.state = ClientState::Disconnected;
                    self.next_timeout = None;
                    self.reset_negotiated_limits();
                    Ok(())
                }
            },
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_event(&mut self, evt: DriverEventIn) -> Result<(), Self::Error> {
        match evt {
            DriverEventIn::SocketConnected => {
                if self.state != ClientState::Start && self.state != ClientState::Disconnected {
                    return Err(Error::InvalidStateTransition);
                }

                self.reset_negotiated_limits();
                let connect_packet = self.build_connect_packet(&self.pending_connect_options)?;
                self.enqueue_packet(ControlPacket::Connect(connect_packet))?;
                self.state = ClientState::Connecting;
                Ok(())
            }
            DriverEventIn::SocketClosed => {
                let was_disconnected = self.state == ClientState::Disconnected;
                self.state = ClientState::Disconnected;
                self.next_timeout = None;
                self.reset_negotiated_limits();

                if !was_disconnected {
                    self.read_queue.push_back(UserWriteOut::Disconnected);
                }

                Ok(())
            }
            DriverEventIn::SocketError => {
                self.state = ClientState::Disconnected;
                self.next_timeout = None;
                self.reset_negotiated_limits();
                self.action_queue.push_back(DriverEventOut::CloseSocket);
                Err(Error::ProtocolError)
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_timeout(&mut self, now: Self::Time) -> Result<(), Self::Error> {
        if self.state != ClientState::Connected {
            return Ok(());
        }

        self.enqueue_packet(ControlPacket::PingReq(PingReq {}))?;
        self.next_timeout = Some(now);

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn close(&mut self) -> Result<(), Self::Error> {
        match self.state {
            ClientState::Connected | ClientState::Connecting => {
                let _ = self.enqueue_packet(ControlPacket::Disconnect(Disconnect {
                    reason_code: DisconnectReasonCode::NormalDisconnection,
                    properties: DisconnectProperties::default(),
                }));
                self.action_queue.push_back(DriverEventOut::CloseSocket);
                self.state = ClientState::Disconnected;
                self.next_timeout = None;
                self.reset_negotiated_limits();
                self.read_queue.push_back(UserWriteOut::Disconnected);
                Ok(())
            }
            ClientState::Disconnected => Ok(()),
            ClientState::Start => {
                self.state = ClientState::Disconnected;
                self.next_timeout = None;
                self.reset_negotiated_limits();
                Ok(())
            }
        }
    }

    fn poll_read(&mut self) -> Option<Self::Rout> {
        self.read_queue.pop_front()
    }

    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.write_queue.pop_front()
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.action_queue.pop_front()
    }

    fn poll_timeout(&mut self) -> Option<Self::Time> {
        self.next_timeout
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
    use sansio_mqtt_v5_types::Qos;
    use sansio_mqtt_v5_types::Topic;

    #[test]
    fn build_connect_packet_maps_will_from_connection_options() {
        let will = crate::types::Will {
            topic: Topic::try_from(Utf8String::try_from("topic/will").expect("valid utf8"))
                .expect("valid topic"),
            payload: Payload::from(&b"will payload"[..]),
            payload_format_indicator: Some(FormatIndicator::Utf8),
            message_expiry_interval: Some(Duration::from_secs(42)),
            topic_alias: None,
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
    fn socket_connected_error_does_not_poison_state() {
        let mut client = Client::<u64>::default();
        client.pending_connect_options = ConnectionOptions {
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
        assert_eq!(client.state, ClientState::Start);
        assert_eq!(client.poll_write(), None);
    }
}
