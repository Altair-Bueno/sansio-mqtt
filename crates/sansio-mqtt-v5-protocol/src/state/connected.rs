use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::session::InboundInflightState;
use crate::session::OutboundInflightState;
use crate::session_ops;
use crate::state::ClientState;
use crate::state::StateHandler;
use crate::state::disconnected::Disconnected;
use crate::state::fail_with_protocol_error;
use crate::types::BrokerMessage;
use crate::types::ClientMessage;
use crate::types::ClientSettings;
use crate::types::DriverEventIn;
use crate::types::DriverEventOut;
use crate::types::Error;
use crate::types::InboundMessageId;
use crate::types::ProtocolTime;
use crate::types::UserWriteIn;
use crate::types::UserWriteOut;
use alloc::vec::Vec;
use core::num::NonZero;
use core::time::Duration;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::GuaranteedQoS;
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

#[derive(Debug)]
pub(crate) struct Connected;

/// Applies the uniform `Connected` transition rule to a fallible operation.
///
/// Every failure path in this state has already torn the connection down via
/// [`queues::fail_protocol_and_disconnect`], so an error always means the FSM
/// has moved to `Disconnected`.
fn stay_or_disconnect(result: Result<(), Error>) -> (ClientState, Result<(), Error>) {
    match result {
        Ok(()) => (ClientState::Connected(Connected), Ok(())),
        Err(err) => (ClientState::Disconnected(Disconnected), Err(err)),
    }
}

/// Which outbound QoS2 stage `packet_id` is in.
///
/// Extracted as a `Copy` tag so the caller can release its borrow of `session`
/// before enqueueing a response; matching the stored [`OutboundInflightState`]
/// directly would either hold the borrow or clone the retained PUBLISH.
#[derive(Debug, Clone, Copy)]
enum OutboundQos2Stage {
    AwaitPubRec,
    AwaitPubComp,
}

fn outbound_qos2_stage(
    session: &ClientSession,
    packet_id: NonZero<u16>,
) -> Option<OutboundQos2Stage> {
    match session.on_flight_sent.get(&packet_id)? {
        OutboundInflightState::Qos2AwaitPubRec { .. } => Some(OutboundQos2Stage::AwaitPubRec),
        OutboundInflightState::Qos2AwaitPubComp => Some(OutboundQos2Stage::AwaitPubComp),
        OutboundInflightState::Qos1AwaitPubAck { .. } => None,
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

fn handle_inbound_qos1_publish<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    publish: Publish,
) -> Result<(), Error> {
    match session.on_flight_received.get(&packet_id).copied() {
        None => {
            scratchpad.read_queue.push_back(
                UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(
                    InboundMessageId::new(packet_id),
                    map_inbound_publish_to_broker_message(publish),
                ),
            );
            session
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
                settings,
                session,
                scratchpad,
                sansio_mqtt_v5_types::DisconnectReasonCode::ProtocolError,
            );
            Err(Error::ProtocolError)
        }
    }
}

fn handle_inbound_qos2_publish<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    publish: Publish,
) -> Result<(), Error> {
    match session.on_flight_received.get(&packet_id).copied() {
        Some(InboundInflightState::Qos2AwaitPubRel) => queues::enqueue_ack_or_fail_protocol(
            settings,
            session,
            scratchpad,
            &queues::pubrec(packet_id, PubRecReasonCode::Success),
        ),
        Some(InboundInflightState::Qos2AwaitAppDecision) => Ok(()),
        Some(InboundInflightState::Qos2Rejected(reason_code)) => {
            queues::enqueue_ack_or_fail_protocol(
                settings,
                session,
                scratchpad,
                &queues::pubrec(packet_id, reason_code),
            )
        }
        Some(InboundInflightState::Qos1AwaitAppDecision) => {
            queues::fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                sansio_mqtt_v5_types::DisconnectReasonCode::ProtocolError,
            );
            Err(Error::ProtocolError)
        }
        None => {
            scratchpad.read_queue.push_back(
                UserWriteOut::ReceivedMessageWithRequiredAcknowledgement(
                    InboundMessageId::new(packet_id),
                    map_inbound_publish_to_broker_message(publish),
                ),
            );
            session
                .on_flight_received
                .insert(packet_id, InboundInflightState::Qos2AwaitAppDecision);
            Ok(())
        }
    }
}

/// Answers an inbound QoS1/QoS2 PUBLISH that is awaiting the application's
/// accept-or-reject decision.
///
/// Acknowledging and rejecting differ only in the reason codes they carry and
/// in the state a QoS2 exchange moves to, so both share this path.
fn respond_to_inbound_publish<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    puback_reason_code: PubAckReasonCode,
    pubrec_reason_code: PubRecReasonCode,
    qos2_next_state: InboundInflightState,
) -> (ClientState, Result<(), Error>) {
    match session.on_flight_received.get(&packet_id).copied() {
        Some(InboundInflightState::Qos1AwaitAppDecision) => {
            // [MQTT-4.3.2-4] The QoS1 exchange completes once PUBACK is sent.
            let result = queues::enqueue_ack_or_fail_protocol(
                settings,
                session,
                scratchpad,
                &queues::puback(packet_id, puback_reason_code),
            );
            if result.is_ok() {
                let _ = session.on_flight_received.remove(&packet_id);
            }
            stay_or_disconnect(result)
        }
        Some(InboundInflightState::Qos2AwaitAppDecision) => {
            // [MQTT-4.3.3-1] The QoS2 exchange continues: PUBREC now, PUBREL next.
            let result = queues::enqueue_ack_or_fail_protocol(
                settings,
                session,
                scratchpad,
                &queues::pubrec(packet_id, pubrec_reason_code),
            );
            if result.is_ok() {
                session
                    .on_flight_received
                    .insert(packet_id, qos2_next_state);
            }
            stay_or_disconnect(result)
        }
        // The application has already decided on this packet id, or never
        // received it.
        Some(InboundInflightState::Qos2AwaitPubRel | InboundInflightState::Qos2Rejected(_))
        | None => (ClientState::Connected(Connected), Err(Error::ProtocolError)),
    }
}

/// The in-flight entry a QoS1/QoS2 PUBLISH must be retained under until it is
/// acknowledged.
type OutboundInflightEntry = (NonZero<u16>, OutboundInflightState);

/// Builds the outbound PUBLISH and, for QoS1/QoS2, the in-flight entry to
/// retain for retransmission.
fn build_outbound_publish(
    msg: ClientMessage,
    session: &mut ClientSession,
) -> Result<(Publish, Option<OutboundInflightEntry>), Error> {
    let message_expiry_interval = msg
        .message_expiry_interval
        .map(|interval| u32::try_from(interval.as_secs()).map_err(|_| Error::ProtocolError))
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
    // [MQTT-2.2.1-2] Only QoS>0 PUBLISH packets carry a Packet Identifier.
    let kind = match GuaranteedQoS::try_from(msg.qos) {
        Ok(qos) => PublishKind::Repetible {
            packet_id: session_ops::next_packet_id_checked(session)?,
            qos,
            dup: false,
        },
        Err(_) => PublishKind::FireAndForget,
    };
    let publish = Publish {
        kind,
        retain: msg.retain,
        payload: msg.payload,
        topic: msg.topic,
        properties,
    };

    // [MQTT-4.4.0-1] QoS0 is fire-and-forget, so nothing is retained; QoS1/QoS2
    // keep the packet until it is acknowledged.
    let inflight_state = match publish.kind {
        PublishKind::FireAndForget => None,
        PublishKind::Repetible { packet_id, qos, .. } => Some((
            packet_id,
            match qos {
                GuaranteedQoS::AtLeastOnce => OutboundInflightState::Qos1AwaitPubAck {
                    publish: publish.clone(),
                },
                GuaranteedQoS::ExactlyOnce => OutboundInflightState::Qos2AwaitPubRec {
                    publish: publish.clone(),
                },
            },
        )),
    };

    Ok((publish, inflight_state))
}

impl<Time> StateHandler<Time> for Connected
where
    Time: ProtocolTime,
{
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
        _received_at: Time,
    ) -> (ClientState, Result<(), Error>) {
        match packet {
            ControlPacket::Publish(mut publish) => {
                if limits::apply_inbound_publish_topic_alias(session, scratchpad, &mut publish)
                    .is_err()
                {
                    return fail_with_protocol_error(settings, session, scratchpad);
                }

                match publish.kind {
                    PublishKind::FireAndForget => {
                        scratchpad
                            .read_queue
                            .push_back(UserWriteOut::ReceivedMessage(
                                map_inbound_publish_to_broker_message(publish),
                            ));
                        (ClientState::Connected(self), Ok(()))
                    }
                    PublishKind::Repetible {
                        packet_id,
                        qos: GuaranteedQoS::AtLeastOnce,
                        ..
                    } => stay_or_disconnect(handle_inbound_qos1_publish(
                        settings, session, scratchpad, packet_id, publish,
                    )),
                    PublishKind::Repetible {
                        packet_id,
                        qos: GuaranteedQoS::ExactlyOnce,
                        ..
                    } => stay_or_disconnect(handle_inbound_qos2_publish(
                        settings, session, scratchpad, packet_id, publish,
                    )),
                }
            }
            ControlPacket::PubRel(pubrel) => {
                let packet_id = pubrel.packet_id;

                match session.on_flight_received.get(&packet_id).copied() {
                    Some(InboundInflightState::Qos2AwaitPubRel) => {
                        let _ = session.on_flight_received.remove(&packet_id);
                        stay_or_disconnect(queues::enqueue_ack_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            &queues::pubcomp(packet_id, PubCompReasonCode::Success),
                        ))
                    }
                    Some(
                        InboundInflightState::Qos1AwaitAppDecision
                        | InboundInflightState::Qos2AwaitAppDecision,
                    ) => fail_with_protocol_error(settings, session, scratchpad),
                    // [MQTT-3.6.4-1] An unknown or already-rejected Packet
                    // Identifier is answered with PUBCOMP carrying
                    // PacketIdentifierNotFound. `remove` is a no-op when the id
                    // was never tracked.
                    Some(InboundInflightState::Qos2Rejected(_)) | None => {
                        let _ = session.on_flight_received.remove(&packet_id);
                        stay_or_disconnect(queues::enqueue_ack_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            &queues::pubcomp(
                                packet_id,
                                PubCompReasonCode::PacketIdentifierNotFound,
                            ),
                        ))
                    }
                }
            }
            ControlPacket::PubAck(puback) => {
                let packet_id = puback.packet_id;

                match session.on_flight_sent.get(&packet_id) {
                    Some(OutboundInflightState::Qos1AwaitPubAck { .. }) => {
                        // [MQTT-4.3.2-3] QoS1 sender keeps PUBLISH unacknowledged until matching
                        // PUBACK is received.
                        let _ = session.on_flight_sent.remove(&packet_id);
                        scratchpad
                            .read_queue
                            .push_back(UserWriteOut::PublishAcknowledged(
                                packet_id,
                                puback.reason_code,
                            ));
                        (ClientState::Connected(self), Ok(()))
                    }
                    _ => fail_with_protocol_error(settings, session, scratchpad),
                }
            }
            ControlPacket::PubRec(pubrec) => {
                let packet_id = pubrec.packet_id;
                let reason_code = pubrec.reason_code;

                match outbound_qos2_stage(session, packet_id) {
                    Some(OutboundQos2Stage::AwaitPubRec) => {
                        // [MQTT-4.3.3-4] QoS2 sender sends PUBREL with the same Packet Identifier
                        // after PUBREC (Reason Code < 0x80).
                        if matches!(
                            reason_code,
                            PubRecReasonCode::Success | PubRecReasonCode::NoMatchingSubscribers
                        ) {
                            let result = queues::enqueue_ack_or_fail_protocol(
                                settings,
                                session,
                                scratchpad,
                                &queues::pubrel(packet_id),
                            );
                            if result.is_ok() {
                                session
                                    .on_flight_sent
                                    .insert(packet_id, OutboundInflightState::Qos2AwaitPubComp);
                            }
                            stay_or_disconnect(result)
                        } else {
                            let _ = session.on_flight_sent.remove(&packet_id);
                            scratchpad.read_queue.push_back(
                                UserWriteOut::PublishDroppedDueToBrokerRejectedPubRec(
                                    packet_id,
                                    reason_code,
                                ),
                            );
                            (ClientState::Connected(self), Ok(()))
                        }
                    }
                    // [MQTT-4.3.3-4] Repeated PUBREC still requires PUBREL with the same Packet
                    // Identifier.
                    Some(OutboundQos2Stage::AwaitPubComp) => {
                        stay_or_disconnect(queues::enqueue_ack_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            &queues::pubrel(packet_id),
                        ))
                    }
                    None => fail_with_protocol_error(settings, session, scratchpad),
                }
            }
            ControlPacket::PubComp(pubcomp) => {
                let packet_id = pubcomp.packet_id;

                match session.on_flight_sent.get(&packet_id) {
                    Some(OutboundInflightState::Qos2AwaitPubComp) => {
                        // [MQTT-4.3.3-5] QoS2 sender treats PUBREL as unacknowledged until matching
                        // PUBCOMP is received.
                        let _ = session.on_flight_sent.remove(&packet_id);
                        scratchpad
                            .read_queue
                            .push_back(UserWriteOut::PublishCompleted(
                                packet_id,
                                pubcomp.reason_code,
                            ));
                        (ClientState::Connected(self), Ok(()))
                    }
                    _ => fail_with_protocol_error(settings, session, scratchpad),
                }
            }
            ControlPacket::PingResp(_) => {
                // [MQTT-3.12.4-1] PINGRESP answers the outstanding PINGREQ, so the
                // keep-alive watchdog must not treat the connection as dead.
                scratchpad.keep_alive_ping_outstanding = false;
                (ClientState::Connected(self), Ok(()))
            }
            ControlPacket::SubAck(suback) => {
                // [MQTT-3.8.4-1] SUBACK MUST correspond to an outstanding SUBSCRIBE Packet
                // Identifier.
                if session.pending_subscribe.remove(&suback.packet_id) {
                    (ClientState::Connected(self), Ok(()))
                } else {
                    fail_with_protocol_error(settings, session, scratchpad)
                }
            }
            ControlPacket::UnsubAck(unsuback) => {
                // [MQTT-3.10.4-1] UNSUBACK MUST correspond to an outstanding UNSUBSCRIBE Packet
                // Identifier.
                if session.pending_unsubscribe.remove(&unsuback.packet_id) {
                    (ClientState::Connected(self), Ok(()))
                } else {
                    fail_with_protocol_error(settings, session, scratchpad)
                }
            }
            ControlPacket::Disconnect(disconnect) => {
                // [MQTT-4.13.0-1] Forward the server's DISCONNECT reason code to the
                // application so it can distinguish normal server disconnects
                // from error conditions.
                queues::reset_connection_state(settings, session, scratchpad);
                scratchpad
                    .read_queue
                    .push_back(UserWriteOut::Disconnected(Some(disconnect.reason_code)));
                scratchpad
                    .action_queue
                    .push_back(DriverEventOut::CloseSocket);
                (ClientState::Disconnected(Disconnected), Ok(()))
            }
            ControlPacket::Auth(auth) => {
                // [MQTT-4.12.0-2] The server MAY send AUTH at any time after the initial
                // CONNECT to initiate re-authentication. Forward it to the application;
                // the application is responsible for responding with AUTH or DISCONNECT.
                // [MQTT-4.12.0-4] The client MUST respond to an AUTH packet from the server.
                scratchpad.read_queue.push_back(UserWriteOut::Auth(auth));
                (ClientState::Connected(self), Ok(()))
            }
            _ => fail_with_protocol_error(settings, session, scratchpad),
        }
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg: UserWriteIn,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            UserWriteIn::Connect(_) => (
                ClientState::Connected(self),
                Err(Error::InvalidStateTransition),
            ),
            UserWriteIn::PublishMessage(msg) => {
                if let Err(e) = limits::validate_outbound_topic_alias(scratchpad, msg.topic_alias) {
                    return (ClientState::Connected(self), Err(e));
                }
                if let Err(e) = limits::validate_outbound_publish_capabilities(scratchpad, &msg) {
                    return (ClientState::Connected(self), Err(e));
                }

                if matches!(msg.qos, Qos::AtLeastOnce | Qos::ExactlyOnce) {
                    // [MQTT-4.9.0-1] Apply peer Receive Maximum before sending QoS1/QoS2 PUBLISH.
                    if let Err(e) =
                        limits::ensure_outbound_receive_maximum_capacity(session, scratchpad)
                    {
                        return (ClientState::Connected(self), Err(e));
                    }
                }

                let (publish, inflight_state) = match build_outbound_publish(msg, session) {
                    Ok(v) => v,
                    Err(e) => return (ClientState::Connected(self), Err(e)),
                };

                if let Err(e) = queues::enqueue_packet(scratchpad, &ControlPacket::Publish(publish))
                {
                    return (ClientState::Connected(self), Err(e));
                }

                if let Some((packet_id, inflight_state)) = inflight_state {
                    session.on_flight_sent.insert(packet_id, inflight_state);
                }

                (ClientState::Connected(self), Ok(()))
            }
            UserWriteIn::AcknowledgeMessage(inbound_message_id) => respond_to_inbound_publish(
                settings,
                session,
                scratchpad,
                inbound_message_id.get(),
                PubAckReasonCode::Success,
                PubRecReasonCode::Success,
                InboundInflightState::Qos2AwaitPubRel,
            ),
            UserWriteIn::RejectMessage(inbound_message_id, reason) => {
                let pubrec_reason_code = PubRecReasonCode::from(reason);
                respond_to_inbound_publish(
                    settings,
                    session,
                    scratchpad,
                    inbound_message_id.get(),
                    PubAckReasonCode::from(reason),
                    pubrec_reason_code,
                    InboundInflightState::Qos2Rejected(pubrec_reason_code),
                )
            }
            UserWriteIn::Subscribe(options) => {
                if let Err(e) = limits::validate_outbound_subscribe(scratchpad, &options) {
                    return (ClientState::Connected(self), Err(e));
                }

                let packet_id = match session_ops::next_packet_id_checked(session) {
                    Ok(id) => id,
                    Err(e) => return (ClientState::Connected(self), Err(e)),
                };

                match queues::enqueue_packet(
                    scratchpad,
                    &ControlPacket::Subscribe(Subscribe {
                        packet_id,
                        subscription: options.subscription,
                        extra_subscriptions: options.extra_subscriptions,
                        properties: SubscribeProperties {
                            subscription_identifier: options.subscription_identifier,
                            user_properties: options.user_properties,
                        },
                    }),
                ) {
                    Ok(()) => {
                        session.pending_subscribe.insert(packet_id);
                        (ClientState::Connected(self), Ok(()))
                    }
                    Err(e) => (ClientState::Connected(self), Err(e)),
                }
            }
            UserWriteIn::Unsubscribe(options) => {
                let packet_id = match session_ops::next_packet_id_checked(session) {
                    Ok(id) => id,
                    Err(e) => return (ClientState::Connected(self), Err(e)),
                };

                match queues::enqueue_packet(
                    scratchpad,
                    &ControlPacket::Unsubscribe(Unsubscribe {
                        packet_id,
                        properties: UnsubscribeProperties {
                            user_properties: options.user_properties,
                        },
                        filter: options.filter,
                        extra_filters: options.extra_filters,
                    }),
                ) {
                    Ok(()) => {
                        session.pending_unsubscribe.insert(packet_id);
                        (ClientState::Connected(self), Ok(()))
                    }
                    Err(e) => (ClientState::Connected(self), Err(e)),
                }
            }
            // A user-requested disconnect is the same teardown as `close`.
            UserWriteIn::Disconnect => self.close(settings, session, scratchpad),
        }
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        match evt {
            DriverEventIn::SocketConnected => (
                ClientState::Connected(self),
                Err(Error::InvalidStateTransition),
            ),
            DriverEventIn::SocketClosed => {
                queues::reset_connection_state(settings, session, scratchpad);
                scratchpad
                    .read_queue
                    .push_back(UserWriteOut::Disconnected(None));
                (ClientState::Disconnected(Disconnected), Ok(()))
            }
            DriverEventIn::SocketError => {
                queues::reset_connection_state(settings, session, scratchpad);
                scratchpad
                    .action_queue
                    .push_back(DriverEventOut::CloseSocket);
                (
                    ClientState::Disconnected(Disconnected),
                    Err(Error::ProtocolError),
                )
            }
        }
    }

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        now: Time,
    ) -> (ClientState, Result<(), Error>) {
        let Some(interval_secs) = scratchpad.keep_alive_interval_secs else {
            scratchpad.next_timeout = None;
            return (ClientState::Connected(self), Ok(()));
        };

        if scratchpad.keep_alive_ping_outstanding {
            // [MQTT-3.1.2-24] [MQTT-4.13.1-1] Keep Alive timeout closes the network
            // connection. The timer was set to interval/2 after sending
            // PINGREQ, so we have now waited a total of 1.5× the keep-alive
            // interval since the last packet was received.
            queues::fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                sansio_mqtt_v5_types::DisconnectReasonCode::KeepAliveTimeout,
            );
            return (
                ClientState::Disconnected(Disconnected),
                Err(Error::ProtocolError),
            );
        }

        if scratchpad.keep_alive_saw_network_activity {
            // [MQTT-3.1.2-22] Any control packet traffic resets keep-alive idle
            // detection. Schedule the next check one full interval from now.
            scratchpad.keep_alive_saw_network_activity = false;
            scratchpad.arm_keep_alive_deadline(now, u64::from(interval_secs.get()));
            return (ClientState::Connected(self), Ok(()));
        }

        // [MQTT-3.1.2-22] [MQTT-3.12.4-1] Send PINGREQ when Keep Alive elapses without
        // traffic. [MQTT-3.1.2-24] After sending PINGREQ, set the next deadline
        // to interval/2 from now so that the total wait from the last packet is
        // 1.5× the keep-alive interval:   t=0:            last packet / timer
        // start   t=interval:     no traffic → send PINGREQ, set deadline to t
        // + interval/2   t=1.5×interval: no PINGRESP → close connection
        match queues::enqueue_packet(scratchpad, &ControlPacket::PingReq(PingReq {})) {
            Ok(()) => {
                scratchpad.keep_alive_ping_outstanding = true;
                // Use interval/2 (rounding up via integer division rounding) for the
                // half-interval deadline. A minimum of 1 second is enforced so the deadline
                // always advances even for a keep-alive of 1 second.
                let half_interval = (interval_secs.get() / 2).max(1);
                scratchpad.arm_keep_alive_deadline(now, u64::from(half_interval));
                (ClientState::Connected(self), Ok(()))
            }
            Err(e) => (ClientState::Connected(self), Err(e)),
        }
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        queues::graceful_disconnect(settings, session, scratchpad);
        (ClientState::Disconnected(Disconnected), Ok(()))
    }
}
