use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::session::OutboundInflightState;
use crate::types::Error;
use crate::types::UserWriteOut;
use core::num::NonZero;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::PubRel;
use sansio_mqtt_v5_types::PubRelProperties;
use sansio_mqtt_v5_types::PubRelReasonCode;
use sansio_mqtt_v5_types::PublishKind;

/// Resets all keep-alive fields on the scratchpad.
///
/// [MQTT-3.1.2-22] [MQTT-3.1.2-23] Keep Alive tracking resets on connection
/// lifecycle boundaries.
pub(crate) fn reset_keepalive<Time>(scratchpad: &mut ClientScratchpad<Time>) {
    scratchpad.keep_alive_interval_secs = None;
    scratchpad.keep_alive_saw_network_activity = false;
    scratchpad.keep_alive_ping_outstanding = false;
    scratchpad.next_timeout = None;
}

/// Clears session inflight/pending state if `session_should_persist` is false.
///
/// [MQTT-3.1.2-4] Clean Start controls whether prior session state is
/// discarded.
pub(crate) fn maybe_reset_session_state<Time>(
    session: &mut ClientSession,
    scratchpad: &ClientScratchpad<Time>,
) {
    if !scratchpad.session_should_persist {
        reset_session_state(session);
    }
}

/// Unconditionally clears all session inflight/pending maps.
pub(crate) fn reset_session_state(session: &mut ClientSession) {
    session.on_flight_sent.clear();
    session.on_flight_received.clear();
    session.pending_subscribe.clear();
    session.pending_unsubscribe.clear();
}

/// Advances and returns the packet id counter, wrapping from u16::MAX back to
/// 1.
///
/// [MQTT-2.2.1-3] A Packet Identifier of 0 is invalid, so the counter wraps to
/// 1 rather than overflowing.
pub(crate) fn next_packet_id(session: &mut ClientSession) -> NonZero<u16> {
    let packet_id = session.next_packet_id;
    session.next_packet_id = packet_id.checked_add(1).unwrap_or(NonZero::<u16>::MIN);

    packet_id
}

/// Loops to find a packet id that is not already claimed by an in-flight
/// PUBLISH, SUBSCRIBE, or UNSUBSCRIBE exchange.
///
/// [MQTT-2.2.1-2] Packet Identifier MUST be unused while an exchange is
/// in-flight.
pub(crate) fn next_packet_id_checked(session: &mut ClientSession) -> Result<NonZero<u16>, Error> {
    for _ in 0..u16::MAX {
        let packet_id = next_packet_id(session);
        if !session.on_flight_sent.contains_key(&packet_id)
            && !session.pending_subscribe.contains(&packet_id)
            && !session.pending_unsubscribe.contains(&packet_id)
        {
            return Ok(packet_id);
        }
    }

    Err(Error::ReceiveMaximumExceeded)
}

/// Pushes `UserWriteOut::PublishDroppedDueToSessionNotResumed` for every
/// in-flight packet.
pub(crate) fn emit_publish_dropped_for_all_inflight<Time>(
    session: &ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    for packet_id in session.on_flight_sent.keys().copied() {
        scratchpad
            .read_queue
            .push_back(UserWriteOut::PublishDroppedDueToSessionNotResumed(
                packet_id,
            ));
    }
}

/// Retransmits unacknowledged QoS1/QoS2 PUBLISH with DUP=1 on session resume.
///
/// [MQTT-4.4.0-1] [MQTT-4.4.0-2] On session resume, retransmit unacknowledged
/// QoS1/QoS2 PUBLISH with DUP=1.
pub(crate) fn replay_outbound_inflight_with_dup<Time>(
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) -> Result<(), Error> {
    // `session` and `scratchpad` are distinct borrows, so the retained packets
    // can be marked and re-enqueued in place — no copy of the in-flight map.
    for (packet_id, state) in session.on_flight_sent.iter_mut() {
        match state {
            OutboundInflightState::Qos1AwaitPubAck { publish }
            | OutboundInflightState::Qos2AwaitPubRec { publish } => {
                if let PublishKind::Repetible { dup, .. } = &mut publish.kind {
                    *dup = true;
                }
                crate::queues::enqueue_packet(
                    scratchpad,
                    &ControlPacket::Publish(publish.clone()),
                )?;
            }
            OutboundInflightState::Qos2AwaitPubComp => {
                crate::queues::enqueue_packet(
                    scratchpad,
                    &ControlPacket::PubRel(PubRel {
                        packet_id: *packet_id,
                        reason_code: PubRelReasonCode::Success,
                        properties: PubRelProperties::default(),
                    }),
                )?;
            }
        }
    }

    Ok(())
}
