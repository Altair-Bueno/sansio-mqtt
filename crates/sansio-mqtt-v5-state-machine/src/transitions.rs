use heapless::Vec;
use sansio_mqtt_v5_contract::{
    Action, DisconnectReason, Input, PublishRequest, Qos, SessionAction, SubscribeRequest, TimerKey,
};

use crate::{Context, MachineState};

pub fn handle(context: &mut Context, state: &mut MachineState, input: Input<'_>) -> Vec<Action, 8> {
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
            if actions.push(Action::SendBytes(disconnect)).is_err() {
                return actions;
            }

            if actions
                .push(Action::CancelTimer(TimerKey::Keepalive))
                .is_err()
            {
                return actions;
            }
            if actions
                .push(Action::CancelTimer(TimerKey::PingRespTimeout))
                .is_err()
            {
                return actions;
            }

            let ack_packet_id = match *state {
                MachineState::WaitingForPubAck { packet_id }
                | MachineState::WaitingForPubRec { packet_id }
                | MachineState::WaitingForPubComp { packet_id }
                | MachineState::WaitingForSubAck { packet_id } => Some(packet_id),
                _ => None,
            };

            if let Some(packet_id) = ack_packet_id {
                if actions
                    .push(Action::CancelTimer(TimerKey::AckTimeout(packet_id)))
                    .is_err()
                {
                    return actions;
                }
            }

            if actions
                .push(Action::SessionAction(SessionAction::Disconnected {
                    reason: DisconnectReason::Normal,
                }))
                .is_err()
            {
                return actions;
            }

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
            if actions
                .push(Action::SessionAction(SessionAction::PublishReceived {
                    topic,
                    payload,
                }))
                .is_err()
            {
                return actions;
            }
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
            if actions.push(Action::SendBytes(disconnect)).is_err() {
                return actions;
            }
            if actions
                .push(Action::SessionAction(SessionAction::Disconnected {
                    reason: DisconnectReason::ProtocolError,
                }))
                .is_err()
            {
                return actions;
            }
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
            if actions
                .push(Action::SendBytes(packet_puback(packet_id)))
                .is_err()
            {
                return actions;
            }
            if actions
                .push(Action::SessionAction(SessionAction::PublishReceived {
                    topic,
                    payload,
                }))
                .is_err()
            {
                return actions;
            }
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
            if actions
                .push(Action::SendBytes(packet_pubrec(packet_id)))
                .is_err()
            {
                return actions;
            }
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
                    if actions
                        .push(Action::SendBytes(packet_pubcomp(packet_id)))
                        .is_err()
                    {
                        return actions;
                    }
                    if actions
                        .push(Action::SessionAction(SessionAction::PublishReceived {
                            topic: pending.topic.clone(),
                            payload: pending.payload.clone(),
                        }))
                        .is_err()
                    {
                        return actions;
                    }
                    context.pending_inbound_qos2 = None;
                }
            }
        }
        (MachineState::Disconnected, Input::UserConnect(connect_options)) => {
            context.set_keepalive_from_connect(connect_options.keep_alive_secs);

            let mut connect_packet = Vec::new();
            if connect_packet.push(0x10).is_err() {
                return actions;
            }
            if connect_packet.push(0x00).is_err() {
                return actions;
            }

            if actions.push(Action::SendBytes(connect_packet)).is_err() {
                return actions;
            }
            if actions
                .push(Action::ScheduleTimer {
                    key: TimerKey::ConnectTimeout,
                    delay_ms: connect_options.connect_timeout_ms,
                })
                .is_err()
            {
                return actions;
            }

            *state = MachineState::Connecting;
        }
        (MachineState::Connecting, Input::PacketConnAck) => {
            if actions
                .push(Action::CancelTimer(TimerKey::ConnectTimeout))
                .is_err()
            {
                return actions;
            }
            if context.keepalive_delay_ms > 0
                && actions
                    .push(Action::ScheduleTimer {
                        key: TimerKey::Keepalive,
                        delay_ms: context.keepalive_delay_ms,
                    })
                    .is_err()
            {
                return actions;
            }
            *state = MachineState::Idle;
        }
        // [MQTT-3.12.4-1] Clients send PINGREQ to indicate liveliness.
        (MachineState::Idle, Input::TimerFired(TimerKey::Keepalive)) => {
            let pingreq = packet_pingreq();
            if actions.push(Action::SendBytes(pingreq)).is_err() {
                return actions;
            }
            if actions
                .push(Action::ScheduleTimer {
                    key: TimerKey::PingRespTimeout,
                    delay_ms: context.pingresp_timeout_ms,
                })
                .is_err()
            {
                return actions;
            }
            *state = MachineState::WaitingForPingResp;
        }
        // [MQTT-3.13.4-1] PINGRESP acknowledges a PINGREQ.
        (MachineState::WaitingForPingResp, Input::PacketPingResp) => {
            if actions
                .push(Action::CancelTimer(TimerKey::PingRespTimeout))
                .is_err()
            {
                return actions;
            }
            if context.keepalive_delay_ms > 0
                && actions
                    .push(Action::ScheduleTimer {
                        key: TimerKey::Keepalive,
                        delay_ms: context.keepalive_delay_ms,
                    })
                    .is_err()
            {
                return actions;
            }
            *state = MachineState::Idle;
        }
        (MachineState::WaitingForPingResp, Input::TimerFired(TimerKey::PingRespTimeout)) => {
            let disconnect = packet_disconnect();
            if actions.push(Action::SendBytes(disconnect)).is_err() {
                return actions;
            }
            if actions
                .push(Action::SessionAction(SessionAction::Disconnected {
                    reason: DisconnectReason::Timeout,
                }))
                .is_err()
            {
                return actions;
            }
            *state = MachineState::Disconnected;
        }
        (MachineState::Idle, Input::UserPublish(publish)) if publish.qos == Qos::AtMost => {
            if let Some(packet) = packet_publish(&publish, None, false) {
                if actions.push(Action::SendBytes(packet)).is_err() {
                    return actions;
                }
            }
        }
        (MachineState::Idle, Input::UserPublish(publish)) if publish.qos == Qos::AtLeast => {
            let packet_id = context.allocate_packet_id();
            if let Some(packet) = packet_publish(&publish, Some(packet_id), false) {
                if actions.push(Action::SendBytes(packet)).is_err() {
                    return actions;
                }
                context.store_pending_qos1(packet_id, &publish);
                if actions
                    .push(Action::ScheduleTimer {
                        key: TimerKey::AckTimeout(packet_id),
                        delay_ms: context.ack_timeout_ms,
                    })
                    .is_err()
                {
                    return actions;
                }
                *state = MachineState::WaitingForPubAck { packet_id };
            }
        }
        (MachineState::Idle, Input::UserPublish(publish)) if publish.qos == Qos::Exactly => {
            let packet_id = context.allocate_packet_id();
            if let Some(packet) = packet_publish(&publish, Some(packet_id), false) {
                if actions.push(Action::SendBytes(packet)).is_err() {
                    return actions;
                }
                context.store_pending_qos2(packet_id, &publish);
                if actions
                    .push(Action::ScheduleTimer {
                        key: TimerKey::AckTimeout(packet_id),
                        delay_ms: context.ack_timeout_ms,
                    })
                    .is_err()
                {
                    return actions;
                }
                *state = MachineState::WaitingForPubRec { packet_id };
            }
        }
        (MachineState::Idle, Input::UserSubscribe(subscribe)) => {
            let packet_id = context.allocate_packet_id();
            if let Some(packet) = packet_subscribe(&subscribe, packet_id, false) {
                if actions.push(Action::SendBytes(packet)).is_err() {
                    return actions;
                }
                context.store_pending_subscribe(packet_id, &subscribe);
                if actions
                    .push(Action::ScheduleTimer {
                        key: TimerKey::AckTimeout(packet_id),
                        delay_ms: context.ack_timeout_ms,
                    })
                    .is_err()
                {
                    return actions;
                }
                *state = MachineState::WaitingForSubAck { packet_id };
            }
        }
        (
            MachineState::WaitingForPubAck { packet_id },
            Input::PacketPubAck { packet_id: ack_id },
        ) if packet_id == &ack_id => {
            if actions
                .push(Action::CancelTimer(TimerKey::AckTimeout(ack_id)))
                .is_err()
            {
                return actions;
            }
            context.pending_qos1 = None;
            *state = MachineState::Idle;
        }
        (
            MachineState::WaitingForPubAck { packet_id },
            Input::TimerFired(TimerKey::AckTimeout(timeout_id)),
        ) if packet_id == &timeout_id => {
            if let Some(pending) = &context.pending_qos1 {
                if let Some(packet) = packet_publish(&pending.publish, Some(*packet_id), true) {
                    if actions.push(Action::SendBytes(packet)).is_err() {
                        return actions;
                    }
                    if actions
                        .push(Action::ScheduleTimer {
                            key: TimerKey::AckTimeout(*packet_id),
                            delay_ms: context.ack_timeout_ms,
                        })
                        .is_err()
                    {
                        return actions;
                    }
                }
            }
        }
        (
            MachineState::WaitingForPubRec { packet_id },
            Input::PacketPubRec { packet_id: rec_id },
        ) if packet_id == &rec_id => {
            if actions
                .push(Action::CancelTimer(TimerKey::AckTimeout(rec_id)))
                .is_err()
            {
                return actions;
            }
            let packet = packet_pubrel(rec_id);
            if actions.push(Action::SendBytes(packet)).is_err() {
                return actions;
            }
            if actions
                .push(Action::ScheduleTimer {
                    key: TimerKey::AckTimeout(rec_id),
                    delay_ms: context.ack_timeout_ms,
                })
                .is_err()
            {
                return actions;
            }
            *state = MachineState::WaitingForPubComp { packet_id: rec_id };
        }
        (
            MachineState::WaitingForPubRec { packet_id },
            Input::TimerFired(TimerKey::AckTimeout(timeout_id)),
        ) if packet_id == &timeout_id => {
            if let Some(pending) = &context.pending_qos2 {
                if let Some(packet) = packet_publish(&pending.publish, Some(*packet_id), true) {
                    if actions.push(Action::SendBytes(packet)).is_err() {
                        return actions;
                    }
                    if actions
                        .push(Action::ScheduleTimer {
                            key: TimerKey::AckTimeout(*packet_id),
                            delay_ms: context.ack_timeout_ms,
                        })
                        .is_err()
                    {
                        return actions;
                    }
                }
            }
        }
        (
            MachineState::WaitingForPubComp { packet_id },
            Input::PacketPubRec { packet_id: rec_id },
        ) if packet_id == &rec_id => {
            let packet = packet_pubrel(rec_id);
            if actions.push(Action::SendBytes(packet)).is_err() {
                return actions;
            }
            if actions
                .push(Action::ScheduleTimer {
                    key: TimerKey::AckTimeout(rec_id),
                    delay_ms: context.ack_timeout_ms,
                })
                .is_err()
            {
                return actions;
            }
        }
        (
            MachineState::WaitingForSubAck { packet_id },
            Input::TimerFired(TimerKey::AckTimeout(timeout_id)),
        ) if packet_id == &timeout_id => {
            if let Some(pending) = &context.pending_subscribe {
                if let Some(packet) = packet_subscribe(&pending.request, *packet_id, true) {
                    if actions.push(Action::SendBytes(packet)).is_err() {
                        return actions;
                    }
                    if actions
                        .push(Action::ScheduleTimer {
                            key: TimerKey::AckTimeout(*packet_id),
                            delay_ms: context.ack_timeout_ms,
                        })
                        .is_err()
                    {
                        return actions;
                    }
                }
            }
        }
        (
            MachineState::WaitingForPubComp { packet_id },
            Input::PacketPubComp { packet_id: comp_id },
        ) if packet_id == &comp_id => {
            if actions
                .push(Action::CancelTimer(TimerKey::AckTimeout(comp_id)))
                .is_err()
            {
                return actions;
            }
            context.pending_qos2 = None;
            *state = MachineState::Idle;
        }
        (
            MachineState::WaitingForPubComp { packet_id },
            Input::TimerFired(TimerKey::AckTimeout(timeout_id)),
        ) if packet_id == &timeout_id => {
            let packet = packet_pubrel(*packet_id);
            if actions.push(Action::SendBytes(packet)).is_err() {
                return actions;
            }
            if actions
                .push(Action::ScheduleTimer {
                    key: TimerKey::AckTimeout(*packet_id),
                    delay_ms: context.ack_timeout_ms,
                })
                .is_err()
            {
                return actions;
            }
        }
        (
            MachineState::WaitingForSubAck { packet_id },
            Input::PacketSubAck {
                packet_id: ack_id,
                reason_codes,
            },
        ) if packet_id == &ack_id => {
            if actions
                .push(Action::CancelTimer(TimerKey::AckTimeout(ack_id)))
                .is_err()
            {
                return actions;
            }
            if actions
                .push(Action::SessionAction(SessionAction::SubscribeAck {
                    packet_id: ack_id,
                    reason_codes,
                }))
                .is_err()
            {
                return actions;
            }
            context.pending_subscribe = None;
            *state = MachineState::Idle;
        }
        _ => {}
    }

    actions
}

fn packet_pingreq() -> Vec<u8, 256> {
    let mut packet = Vec::new();
    let _ = packet.push(0xC0);
    let _ = packet.push(0x00);
    packet
}

fn packet_disconnect() -> Vec<u8, 256> {
    let mut packet = Vec::new();
    let _ = packet.push(0xE0);
    let _ = packet.push(0x00);
    packet
}

fn packet_publish(
    publish: &PublishRequest,
    packet_id: Option<u16>,
    retry: bool,
) -> Option<Vec<u8, 256>> {
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

    push_checked(&mut packet, header)?;
    push_variable_byte_integer(&mut packet, remaining_len)?;
    push_checked(&mut packet, ((topic_len >> 8) & 0xFF) as u8)?;
    push_checked(&mut packet, (topic_len & 0xFF) as u8)?;
    extend_checked(&mut packet, publish.topic.as_bytes())?;

    if let Some(id) = packet_id {
        push_checked(&mut packet, (id >> 8) as u8)?;
        push_checked(&mut packet, (id & 0xFF) as u8)?;
    }

    // [MQTT-3.3.2-1] Property Length is present in MQTT v5 PUBLISH variable header.
    push_checked(&mut packet, 0x00)?;

    extend_checked(&mut packet, publish.payload.as_slice())?;

    Some(packet)
}

fn push_checked(packet: &mut Vec<u8, 256>, byte: u8) -> Option<()> {
    packet.push(byte).ok()
}

fn extend_checked(packet: &mut Vec<u8, 256>, bytes: &[u8]) -> Option<()> {
    for byte in bytes.iter().copied() {
        push_checked(packet, byte)?;
    }
    Some(())
}

fn push_variable_byte_integer(packet: &mut Vec<u8, 256>, value: usize) -> Option<()> {
    for byte in encode_variable_byte_integer(value)?.iter().copied() {
        push_checked(packet, byte)?;
    }
    Some(())
}

fn encode_variable_byte_integer(mut value: usize) -> Option<Vec<u8, 4>> {
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
        encoded.push(byte).ok()?;
        if value == 0 {
            break;
        }
    }

    Some(encoded)
}

fn packet_pubrel(packet_id: u16) -> Vec<u8, 256> {
    let mut packet = Vec::new();
    let _ = packet.push(0x62);
    let _ = packet.push(0x02);
    let _ = packet.push((packet_id >> 8) as u8);
    let _ = packet.push((packet_id & 0xFF) as u8);
    packet
}

fn packet_puback(packet_id: u16) -> Vec<u8, 256> {
    let mut packet = Vec::new();
    let _ = packet.push(0x40);
    let _ = packet.push(0x02);
    let _ = packet.push((packet_id >> 8) as u8);
    let _ = packet.push((packet_id & 0xFF) as u8);
    packet
}

fn packet_pubrec(packet_id: u16) -> Vec<u8, 256> {
    let mut packet = Vec::new();
    let _ = packet.push(0x50);
    let _ = packet.push(0x02);
    let _ = packet.push((packet_id >> 8) as u8);
    let _ = packet.push((packet_id & 0xFF) as u8);
    packet
}

fn packet_pubcomp(packet_id: u16) -> Vec<u8, 256> {
    let mut packet = Vec::new();
    let _ = packet.push(0x70);
    let _ = packet.push(0x02);
    let _ = packet.push((packet_id >> 8) as u8);
    let _ = packet.push((packet_id & 0xFF) as u8);
    packet
}

fn packet_subscribe(
    subscribe: &SubscribeRequest,
    packet_id: u16,
    _retry: bool,
) -> Option<Vec<u8, 256>> {
    let mut packet = Vec::new();

    let topic_len = subscribe.topic_filter.len();
    let remaining_len = 2 + 1 + 2 + topic_len + 1;

    let qos_flags = match subscribe.qos {
        Qos::AtMost => 0x00,
        Qos::AtLeast => 0x01,
        Qos::Exactly => 0x02,
    };

    push_checked(&mut packet, 0x82)?;
    push_variable_byte_integer(&mut packet, remaining_len)?;
    push_checked(&mut packet, (packet_id >> 8) as u8)?;
    push_checked(&mut packet, (packet_id & 0xFF) as u8)?;

    // [MQTT-3.8.2.1-1] SUBSCRIBE Properties length appears in MQTT v5.
    push_checked(&mut packet, 0x00)?;

    push_checked(&mut packet, ((topic_len >> 8) & 0xFF) as u8)?;
    push_checked(&mut packet, (topic_len & 0xFF) as u8)?;
    extend_checked(&mut packet, subscribe.topic_filter.as_bytes())?;
    push_checked(&mut packet, qos_flags)?;

    Some(packet)
}
