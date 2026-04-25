use alloc::vec::Vec;
use core::time::Duration;

use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::Disconnect;
use sansio_mqtt_v5_types::DisconnectProperties;
use sansio_mqtt_v5_types::DisconnectReasonCode;
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

use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::session::InboundInflightState;
use crate::session::OutboundInflightState;
use crate::session_ops;
use crate::state::disconnected::Disconnected;
use crate::state::{ClientState, StateHandler};
use crate::types::{
    BrokerMessage, ClientMessage, ClientSettings, DriverEventIn, DriverEventOut, Error,
    InboundMessageId, IncomingRejectReason, InstantAdd, UserWriteIn, UserWriteOut,
};

#[derive(Debug)]
pub(crate) struct Connected;

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

fn handle_inbound_qos1_publish<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: core::num::NonZero<u16>,
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
            let _ = queues::fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                DisconnectReasonCode::ProtocolError,
            );
            Err(Error::ProtocolError)
        }
    }
}

fn handle_inbound_qos2_publish<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: core::num::NonZero<u16>,
    publish: Publish,
) -> Result<(), Error> {
    match session.on_flight_received.get(&packet_id).copied() {
        Some(InboundInflightState::Qos2AwaitPubRel) => queues::enqueue_pubrec_or_fail_protocol(
            settings,
            session,
            scratchpad,
            packet_id,
            PubRecReasonCode::Success,
        ),
        Some(InboundInflightState::Qos2AwaitAppDecision) => Ok(()),
        Some(InboundInflightState::Qos2Rejected(reason_code)) => {
            queues::enqueue_pubrec_or_fail_protocol(
                settings,
                session,
                scratchpad,
                packet_id,
                reason_code,
            )
        }
        Some(InboundInflightState::Qos1AwaitAppDecision) => {
            let _ = queues::fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                DisconnectReasonCode::ProtocolError,
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

fn build_outbound_publish(
    msg: ClientMessage,
    session: &mut ClientSession,
) -> Result<(Publish, Option<OutboundInflightState>), Error> {
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
    let kind = match msg.qos {
        Qos::AtMostOnce => PublishKind::FireAndForget,
        Qos::AtLeastOnce => {
            let packet_id = session_ops::next_outbound_publish_packet_id(session)?;
            PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::AtLeastOnce,
                dup: false,
            }
        }
        Qos::ExactlyOnce => {
            let packet_id = session_ops::next_outbound_publish_packet_id(session)?;
            PublishKind::Repetible {
                packet_id,
                qos: GuaranteedQoS::ExactlyOnce,
                dup: false,
            }
        }
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
    let publish = Publish {
        kind,
        retain: msg.retain,
        payload: msg.payload,
        topic: msg.topic,
        properties,
    };
    Ok((publish, inflight_state))
}

impl<Time: InstantAdd> StateHandler<Time> for Connected {
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        match packet {
            ControlPacket::Publish(mut publish) => {
                if limits::apply_inbound_publish_topic_alias(session, scratchpad, &mut publish)
                    .is_err()
                {
                    let _ = queues::fail_protocol_and_disconnect(
                        settings,
                        session,
                        scratchpad,
                        DisconnectReasonCode::ProtocolError,
                    );
                    return (
                        ClientState::Disconnected(Disconnected),
                        Err(Error::ProtocolError),
                    );
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
                    } => {
                        match handle_inbound_qos1_publish(
                            settings, session, scratchpad, packet_id, publish,
                        ) {
                            Ok(()) => (ClientState::Connected(self), Ok(())),
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                    PublishKind::Repetible {
                        packet_id,
                        qos: GuaranteedQoS::ExactlyOnce,
                        ..
                    } => {
                        match handle_inbound_qos2_publish(
                            settings, session, scratchpad, packet_id, publish,
                        ) {
                            Ok(()) => (ClientState::Connected(self), Ok(())),
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                }
            }
            ControlPacket::PubRel(pubrel) => {
                let packet_id = pubrel.packet_id;

                match session.on_flight_received.get(&packet_id).copied() {
                    Some(InboundInflightState::Qos2AwaitPubRel) => {
                        let _ = session.on_flight_received.remove(&packet_id);
                        let result = queues::enqueue_pubcomp_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            packet_id,
                            PubCompReasonCode::Success,
                        );
                        match result {
                            Ok(()) => (ClientState::Connected(self), Ok(())),
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                    Some(
                        InboundInflightState::Qos1AwaitAppDecision
                        | InboundInflightState::Qos2AwaitAppDecision,
                    ) => {
                        let _ = queues::fail_protocol_and_disconnect(
                            settings,
                            session,
                            scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        );
                        (
                            ClientState::Disconnected(Disconnected),
                            Err(Error::ProtocolError),
                        )
                    }
                    Some(InboundInflightState::Qos2Rejected(_)) => {
                        let _ = session.on_flight_received.remove(&packet_id);
                        let result = queues::enqueue_pubcomp_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            packet_id,
                            PubCompReasonCode::PacketIdentifierNotFound,
                        );
                        match result {
                            Ok(()) => (ClientState::Connected(self), Ok(())),
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                    None => {
                        let result = queues::enqueue_pubcomp_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            packet_id,
                            PubCompReasonCode::PacketIdentifierNotFound,
                        );
                        match result {
                            Ok(()) => (ClientState::Connected(self), Ok(())),
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                }
            }
            ControlPacket::PubAck(puback) => {
                let packet_id = puback.packet_id;
                let reason_code = puback.reason_code;

                match session.on_flight_sent.get(&packet_id) {
                    Some(OutboundInflightState::Qos1AwaitPubAck { .. }) => {
                        // [MQTT-4.3.2-3] QoS1 sender keeps PUBLISH unacknowledged until matching PUBACK is received.
                        let _ = session.on_flight_sent.remove(&packet_id);
                        scratchpad
                            .read_queue
                            .push_back(UserWriteOut::PublishAcknowledged(packet_id, reason_code));
                        (ClientState::Connected(self), Ok(()))
                    }
                    _ => {
                        let _ = queues::fail_protocol_and_disconnect(
                            settings,
                            session,
                            scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        );
                        (
                            ClientState::Disconnected(Disconnected),
                            Err(Error::ProtocolError),
                        )
                    }
                }
            }
            ControlPacket::PubRec(pubrec) => {
                let packet_id = pubrec.packet_id;
                let reason_code = pubrec.reason_code;

                match session.on_flight_sent.get(&packet_id).cloned() {
                    Some(OutboundInflightState::Qos2AwaitPubRec { .. }) => {
                        // [MQTT-4.3.3-4] QoS2 sender sends PUBREL with the same Packet Identifier after PUBREC (Reason Code < 0x80).
                        if matches!(
                            reason_code,
                            PubRecReasonCode::Success | PubRecReasonCode::NoMatchingSubscribers
                        ) {
                            match queues::enqueue_pubrel_or_fail_protocol(
                                settings, session, scratchpad, packet_id,
                            ) {
                                Ok(()) => {
                                    session
                                        .on_flight_sent
                                        .insert(packet_id, OutboundInflightState::Qos2AwaitPubComp);
                                    (ClientState::Connected(self), Ok(()))
                                }
                                Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                            }
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
                    Some(OutboundInflightState::Qos2AwaitPubComp) => {
                        // [MQTT-4.3.3-4] Repeated PUBREC still requires PUBREL with the same Packet Identifier.
                        match queues::enqueue_pubrel_or_fail_protocol(
                            settings, session, scratchpad, packet_id,
                        ) {
                            Ok(()) => (ClientState::Connected(self), Ok(())),
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                    _ => {
                        let _ = queues::fail_protocol_and_disconnect(
                            settings,
                            session,
                            scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        );
                        (
                            ClientState::Disconnected(Disconnected),
                            Err(Error::ProtocolError),
                        )
                    }
                }
            }
            ControlPacket::PubComp(pubcomp) => {
                let packet_id = pubcomp.packet_id;
                let reason_code = pubcomp.reason_code;

                match session.on_flight_sent.get(&packet_id) {
                    Some(OutboundInflightState::Qos2AwaitPubComp) => {
                        // [MQTT-4.3.3-5] QoS2 sender treats PUBREL as unacknowledged until matching PUBCOMP is received.
                        let _ = session.on_flight_sent.remove(&packet_id);
                        scratchpad
                            .read_queue
                            .push_back(UserWriteOut::PublishCompleted(packet_id, reason_code));
                        (ClientState::Connected(self), Ok(()))
                    }
                    _ => {
                        let _ = queues::fail_protocol_and_disconnect(
                            settings,
                            session,
                            scratchpad,
                            DisconnectReasonCode::ProtocolError,
                        );
                        (
                            ClientState::Disconnected(Disconnected),
                            Err(Error::ProtocolError),
                        )
                    }
                }
            }
            ControlPacket::PingResp(_) => (ClientState::Connected(self), Ok(())),
            ControlPacket::SubAck(suback) => {
                // [MQTT-3.8.4-1] SUBACK MUST correspond to an outstanding SUBSCRIBE Packet Identifier.
                if session
                    .pending_subscribe
                    .remove(&suback.packet_id)
                    .is_none()
                {
                    let _ = queues::fail_protocol_and_disconnect(
                        settings,
                        session,
                        scratchpad,
                        DisconnectReasonCode::ProtocolError,
                    );
                    return (
                        ClientState::Disconnected(Disconnected),
                        Err(Error::ProtocolError),
                    );
                }
                (ClientState::Connected(self), Ok(()))
            }
            ControlPacket::UnsubAck(unsuback) => {
                // [MQTT-3.10.4-1] UNSUBACK MUST correspond to an outstanding UNSUBSCRIBE Packet Identifier.
                if session
                    .pending_unsubscribe
                    .remove(&unsuback.packet_id)
                    .is_none()
                {
                    let _ = queues::fail_protocol_and_disconnect(
                        settings,
                        session,
                        scratchpad,
                        DisconnectReasonCode::ProtocolError,
                    );
                    return (
                        ClientState::Disconnected(Disconnected),
                        Err(Error::ProtocolError),
                    );
                }
                (ClientState::Connected(self), Ok(()))
            }
            ControlPacket::Disconnect(disconnect) => {
                // [MQTT-4.13.0-1] Forward the server's DISCONNECT reason code to the application
                // so it can distinguish normal server disconnects from error conditions.
                let reason_code = disconnect.reason_code;
                session_ops::reset_keepalive(scratchpad);
                limits::reset_negotiated_limits(settings, session, scratchpad);
                session_ops::maybe_reset_session_state(session, scratchpad);
                scratchpad
                    .read_queue
                    .push_back(UserWriteOut::Disconnected(Some(reason_code)));
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
            _ => {
                let _ = queues::fail_protocol_and_disconnect(
                    settings,
                    session,
                    scratchpad,
                    DisconnectReasonCode::ProtocolError,
                );
                (
                    ClientState::Disconnected(Disconnected),
                    Err(Error::ProtocolError),
                )
            }
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
                let kind = publish.kind.clone();
                let packet = ControlPacket::Publish(publish);

                if let Err(e) = queues::enqueue_packet(scratchpad, &packet) {
                    return (ClientState::Connected(self), Err(e));
                }

                if let (PublishKind::Repetible { packet_id, .. }, Some(inflight_state)) =
                    (kind, inflight_state)
                {
                    session.on_flight_sent.insert(packet_id, inflight_state);
                }

                (ClientState::Connected(self), Ok(()))
            }
            UserWriteIn::AcknowledgeMessage(inbound_message_id) => {
                let packet_id = inbound_message_id.get();

                match session.on_flight_received.get(&packet_id).copied() {
                    Some(InboundInflightState::Qos1AwaitAppDecision) => {
                        match queues::enqueue_puback_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            packet_id,
                            PubAckReasonCode::Success,
                        ) {
                            Ok(()) => {
                                let _ = session.on_flight_received.remove(&packet_id);
                                (ClientState::Connected(self), Ok(()))
                            }
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                    Some(InboundInflightState::Qos2AwaitAppDecision) => {
                        match queues::enqueue_pubrec_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            packet_id,
                            PubRecReasonCode::Success,
                        ) {
                            Ok(()) => {
                                session
                                    .on_flight_received
                                    .insert(packet_id, InboundInflightState::Qos2AwaitPubRel);
                                (ClientState::Connected(self), Ok(()))
                            }
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                    Some(InboundInflightState::Qos2AwaitPubRel)
                    | Some(InboundInflightState::Qos2Rejected(_))
                    | None => (ClientState::Connected(self), Err(Error::ProtocolError)),
                }
            }
            UserWriteIn::RejectMessage(inbound_message_id, reason) => {
                let packet_id = inbound_message_id.get();

                match session.on_flight_received.get(&packet_id).copied() {
                    Some(InboundInflightState::Qos1AwaitAppDecision) => {
                        match queues::enqueue_puback_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            packet_id,
                            map_incoming_reject_reason_to_puback(reason),
                        ) {
                            Ok(()) => {
                                let _ = session.on_flight_received.remove(&packet_id);
                                (ClientState::Connected(self), Ok(()))
                            }
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                    Some(InboundInflightState::Qos2AwaitAppDecision) => {
                        let reason_code = map_incoming_reject_reason_to_pubrec(reason);
                        match queues::enqueue_pubrec_or_fail_protocol(
                            settings,
                            session,
                            scratchpad,
                            packet_id,
                            reason_code,
                        ) {
                            Ok(()) => {
                                session.on_flight_received.insert(
                                    packet_id,
                                    InboundInflightState::Qos2Rejected(reason_code),
                                );
                                (ClientState::Connected(self), Ok(()))
                            }
                            Err(e) => (ClientState::Disconnected(Disconnected), Err(e)),
                        }
                    }
                    Some(InboundInflightState::Qos2AwaitPubRel)
                    | Some(InboundInflightState::Qos2Rejected(_))
                    | None => (ClientState::Connected(self), Err(Error::ProtocolError)),
                }
            }
            UserWriteIn::Subscribe(options) => {
                if options.subscription_identifier.is_some()
                    && !scratchpad.effective_subscription_identifiers_available
                {
                    return (ClientState::Connected(self), Err(Error::ProtocolError));
                }

                let subscriptions = core::iter::once(options.subscription)
                    .chain(options.extra_subscriptions)
                    .map(|subscription| {
                        let topic_filter_str: &str = subscription.topic_filter.as_ref();
                        let is_shared = topic_filter_str.starts_with("$share/");
                        let has_wildcard =
                            topic_filter_str.contains('+') || topic_filter_str.contains('#');

                        if has_wildcard && !scratchpad.effective_wildcard_subscription_available {
                            return Err(Error::ProtocolError);
                        }

                        if is_shared {
                            if !scratchpad.effective_shared_subscription_available {
                                return Err(Error::ProtocolError);
                            }

                            // [MQTT-3.8.3-4] A Shared Subscription cannot be used with No Local.
                            if subscription.no_local {
                                return Err(Error::ProtocolError);
                            }
                        }

                        Ok(subscription)
                    })
                    .collect::<Result<Vec<_>, Error>>();

                let subscriptions = match subscriptions {
                    Ok(v) => v,
                    Err(e) => return (ClientState::Connected(self), Err(e)),
                };
                let mut subscriptions = subscriptions.into_iter();
                let subscription = match subscriptions.next() {
                    Some(s) => s,
                    None => return (ClientState::Connected(self), Err(Error::ProtocolError)),
                };
                let packet_id = match session_ops::next_packet_id_checked(session) {
                    Ok(id) => id,
                    Err(e) => return (ClientState::Connected(self), Err(e)),
                };

                match queues::enqueue_packet(
                    scratchpad,
                    &ControlPacket::Subscribe(Subscribe {
                        packet_id,
                        subscription,
                        extra_subscriptions: subscriptions.collect(),
                        properties: SubscribeProperties {
                            subscription_identifier: options.subscription_identifier,
                            user_properties: options.user_properties,
                        },
                    }),
                ) {
                    Ok(()) => {
                        session.pending_subscribe.insert(packet_id, ());
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
                        session.pending_unsubscribe.insert(packet_id, ());
                        (ClientState::Connected(self), Ok(()))
                    }
                    Err(e) => (ClientState::Connected(self), Err(e)),
                }
            }
            UserWriteIn::Disconnect => {
                let _ = queues::enqueue_packet(
                    scratchpad,
                    &ControlPacket::Disconnect(Disconnect {
                        reason_code: DisconnectReasonCode::NormalDisconnection,
                        properties: DisconnectProperties::default(),
                    }),
                );
                scratchpad
                    .action_queue
                    .push_back(DriverEventOut::CloseSocket);
                scratchpad.read_buffer.clear();
                session_ops::reset_keepalive(scratchpad);
                limits::reset_negotiated_limits(settings, session, scratchpad);
                session_ops::maybe_reset_session_state(session, scratchpad);
                scratchpad
                    .read_queue
                    .push_back(UserWriteOut::Disconnected(None));
                (ClientState::Disconnected(Disconnected), Ok(()))
            }
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
                scratchpad.read_buffer.clear();
                session_ops::reset_keepalive(scratchpad);
                limits::reset_negotiated_limits(settings, session, scratchpad);
                session_ops::maybe_reset_session_state(session, scratchpad);
                scratchpad
                    .read_queue
                    .push_back(UserWriteOut::Disconnected(None));
                (ClientState::Disconnected(Disconnected), Ok(()))
            }
            DriverEventIn::SocketError => {
                scratchpad.read_buffer.clear();
                session_ops::reset_keepalive(scratchpad);
                limits::reset_negotiated_limits(settings, session, scratchpad);
                session_ops::maybe_reset_session_state(session, scratchpad);
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
            // [MQTT-3.1.2-24] [MQTT-4.13.1-1] Keep Alive timeout closes the network connection.
            // The timer was set to interval/2 after sending PINGREQ, so we have now waited
            // a total of 1.5× the keep-alive interval since the last packet was received.
            let _ = queues::fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                DisconnectReasonCode::KeepAliveTimeout,
            );
            return (
                ClientState::Disconnected(Disconnected),
                Err(Error::ProtocolError),
            );
        }

        // Schedule the next keep-alive check one full interval from now.
        let next_deadline = now.add_secs(interval_secs.get());

        if scratchpad.keep_alive_saw_network_activity {
            // [MQTT-3.1.2-22] Any control packet traffic resets keep-alive idle detection.
            scratchpad.keep_alive_saw_network_activity = false;
            scratchpad.next_timeout = Some(next_deadline);
            return (ClientState::Connected(self), Ok(()));
        }

        // [MQTT-3.1.2-22] [MQTT-3.12.4-1] Send PINGREQ when Keep Alive elapses without traffic.
        // [MQTT-3.1.2-24] After sending PINGREQ, set the next deadline to interval/2 from now
        // so that the total wait from the last packet is 1.5× the keep-alive interval:
        //   t=0:            last packet / timer start
        //   t=interval:     no traffic → send PINGREQ, set deadline to t + interval/2
        //   t=1.5×interval: no PINGRESP → close connection
        match queues::enqueue_packet(scratchpad, &ControlPacket::PingReq(PingReq {})) {
            Ok(()) => {
                scratchpad.keep_alive_ping_outstanding = true;
                // Use interval/2 (rounding up via integer division rounding) for the
                // half-interval deadline. A minimum of 1 second is enforced so the deadline
                // always advances even for a keep-alive of 1 second.
                let half_interval = (interval_secs.get() / 2).max(1);
                scratchpad.next_timeout = Some(now.add_secs(half_interval));
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
        let _ = queues::enqueue_packet(
            scratchpad,
            &ControlPacket::Disconnect(Disconnect {
                reason_code: DisconnectReasonCode::NormalDisconnection,
                properties: DisconnectProperties::default(),
            }),
        );
        scratchpad
            .action_queue
            .push_back(DriverEventOut::CloseSocket);
        scratchpad.read_buffer.clear();
        session_ops::reset_keepalive(scratchpad);
        limits::reset_negotiated_limits(settings, session, scratchpad);
        session_ops::maybe_reset_session_state(session, scratchpad);
        scratchpad
            .read_queue
            .push_back(UserWriteOut::Disconnected(None));
        (ClientState::Disconnected(Disconnected), Ok(()))
    }
}
