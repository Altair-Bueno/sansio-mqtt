use alloc::vec::Vec;
use bytes::Bytes;
use core::num::NonZero;
use encode::Encodable;
use sansio_mqtt_v5_types::{
    ControlPacket, Disconnect, DisconnectProperties, DisconnectReasonCode, EncodeError, PubAck,
    PubAckProperties, PubAckReasonCode, PubComp, PubCompProperties, PubCompReasonCode, PubRec,
    PubRecProperties, PubRecReasonCode, PubRel, PubRelProperties, PubRelReasonCode,
};

use crate::proto::{ClientLifecycleState, ClientScratchpad, ClientSession};
use crate::types::{ClientSettings, DriverEventOut, Error};

pub(crate) fn encode_control_packet(packet: &ControlPacket) -> Result<Bytes, Error> {
    let mut encoded = Vec::new();
    packet.encode(&mut encoded).map_err(|err| match err {
        EncodeError::PacketTooLarge(_) => Error::PacketTooLarge,
        _ => Error::EncodeFailure,
    })?;
    Ok(Bytes::from(encoded))
}

pub(crate) fn enqueue_packet<Time: 'static>(
    scratchpad: &mut ClientScratchpad<Time>,
    packet: &ControlPacket,
) -> Result<(), Error> {
    let encoded = encode_control_packet(packet)?;
    crate::limits::validate_outbound_packet_size(scratchpad, encoded.len())?;
    scratchpad.write_queue.push_back(encoded);
    scratchpad.keep_alive_saw_network_activity = true;
    Ok(())
}

/// Enqueues DISCONNECT best-effort, closes socket, transitions lifecycle to Disconnected,
/// and resets keepalive + negotiated limits + session state.
///
/// NOTE: During Tasks 4–11 this function still sets scratchpad.lifecycle_state directly.
/// In Task 12 (FSM cutover) that line is removed and callers return ClientState::Disconnected.
pub(crate) fn fail_protocol_and_disconnect<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    reason: DisconnectReasonCode,
) -> Result<(), Error> {
    let _ = enqueue_packet(
        scratchpad,
        &ControlPacket::Disconnect(Disconnect {
            reason_code: reason,
            properties: DisconnectProperties::default(),
        }),
    );
    scratchpad
        .action_queue
        .push_back(DriverEventOut::CloseSocket);
    scratchpad.lifecycle_state = ClientLifecycleState::Disconnected; // removed in Task 12
    scratchpad.read_buffer.clear();
    crate::session_ops::reset_keepalive(scratchpad);
    // reset negotiated limits (also clears inbound topic aliases)
    crate::limits::reset_negotiated_limits(settings, session, scratchpad);
    crate::session_ops::maybe_reset_session_state(session, scratchpad);
    Ok(())
}

pub(crate) fn enqueue_pubrel_or_fail_protocol<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
) -> Result<(), Error> {
    match enqueue_packet(
        scratchpad,
        &ControlPacket::PubRel(PubRel {
            packet_id,
            reason_code: PubRelReasonCode::Success,
            properties: PubRelProperties::default(),
        }),
    ) {
        Ok(()) => Ok(()),
        Err(_) => {
            fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                DisconnectReasonCode::ProtocolError,
            )?;
            Err(Error::ProtocolError)
        }
    }
}

pub(crate) fn enqueue_puback_or_fail_protocol<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    reason_code: PubAckReasonCode,
) -> Result<(), Error> {
    match enqueue_packet(
        scratchpad,
        &ControlPacket::PubAck(PubAck {
            packet_id,
            reason_code,
            properties: PubAckProperties::default(),
        }),
    ) {
        Ok(()) => Ok(()),
        Err(_) => {
            fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                DisconnectReasonCode::ProtocolError,
            )?;
            Err(Error::ProtocolError)
        }
    }
}

pub(crate) fn enqueue_pubrec_or_fail_protocol<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    reason_code: PubRecReasonCode,
) -> Result<(), Error> {
    match enqueue_packet(
        scratchpad,
        &ControlPacket::PubRec(PubRec {
            packet_id,
            reason_code,
            properties: PubRecProperties::default(),
        }),
    ) {
        Ok(()) => Ok(()),
        Err(_) => {
            fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                DisconnectReasonCode::ProtocolError,
            )?;
            Err(Error::ProtocolError)
        }
    }
}

pub(crate) fn enqueue_pubcomp_or_fail_protocol<Time: 'static>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet_id: NonZero<u16>,
    reason_code: PubCompReasonCode,
) -> Result<(), Error> {
    match enqueue_packet(
        scratchpad,
        &ControlPacket::PubComp(PubComp {
            packet_id,
            reason_code,
            properties: PubCompProperties::default(),
        }),
    ) {
        Ok(()) => Ok(()),
        Err(_) => {
            fail_protocol_and_disconnect(
                settings,
                session,
                scratchpad,
                DisconnectReasonCode::ProtocolError,
            )?;
            Err(Error::ProtocolError)
        }
    }
}
