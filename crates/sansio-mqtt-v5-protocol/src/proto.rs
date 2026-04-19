use core::num::NonZero;
use core::time::Duration;

use crate::limits;
use crate::queues;
use crate::scratchpad::{ClientLifecycleState, ClientScratchpad, ConnectingPhase};
use crate::session::{ClientSession, InboundInflightState, OutboundInflightState};
use crate::session_ops;
use crate::types::*;
use alloc::vec::Vec;
use bytes::Bytes;
use bytes::BytesMut;
use sansio::Protocol;
use sansio_mqtt_v5_types::BinaryData;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::Connect;
use sansio_mqtt_v5_types::ConnectProperties;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::Disconnect;
use sansio_mqtt_v5_types::DisconnectProperties;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::GuaranteedQoS;
use sansio_mqtt_v5_types::ParserSettings;
use sansio_mqtt_v5_types::PingReq;
use sansio_mqtt_v5_types::PubAckReasonCode;
use sansio_mqtt_v5_types::PubCompReasonCode;
use sansio_mqtt_v5_types::PubRecReasonCode;
use sansio_mqtt_v5_types::Publish;
use sansio_mqtt_v5_types::PublishKind;
use sansio_mqtt_v5_types::PublishProperties;
use sansio_mqtt_v5_types::Qos;
use sansio_mqtt_v5_types::Subscribe;
use sansio_mqtt_v5_types::SubscribeProperties;
use sansio_mqtt_v5_types::Unsubscribe;
use sansio_mqtt_v5_types::UnsubscribeProperties;
use sansio_mqtt_v5_types::Utf8String;
use sansio_mqtt_v5_types::Will as ConnectWill;
use sansio_mqtt_v5_types::WillProperties;
use winnow::error::ErrMode;
use winnow::stream::Partial;
use winnow::Parser;

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
    pub fn with_settings_and_session(settings: ClientSettings, session: ClientSession) -> Self {
        let mut client = Self {
            settings,
            session,
            scratchpad: ClientScratchpad::default(),
        };
        limits::recompute_effective_limits(&client.settings, &mut client.scratchpad);
        client
    }

    pub fn with_settings(settings: ClientSettings) -> Self {
        Self::with_settings_and_session(settings, Default::default())
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
                receive_maximum: limits::min_option_nonzero_u16(
                    options.receive_maximum,
                    self.settings.max_incoming_receive_maximum,
                ),
                maximum_packet_size: limits::min_option_nonzero_u32(
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
                        limits::recompute_effective_limits(&self.settings, &mut self.scratchpad);
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
                                    queues::fail_protocol_and_disconnect(
                                        &self.settings,
                                        &mut self.session,
                                        &mut self.scratchpad,
                                        DisconnectReasonCode::ProtocolError,
                                    )?;
                                    return Err(Error::ProtocolError);
                                }
                                // [MQTT-4.4.0-1] [MQTT-4.4.0-2] Session Present=1 resumes in-flight QoS transactions and replay path.
                                if session_ops::replay_outbound_inflight_with_dup(
                                    &mut self.session,
                                    &mut self.scratchpad,
                                )
                                .is_err()
                                {
                                    queues::fail_protocol_and_disconnect(
                                        &self.settings,
                                        &mut self.session,
                                        &mut self.scratchpad,
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
                                session_ops::emit_publish_dropped_for_all_inflight(
                                    &self.session,
                                    &mut self.scratchpad,
                                );
                                session_ops::reset_session_state(&mut self.session);
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
                        limits::reset_negotiated_limits(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                        );
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
                        queues::fail_protocol_and_disconnect(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        )?;
                        return Err(Error::ProtocolError);
                    }

                    if !matches!(
                        auth.reason_code,
                        sansio_mqtt_v5_types::AuthReasonCode::ContinueAuthentication
                    ) {
                        queues::fail_protocol_and_disconnect(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        )?;
                        return Err(Error::ProtocolError);
                    }

                    self.scratchpad.connecting_phase = ConnectingPhase::AuthInProgress;
                    Ok(())
                }
                _ => {
                    queues::fail_protocol_and_disconnect(
                        &self.settings,
                        &mut self.session,
                        &mut self.scratchpad,
                        DisconnectReasonCode::ProtocolError,
                    )?;
                    Err(Error::ProtocolError)
                }
            },
            ClientLifecycleState::Connected => match packet {
                ControlPacket::Publish(mut publish) => {
                    if limits::apply_inbound_publish_topic_alias(
                        &mut self.session,
                        &self.scratchpad,
                        &mut publish,
                    )
                    .is_err()
                    {
                        queues::fail_protocol_and_disconnect(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        )?;
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
                                queues::fail_protocol_and_disconnect(
                                    &self.settings,
                                    &mut self.session,
                                    &mut self.scratchpad,
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
                            Some(InboundInflightState::Qos2AwaitPubRel) => {
                                queues::enqueue_pubrec_or_fail_protocol(
                                    &self.settings,
                                    &mut self.session,
                                    &mut self.scratchpad,
                                    packet_id,
                                    PubRecReasonCode::Success,
                                )
                            }
                            Some(InboundInflightState::Qos2AwaitAppDecision) => Ok(()),
                            Some(InboundInflightState::Qos2Rejected(reason_code)) => {
                                queues::enqueue_pubrec_or_fail_protocol(
                                    &self.settings,
                                    &mut self.session,
                                    &mut self.scratchpad,
                                    packet_id,
                                    reason_code,
                                )
                            }
                            Some(InboundInflightState::Qos1AwaitAppDecision) => {
                                queues::fail_protocol_and_disconnect(
                                    &self.settings,
                                    &mut self.session,
                                    &mut self.scratchpad,
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
                            queues::enqueue_pubcomp_or_fail_protocol(
                                &self.settings,
                                &mut self.session,
                                &mut self.scratchpad,
                                packet_id,
                                PubCompReasonCode::Success,
                            )
                        }
                        Some(
                            InboundInflightState::Qos1AwaitAppDecision
                            | InboundInflightState::Qos2AwaitAppDecision,
                        ) => {
                            queues::fail_protocol_and_disconnect(
                                &self.settings,
                                &mut self.session,
                                &mut self.scratchpad,
                                DisconnectReasonCode::ProtocolError,
                            )?;
                            Err(Error::ProtocolError)
                        }
                        Some(InboundInflightState::Qos2Rejected(_)) => {
                            let _ = self.session.on_flight_received.remove(&packet_id);
                            queues::enqueue_pubcomp_or_fail_protocol(
                                &self.settings,
                                &mut self.session,
                                &mut self.scratchpad,
                                packet_id,
                                PubCompReasonCode::PacketIdentifierNotFound,
                            )
                        }
                        None => queues::enqueue_pubcomp_or_fail_protocol(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
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
                            queues::fail_protocol_and_disconnect(
                                &self.settings,
                                &mut self.session,
                                &mut self.scratchpad,
                                DisconnectReasonCode::ProtocolError,
                            )?;
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
                                queues::enqueue_pubrel_or_fail_protocol(
                                    &self.settings,
                                    &mut self.session,
                                    &mut self.scratchpad,
                                    packet_id,
                                )?;

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
                            queues::enqueue_pubrel_or_fail_protocol(
                                &self.settings,
                                &mut self.session,
                                &mut self.scratchpad,
                                packet_id,
                            )?;
                            Ok(())
                        }
                        _ => {
                            queues::fail_protocol_and_disconnect(
                                &self.settings,
                                &mut self.session,
                                &mut self.scratchpad,
                                DisconnectReasonCode::ProtocolError,
                            )?;
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
                            queues::fail_protocol_and_disconnect(
                                &self.settings,
                                &mut self.session,
                                &mut self.scratchpad,
                                DisconnectReasonCode::ProtocolError,
                            )?;
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
                        queues::fail_protocol_and_disconnect(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        )?;
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
                        queues::fail_protocol_and_disconnect(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        )?;
                        return Err(Error::ProtocolError);
                    }
                    Ok(())
                }
                ControlPacket::Disconnect(_) => {
                    self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                    session_ops::reset_keepalive(&mut self.scratchpad);
                    limits::reset_negotiated_limits(
                        &self.settings,
                        &mut self.session,
                        &mut self.scratchpad,
                    );
                    session_ops::maybe_reset_session_state(&mut self.session, &self.scratchpad);
                    self.scratchpad
                        .read_queue
                        .push_back(UserWriteOut::Disconnected);
                    self.scratchpad
                        .action_queue
                        .push_back(DriverEventOut::CloseSocket);
                    Ok(())
                }
                _ => {
                    queues::fail_protocol_and_disconnect(
                        &self.settings,
                        &mut self.session,
                        &mut self.scratchpad,
                        DisconnectReasonCode::ProtocolError,
                    )?;
                    Err(Error::ProtocolError)
                }
            },
            ClientLifecycleState::Start | ClientLifecycleState::Disconnected => {
                queues::fail_protocol_and_disconnect(
                    &self.settings,
                    &mut self.session,
                    &mut self.scratchpad,
                    DisconnectReasonCode::ProtocolError,
                )?;
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
                    queues::fail_protocol_and_disconnect(
                        &self.settings,
                        &mut self.session,
                        &mut self.scratchpad,
                        DisconnectReasonCode::MalformedPacket,
                    )?;

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
                    limits::recompute_effective_limits(&self.settings, &mut self.scratchpad);
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

                limits::validate_outbound_topic_alias(&self.scratchpad, msg.topic_alias)?;
                limits::validate_outbound_publish_capabilities(&self.scratchpad, &msg)?;

                if matches!(msg.qos, Qos::AtLeastOnce | Qos::ExactlyOnce) {
                    // [MQTT-4.9.0-1] Apply peer Receive Maximum before sending QoS1/QoS2 PUBLISH.
                    limits::ensure_outbound_receive_maximum_capacity(
                        &self.session,
                        &self.scratchpad,
                    )?;
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
                        packet_id: session_ops::next_outbound_publish_packet_id(&mut self.session)?,
                        qos: GuaranteedQoS::AtLeastOnce,
                        dup: false,
                    },
                    Qos::ExactlyOnce => PublishKind::Repetible {
                        packet_id: session_ops::next_outbound_publish_packet_id(&mut self.session)?,
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

                queues::enqueue_packet(&mut self.scratchpad, &packet)?;

                if let (PublishKind::Repetible { packet_id, .. }, Some(inflight_state)) =
                    (kind, inflight_state)
                {
                    self.session
                        .on_flight_sent
                        .insert(packet_id, inflight_state);
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
                        queues::enqueue_puback_or_fail_protocol(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            packet_id,
                            PubAckReasonCode::Success,
                        )?;
                        let _ = self.session.on_flight_received.remove(&packet_id);
                        Ok(())
                    }
                    Some(InboundInflightState::Qos2AwaitAppDecision) => {
                        queues::enqueue_pubrec_or_fail_protocol(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            packet_id,
                            PubRecReasonCode::Success,
                        )?;
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
                        queues::enqueue_puback_or_fail_protocol(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            packet_id,
                            Self::map_incoming_reject_reason_to_puback(reason),
                        )?;
                        let _ = self.session.on_flight_received.remove(&packet_id);
                        Ok(())
                    }
                    Some(InboundInflightState::Qos2AwaitAppDecision) => {
                        let reason_code = Self::map_incoming_reject_reason_to_pubrec(reason);
                        queues::enqueue_pubrec_or_fail_protocol(
                            &self.settings,
                            &mut self.session,
                            &mut self.scratchpad,
                            packet_id,
                            reason_code,
                        )?;
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
                let packet_id = session_ops::next_packet_id_checked(&mut self.session)?;

                queues::enqueue_packet(
                    &mut self.scratchpad,
                    &ControlPacket::Subscribe(Subscribe {
                        packet_id,
                        subscription,
                        extra_subscriptions: subscriptions.collect(),
                        properties: SubscribeProperties {
                            subscription_identifier: options.subscription_identifier,
                            user_properties: options.user_properties,
                        },
                    }),
                )?;
                self.session.pending_subscribe.insert(packet_id, ());

                Ok(())
            }
            UserWriteIn::Unsubscribe(options) => {
                if self.scratchpad.lifecycle_state != ClientLifecycleState::Connected {
                    return Err(Error::InvalidStateTransition);
                }
                let packet_id = session_ops::next_packet_id_checked(&mut self.session)?;

                queues::enqueue_packet(
                    &mut self.scratchpad,
                    &ControlPacket::Unsubscribe(Unsubscribe {
                        packet_id,
                        properties: UnsubscribeProperties {
                            user_properties: options.user_properties,
                        },
                        filter: options.filter,
                        extra_filters: options.extra_filters,
                    }),
                )?;
                self.session.pending_unsubscribe.insert(packet_id, ());

                Ok(())
            }
            UserWriteIn::Disconnect => match self.scratchpad.lifecycle_state {
                ClientLifecycleState::Connected | ClientLifecycleState::Connecting => {
                    let _ = queues::enqueue_packet(
                        &mut self.scratchpad,
                        &ControlPacket::Disconnect(Disconnect {
                            reason_code: DisconnectReasonCode::NormalDisconnection,
                            properties: DisconnectProperties::default(),
                        }),
                    );
                    self.scratchpad
                        .action_queue
                        .push_back(DriverEventOut::CloseSocket);
                    self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                    self.scratchpad.read_buffer.clear();
                    session_ops::reset_keepalive(&mut self.scratchpad);
                    limits::reset_negotiated_limits(
                        &self.settings,
                        &mut self.session,
                        &mut self.scratchpad,
                    );
                    session_ops::maybe_reset_session_state(&mut self.session, &self.scratchpad);
                    self.scratchpad
                        .read_queue
                        .push_back(UserWriteOut::Disconnected);
                    Ok(())
                }
                ClientLifecycleState::Disconnected => Ok(()),
                ClientLifecycleState::Start => {
                    self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                    self.scratchpad.read_buffer.clear();
                    session_ops::reset_keepalive(&mut self.scratchpad);
                    limits::reset_negotiated_limits(
                        &self.settings,
                        &mut self.session,
                        &mut self.scratchpad,
                    );
                    session_ops::maybe_reset_session_state(&mut self.session, &self.scratchpad);
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

                limits::reset_negotiated_limits(
                    &self.settings,
                    &mut self.session,
                    &mut self.scratchpad,
                );
                let connect_packet =
                    self.build_connect_packet(&self.scratchpad.pending_connect_options)?;
                queues::enqueue_packet(
                    &mut self.scratchpad,
                    &ControlPacket::Connect(connect_packet),
                )?;
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
                session_ops::reset_keepalive(&mut self.scratchpad);
                limits::reset_negotiated_limits(
                    &self.settings,
                    &mut self.session,
                    &mut self.scratchpad,
                );
                session_ops::maybe_reset_session_state(&mut self.session, &self.scratchpad);

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
                session_ops::reset_keepalive(&mut self.scratchpad);
                limits::reset_negotiated_limits(
                    &self.settings,
                    &mut self.session,
                    &mut self.scratchpad,
                );
                session_ops::maybe_reset_session_state(&mut self.session, &self.scratchpad);
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
            queues::fail_protocol_and_disconnect(
                &self.settings,
                &mut self.session,
                &mut self.scratchpad,
                DisconnectReasonCode::KeepAliveTimeout,
            )?;
            return Err(Error::ProtocolError);
        }

        if self.scratchpad.keep_alive_saw_network_activity {
            // [MQTT-3.1.2-22] Any control packet traffic resets keep-alive idle detection.
            self.scratchpad.keep_alive_saw_network_activity = false;
            self.scratchpad.next_timeout = Some(now);
            return Ok(());
        }

        // [MQTT-3.1.2-22] [MQTT-3.12.4-1] Send PINGREQ when Keep Alive elapses without traffic.
        queues::enqueue_packet(&mut self.scratchpad, &ControlPacket::PingReq(PingReq {}))?;
        self.scratchpad.keep_alive_ping_outstanding = true;
        self.scratchpad.next_timeout = Some(now);

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn close(&mut self) -> Result<(), Self::Error> {
        match self.scratchpad.lifecycle_state {
            ClientLifecycleState::Connected | ClientLifecycleState::Connecting => {
                let _ = queues::enqueue_packet(
                    &mut self.scratchpad,
                    &ControlPacket::Disconnect(Disconnect {
                        reason_code: DisconnectReasonCode::NormalDisconnection,
                        properties: DisconnectProperties::default(),
                    }),
                );
                self.scratchpad
                    .action_queue
                    .push_back(DriverEventOut::CloseSocket);
                self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                self.scratchpad.read_buffer.clear();
                session_ops::reset_keepalive(&mut self.scratchpad);
                limits::reset_negotiated_limits(
                    &self.settings,
                    &mut self.session,
                    &mut self.scratchpad,
                );
                session_ops::maybe_reset_session_state(&mut self.session, &self.scratchpad);
                self.scratchpad
                    .read_queue
                    .push_back(UserWriteOut::Disconnected);
                Ok(())
            }
            ClientLifecycleState::Disconnected => Ok(()),
            ClientLifecycleState::Start => {
                self.scratchpad.lifecycle_state = ClientLifecycleState::Disconnected;
                self.scratchpad.read_buffer.clear();
                session_ops::reset_keepalive(&mut self.scratchpad);
                limits::reset_negotiated_limits(
                    &self.settings,
                    &mut self.session,
                    &mut self.scratchpad,
                );
                session_ops::maybe_reset_session_state(&mut self.session, &self.scratchpad);
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
