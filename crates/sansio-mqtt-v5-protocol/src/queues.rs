use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::types::ClientSettings;
use crate::types::DriverEventOut;
use crate::types::Error;
use crate::types::UserWriteOut;
use alloc::vec::Vec;
use bytes::Bytes;
use core::num::NonZero;
use encode::Encodable;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::Disconnect;
use sansio_mqtt_v5_types::DisconnectProperties;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::EncodeError;
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

pub(crate) fn encode_control_packet(packet: &ControlPacket) -> Result<Bytes, Error> {
    let mut encoded = Vec::new();
    packet.encode(&mut encoded).map_err(|err| match err {
        EncodeError::PacketTooLarge(_) => Error::PacketTooLarge,
        _ => Error::EncodeFailure,
    })?;
    Ok(Bytes::from(encoded))
}

pub(crate) fn enqueue_packet<Time>(
    scratchpad: &mut ClientScratchpad<Time>,
    packet: &ControlPacket,
) -> Result<(), Error> {
    let encoded = encode_control_packet(packet)?;
    crate::limits::validate_outbound_packet_size(scratchpad, encoded.len())?;
    scratchpad.write_queue.push_back(encoded);
    // [MQTT-3.1.2-22]: Any outbound control packet counts as keep-alive
    // activity, except PINGREQ itself (which is the keep-alive probe and must
    // not suppress its own sending).
    if !matches!(packet, ControlPacket::PingReq(_)) {
        scratchpad.keep_alive_saw_network_activity = true;
    }
    Ok(())
}

/// Clears the read buffer and resets keep-alive, negotiated limits, and (unless
/// the session must persist) session state.
///
/// Every connection teardown path funnels through here so the reset ordering
/// stays identical; `reset_negotiated_limits` also clears inbound topic aliases
/// per [MQTT-3.8.2-1].
pub(crate) fn reset_connection_state<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    scratchpad.read_buffer.clear();
    crate::session_ops::reset_keepalive(scratchpad);
    crate::limits::reset_negotiated_limits(settings, session, scratchpad);
    crate::session_ops::maybe_reset_session_state(session, scratchpad);
}

/// Enqueues a DISCONNECT with `reason` best-effort, asks the driver to close
/// the socket, and resets all connection state.
///
/// Does not report anything to the application: callers that tear down on the
/// user's behalf use [`graceful_disconnect`] instead.
pub(crate) fn fail_protocol_and_disconnect<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    reason: DisconnectReasonCode,
) {
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
    reset_connection_state(settings, session, scratchpad);
}

/// Performs a client-initiated normal disconnect and reports it to the
/// application.
///
/// [MQTT-3.14.4-1] After sending DISCONNECT the client MUST close the Network
/// Connection and MUST NOT send any more packets on it.
pub(crate) fn graceful_disconnect<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) {
    fail_protocol_and_disconnect(
        settings,
        session,
        scratchpad,
        DisconnectReasonCode::NormalDisconnection,
    );
    scratchpad
        .read_queue
        .push_back(UserWriteOut::Disconnected(None));
}

/// Enqueues an acknowledgement packet, tearing the connection down if it cannot
/// be sent.
///
/// An acknowledgement that cannot be encoded or exceeds the broker's maximum
/// packet size leaves the QoS exchange unresolvable, so the only correct
/// response is to fail the connection.
pub(crate) fn enqueue_ack_or_fail_protocol<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    packet: &ControlPacket,
) -> Result<(), Error> {
    if enqueue_packet(scratchpad, packet).is_err() {
        fail_protocol_and_disconnect(
            settings,
            session,
            scratchpad,
            DisconnectReasonCode::ProtocolError,
        );
        return Err(Error::ProtocolError);
    }

    Ok(())
}

pub(crate) fn pubrel(packet_id: NonZero<u16>) -> ControlPacket {
    ControlPacket::PubRel(PubRel {
        packet_id,
        reason_code: PubRelReasonCode::Success,
        properties: PubRelProperties::default(),
    })
}

pub(crate) fn puback(packet_id: NonZero<u16>, reason_code: PubAckReasonCode) -> ControlPacket {
    ControlPacket::PubAck(PubAck {
        packet_id,
        reason_code,
        properties: PubAckProperties::default(),
    })
}

pub(crate) fn pubrec(packet_id: NonZero<u16>, reason_code: PubRecReasonCode) -> ControlPacket {
    ControlPacket::PubRec(PubRec {
        packet_id,
        reason_code,
        properties: PubRecProperties::default(),
    })
}

pub(crate) fn pubcomp(packet_id: NonZero<u16>, reason_code: PubCompReasonCode) -> ControlPacket {
    ControlPacket::PubComp(PubComp {
        packet_id,
        reason_code,
        properties: PubCompProperties::default(),
    })
}
