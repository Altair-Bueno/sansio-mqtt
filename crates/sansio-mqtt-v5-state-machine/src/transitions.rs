use alloc::vec::Vec;
use sansio_mqtt_v5_contract::{
    Action, DisconnectReason, Input, PublishRequest, Qos, SessionAction, SubscribeRequest, TimerKey,
};

use crate::{Context, MachineState};

pub fn handle(context: &mut Context, state: &mut MachineState, input: Input<'_>) -> Vec<Action> {
    let mut actions = Vec::new();

    match (&*state, input) {
        (
            MachineState::Idle
            | MachineState::WaitingForPingResp
            | MachineState::WaitingForPubAck { .. }
            | MachineState::WaitingForPubRec { .. }
            | MachineState::WaitingForPubComp { .. }
            | MachineState::WaitingForSubAck { .. },
            Input::UserDisconnect,
        ) => {
            let disconnect = packet_disconnect();
            actions.push(Action::SendBytes(disconnect));

            actions.push(Action::CancelTimer(TimerKey::Keepalive));
            actions.push(Action::CancelTimer(TimerKey::PingRespTimeout));

            let ack_packet_id = match *state {
                MachineState::WaitingForPubAck { packet_id }
                | MachineState::WaitingForPubRec { packet_id }
                | MachineState::WaitingForPubComp { packet_id }
                | MachineState::WaitingForSubAck { packet_id } => Some(packet_id),
                _ => None,
            };

            if let Some(packet_id) = ack_packet_id {
                actions.push(Action::CancelTimer(TimerKey::AckTimeout(packet_id)));
            }

            actions.push(Action::SessionAction(SessionAction::Disconnected {
                reason: DisconnectReason::Normal,
            }));

            context.pending_qos1 = None;
            context.pending_qos2 = None;
            context.pending_subscribe = None;
            context.pending_inbound_qos2 = None;
            *state = MachineState::Disconnected;
        }
        (
            MachineState::Idle
            | MachineState::WaitingForPingResp
            | MachineState::WaitingForPubAck { .. }
            | MachineState::WaitingForPubRec { .. }
            | MachineState::WaitingForPubComp { .. }
            | MachineState::WaitingForSubAck { .. },
            Input::PacketPublish {
                topic,
                payload,
                qos: Qos::AtMost,
                ..
            },
        ) => {
            actions.push(Action::SessionAction(SessionAction::PublishReceived {
                topic,
                payload,
            }));
        }
        (
            MachineState::Idle
            | MachineState::WaitingForPingResp
            | MachineState::WaitingForPubAck { .. }
            | MachineState::WaitingForPubRec { .. }
            | MachineState::WaitingForPubComp { .. }
            | MachineState::WaitingForSubAck { .. },
            Input::PacketPublish {
                qos: Qos::AtLeast,
                packet_id: None,
                ..
            }
            | Input::PacketPublish {
                qos: Qos::Exactly,
                packet_id: None,
                ..
            },
        ) => {
            let disconnect = packet_disconnect();
            actions.push(Action::SendBytes(disconnect));
            actions.push(Action::SessionAction(SessionAction::Disconnected {
                reason: DisconnectReason::ProtocolError,
            }));
            *state = MachineState::Disconnected;
        }
        (
            MachineState::Idle
            | MachineState::WaitingForPingResp
            | MachineState::WaitingForPubAck { .. }
            | MachineState::WaitingForPubRec { .. }
            | MachineState::WaitingForPubComp { .. }
            | MachineState::WaitingForSubAck { .. },
            Input::PacketPublish {
                topic,
                payload,
                qos: Qos::AtLeast,
                packet_id: Some(packet_id),
            },
        ) => {
            actions.push(Action::SendBytes(packet_puback(packet_id)));
            actions.push(Action::SessionAction(SessionAction::PublishReceived {
                topic,
                payload,
            }));
        }
        (
            MachineState::Idle
            | MachineState::WaitingForPingResp
            | MachineState::WaitingForPubAck { .. }
            | MachineState::WaitingForPubRec { .. }
            | MachineState::WaitingForPubComp { .. }
            | MachineState::WaitingForSubAck { .. },
            Input::PacketPublish {
                topic,
                payload,
                qos: Qos::Exactly,
                packet_id: Some(packet_id),
            },
        ) => {
            actions.push(Action::SendBytes(packet_pubrec(packet_id)));
            context.store_pending_inbound_qos2(packet_id, &topic, &payload);
        }
        (
            MachineState::Idle
            | MachineState::WaitingForPingResp
            | MachineState::WaitingForPubAck { .. }
            | MachineState::WaitingForPubRec { .. }
            | MachineState::WaitingForPubComp { .. }
            | MachineState::WaitingForSubAck { .. },
            Input::PacketPubRel { packet_id },
        ) => {
            if let Some(pending) = &context.pending_inbound_qos2 {
                if pending.packet_id == packet_id {
                    actions.push(Action::SendBytes(packet_pubcomp(packet_id)));
                    actions.push(Action::SessionAction(SessionAction::PublishReceived {
                        topic: pending.topic.clone(),
                        payload: pending.payload.clone(),
                    }));
                    context.pending_inbound_qos2 = None;
                }
            }
        }
        (MachineState::Disconnected, Input::UserConnect(connect_options)) => {
            context.set_keepalive_from_connect(connect_options.keep_alive_secs);

            actions.push(Action::SendBytes(Vec::from([0x10, 0x00])));
            actions.push(Action::ScheduleTimer {
                key: TimerKey::ConnectTimeout,
                delay_ms: connect_options.connect_timeout_ms,
            });

            *state = MachineState::Connecting;
        }
        (MachineState::Connecting, Input::PacketConnAck) => {
            actions.push(Action::CancelTimer(TimerKey::ConnectTimeout));
            if context.keepalive_delay_ms > 0 {
                actions.push(Action::ScheduleTimer {
                    key: TimerKey::Keepalive,
                    delay_ms: context.keepalive_delay_ms,
                });
            }
            *state = MachineState::Idle;
        }
        // [MQTT-3.12.4-1] Clients send PINGREQ to indicate liveliness.
        (MachineState::Idle, Input::TimerFired(TimerKey::Keepalive)) => {
            let pingreq = packet_pingreq();
            actions.push(Action::SendBytes(pingreq));
            actions.push(Action::ScheduleTimer {
                key: TimerKey::PingRespTimeout,
                delay_ms: context.pingresp_timeout_ms,
            });
            *state = MachineState::WaitingForPingResp;
        }
        // [MQTT-3.13.4-1] PINGRESP acknowledges a PINGREQ.
        (MachineState::WaitingForPingResp, Input::PacketPingResp) => {
            actions.push(Action::CancelTimer(TimerKey::PingRespTimeout));
            if context.keepalive_delay_ms > 0 {
                actions.push(Action::ScheduleTimer {
                    key: TimerKey::Keepalive,
                    delay_ms: context.keepalive_delay_ms,
                });
            }
            *state = MachineState::Idle;
        }
        (MachineState::WaitingForPingResp, Input::TimerFired(TimerKey::PingRespTimeout)) => {
            let disconnect = packet_disconnect();
            actions.push(Action::SendBytes(disconnect));
            actions.push(Action::SessionAction(SessionAction::Disconnected {
                reason: DisconnectReason::Timeout,
            }));
            *state = MachineState::Disconnected;
        }
        (MachineState::Idle, Input::UserPublish(publish)) if publish.qos == Qos::AtMost => {
            if let Some(packet) = packet_publish(&publish, None, false) {
                actions.push(Action::SendBytes(packet));
            }
        }
        (MachineState::Idle, Input::UserPublish(publish)) if publish.qos == Qos::AtLeast => {
            let packet_id = context.allocate_packet_id();
            if let Some(packet) = packet_publish(&publish, Some(packet_id), false) {
                actions.push(Action::SendBytes(packet));
                context.store_pending_qos1(packet_id, &publish);
                actions.push(Action::ScheduleTimer {
                    key: TimerKey::AckTimeout(packet_id),
                    delay_ms: context.ack_timeout_ms,
                });
                *state = MachineState::WaitingForPubAck { packet_id };
            }
        }
        (MachineState::Idle, Input::UserPublish(publish)) if publish.qos == Qos::Exactly => {
            let packet_id = context.allocate_packet_id();
            if let Some(packet) = packet_publish(&publish, Some(packet_id), false) {
                actions.push(Action::SendBytes(packet));
                context.store_pending_qos2(packet_id, &publish);
                actions.push(Action::ScheduleTimer {
                    key: TimerKey::AckTimeout(packet_id),
                    delay_ms: context.ack_timeout_ms,
                });
                *state = MachineState::WaitingForPubRec { packet_id };
            }
        }
        (MachineState::Idle, Input::UserSubscribe(subscribe)) => {
            let packet_id = context.allocate_packet_id();
            if let Some(packet) = packet_subscribe(&subscribe, packet_id, false) {
                actions.push(Action::SendBytes(packet));
                context.store_pending_subscribe(packet_id, &subscribe);
                actions.push(Action::ScheduleTimer {
                    key: TimerKey::AckTimeout(packet_id),
                    delay_ms: context.ack_timeout_ms,
                });
                *state = MachineState::WaitingForSubAck { packet_id };
            }
        }
        (
            MachineState::WaitingForPubAck { packet_id },
            Input::PacketPubAck { packet_id: ack_id },
        ) if packet_id == &ack_id => {
            actions.push(Action::CancelTimer(TimerKey::AckTimeout(ack_id)));
            context.pending_qos1 = None;
            *state = MachineState::Idle;
        }
        (
            MachineState::WaitingForPubAck { packet_id },
            Input::TimerFired(TimerKey::AckTimeout(timeout_id)),
        ) if packet_id == &timeout_id => {
            if let Some(pending) = &context.pending_qos1 {
                if let Some(packet) = packet_publish(&pending.publish, Some(*packet_id), true) {
                    actions.push(Action::SendBytes(packet));
                    actions.push(Action::ScheduleTimer {
                        key: TimerKey::AckTimeout(*packet_id),
                        delay_ms: context.ack_timeout_ms,
                    });
                }
            }
        }
        (
            MachineState::WaitingForPubRec { packet_id },
            Input::PacketPubRec { packet_id: rec_id },
        ) if packet_id == &rec_id => {
            actions.push(Action::CancelTimer(TimerKey::AckTimeout(rec_id)));
            let packet = packet_pubrel(rec_id);
            actions.push(Action::SendBytes(packet));
            actions.push(Action::ScheduleTimer {
                key: TimerKey::AckTimeout(rec_id),
                delay_ms: context.ack_timeout_ms,
            });
            *state = MachineState::WaitingForPubComp { packet_id: rec_id };
        }
        (
            MachineState::WaitingForPubRec { packet_id },
            Input::TimerFired(TimerKey::AckTimeout(timeout_id)),
        ) if packet_id == &timeout_id => {
            if let Some(pending) = &context.pending_qos2 {
                if let Some(packet) = packet_publish(&pending.publish, Some(*packet_id), true) {
                    actions.push(Action::SendBytes(packet));
                    actions.push(Action::ScheduleTimer {
                        key: TimerKey::AckTimeout(*packet_id),
                        delay_ms: context.ack_timeout_ms,
                    });
                }
            }
        }
        (
            MachineState::WaitingForPubComp { packet_id },
            Input::PacketPubRec { packet_id: rec_id },
        ) if packet_id == &rec_id => {
            let packet = packet_pubrel(rec_id);
            actions.push(Action::SendBytes(packet));
            actions.push(Action::ScheduleTimer {
                key: TimerKey::AckTimeout(rec_id),
                delay_ms: context.ack_timeout_ms,
            });
        }
        (
            MachineState::WaitingForSubAck { packet_id },
            Input::TimerFired(TimerKey::AckTimeout(timeout_id)),
        ) if packet_id == &timeout_id => {
            if let Some(pending) = &context.pending_subscribe {
                if let Some(packet) = packet_subscribe(&pending.request, *packet_id, true) {
                    actions.push(Action::SendBytes(packet));
                    actions.push(Action::ScheduleTimer {
                        key: TimerKey::AckTimeout(*packet_id),
                        delay_ms: context.ack_timeout_ms,
                    });
                }
            }
        }
        (
            MachineState::WaitingForPubComp { packet_id },
            Input::PacketPubComp { packet_id: comp_id },
        ) if packet_id == &comp_id => {
            actions.push(Action::CancelTimer(TimerKey::AckTimeout(comp_id)));
            context.pending_qos2 = None;
            *state = MachineState::Idle;
        }
        (
            MachineState::WaitingForPubComp { packet_id },
            Input::TimerFired(TimerKey::AckTimeout(timeout_id)),
        ) if packet_id == &timeout_id => {
            let packet = packet_pubrel(*packet_id);
            actions.push(Action::SendBytes(packet));
            actions.push(Action::ScheduleTimer {
                key: TimerKey::AckTimeout(*packet_id),
                delay_ms: context.ack_timeout_ms,
            });
        }
        (
            MachineState::WaitingForSubAck { packet_id },
            Input::PacketSubAck {
                packet_id: ack_id,
                reason_codes,
            },
        ) if packet_id == &ack_id => {
            actions.push(Action::CancelTimer(TimerKey::AckTimeout(ack_id)));
            actions.push(Action::SessionAction(SessionAction::SubscribeAck {
                packet_id: ack_id,
                reason_codes,
            }));
            context.pending_subscribe = None;
            *state = MachineState::Idle;
        }
        _ => {}
    }

    actions
}

fn packet_pingreq() -> Vec<u8> {
    Vec::from([0xC0, 0x00])
}

fn packet_disconnect() -> Vec<u8> {
    Vec::from([0xE0, 0x00])
}

fn packet_publish(
    publish: &PublishRequest,
    packet_id: Option<u16>,
    retry: bool,
) -> Option<Vec<u8>> {
    let mut packet = Vec::new();

    let header = match (publish.qos, retry) {
        (Qos::AtMost, _) => 0x30,
        (Qos::AtLeast, false) => 0x32,
        (Qos::AtLeast, true) => 0x3A,
        (Qos::Exactly, false) => 0x34,
        (Qos::Exactly, true) => 0x3C,
    };
    let topic_len = publish.topic.len();
    let packet_id_len = if packet_id.is_some() { 2 } else { 0 };
    let remaining_len = 2 + topic_len + packet_id_len + 1 + publish.payload.len();

    packet.push(header);
    push_variable_byte_integer(&mut packet, remaining_len)?;
    packet.push(((topic_len >> 8) & 0xFF) as u8);
    packet.push((topic_len & 0xFF) as u8);
    packet.extend_from_slice(publish.topic.as_bytes());

    if let Some(id) = packet_id {
        packet.push((id >> 8) as u8);
        packet.push((id & 0xFF) as u8);
    }

    // [MQTT-3.3.2-1] Property Length is present in MQTT v5 PUBLISH variable header.
    packet.push(0x00);

    packet.extend_from_slice(publish.payload.as_slice());

    Some(packet)
}

fn push_variable_byte_integer(packet: &mut Vec<u8>, value: usize) -> Option<()> {
    packet.extend(encode_variable_byte_integer(value)?);
    Some(())
}

fn encode_variable_byte_integer(mut value: usize) -> Option<Vec<u8>> {
    if value > 268_435_455 {
        return None;
    }

    let mut encoded = Vec::new();
    loop {
        let mut byte = (value % 128) as u8;
        value /= 128;
        if value > 0 {
            byte |= 0x80;
        }
        encoded.push(byte);
        if value == 0 {
            break;
        }
    }

    Some(encoded)
}

fn packet_pubrel(packet_id: u16) -> Vec<u8> {
    Vec::from([0x62, 0x02, (packet_id >> 8) as u8, (packet_id & 0xFF) as u8])
}

fn packet_puback(packet_id: u16) -> Vec<u8> {
    Vec::from([0x40, 0x02, (packet_id >> 8) as u8, (packet_id & 0xFF) as u8])
}

fn packet_pubrec(packet_id: u16) -> Vec<u8> {
    Vec::from([0x50, 0x02, (packet_id >> 8) as u8, (packet_id & 0xFF) as u8])
}

fn packet_pubcomp(packet_id: u16) -> Vec<u8> {
    Vec::from([0x70, 0x02, (packet_id >> 8) as u8, (packet_id & 0xFF) as u8])
}

fn packet_subscribe(subscribe: &SubscribeRequest, packet_id: u16, _retry: bool) -> Option<Vec<u8>> {
    let mut packet = Vec::new();

    let topic_len = subscribe.topic_filter.len();
    let remaining_len = 2 + 1 + 2 + topic_len + 1;

    let qos_flags = match subscribe.qos {
        Qos::AtMost => 0x00,
        Qos::AtLeast => 0x01,
        Qos::Exactly => 0x02,
    };

    packet.push(0x82);
    push_variable_byte_integer(&mut packet, remaining_len)?;
    packet.push((packet_id >> 8) as u8);
    packet.push((packet_id & 0xFF) as u8);

    // [MQTT-3.8.2.1-1] SUBSCRIBE Properties length appears in MQTT v5.
    packet.push(0x00);

    packet.push(((topic_len >> 8) & 0xFF) as u8);
    packet.push((topic_len & 0xFF) as u8);
    packet.extend_from_slice(subscribe.topic_filter.as_bytes());
    packet.push(qos_flags);

    Some(packet)
}
