use crate::client::ClientSettings;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use alloc::vec::Vec;
use bytes::Bytes;
use core::num::NonZero;
use encode::Encodable;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::ReasonCode;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::Disconnect;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::EncodeError;
use sansio_mqtt_v5_types::PubAck;
use sansio_mqtt_v5_types::PubAckReasonCode;
use sansio_mqtt_v5_types::PubComp;
use sansio_mqtt_v5_types::PubCompReasonCode;
use sansio_mqtt_v5_types::PubRec;
use sansio_mqtt_v5_types::PubRecReasonCode;
use sansio_mqtt_v5_types::PubRel;
use sansio_mqtt_v5_types::PubRelReasonCode;

pub(crate) fn encode_control_packet(packet: &ControlPacket) -> Result<Bytes, Error> {
    let mut encoded = Vec::new();
    packet.encode(&mut encoded).map_err(|err| match err {
        EncodeError::PacketTooLarge(_) => Error::PacketTooLarge,
        _ => Error::EncodeFailure,
    })?;
    Ok(Bytes::from(encoded))
}

pub(crate) fn enqueue_packet<T>(
    scratchpad: &mut ClientScratchpad<T>,
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
pub(crate) fn reset_connection_state<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
) {
    scratchpad.read_buffer.clear();
    crate::session_ops::reset_keepalive(scratchpad);
    crate::limits::reset_negotiated_limits(settings, session, scratchpad);
    crate::session_ops::maybe_reset_session_state(session, scratchpad);
}

/// Marker returned by helpers that have already torn the connection down.
///
/// It carries no [`Error`]: a connection failure the protocol handles itself is
/// reported to the application as [`Event::Disconnected`], not as an `Err` to
/// the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConnectionFailed;

/// Enqueues a DISCONNECT carrying `reason` best-effort, asks the driver to
/// close the socket, and resets all connection state.
///
/// The DISCONNECT bytes are queued before `CloseSocket`, so a driver that
/// drains `poll_write` before performing an action sends them on the socket
/// being closed. Reports nothing to the application; see
/// [`graceful_disconnect`] and [`fail_connection`].
pub(crate) fn disconnect_and_reset<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    reason: DisconnectReasonCode,
) {
    let _ = enqueue_packet(
        scratchpad,
        &ControlPacket::Disconnect(Disconnect::builder().reason_code(reason).build()),
    );
    scratchpad.action_queue.push_back(DriverAction::CloseSocket);
    reset_connection_state(settings, session, scratchpad);
}

/// Performs a client-initiated normal disconnect and reports it to the
/// application.
///
/// [MQTT-3.14.4-1] After sending DISCONNECT the client MUST close the Network
/// Connection and MUST NOT send any more packets on it.
pub(crate) fn graceful_disconnect<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
) {
    disconnect_and_reset(
        settings,
        session,
        scratchpad,
        DisconnectReasonCode::NormalDisconnection,
    );
    scratchpad.read_queue.push_back(Event::Disconnected(None));
}

/// Tears the connection down because of a failure the protocol detected, and
/// reports `reason` to the application.
///
/// [MQTT-4.13.1-1] On a Malformed Packet or Protocol Error the client sends
/// DISCONNECT with the matching Reason Code and closes the Network Connection.
pub(crate) fn fail_connection<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    reason: DisconnectReasonCode,
) -> ConnectionFailed {
    disconnect_and_reset(settings, session, scratchpad, reason);
    scratchpad.read_queue.push_back(Event::Disconnected(Some(
        crate::convert::disconnect_reason_to_protocol(reason),
    )));
    ConnectionFailed
}

/// Closes a socket on which no MQTT session was ever established, reporting
/// `reason` to the application.
///
/// No DISCONNECT is sent: before CONNACK there is nothing to disconnect from
/// ([MQTT-3.2.2-7] after a failed CONNACK the server has already closed the
/// Network Connection).
pub(crate) fn close_without_disconnect<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    reason: Option<ReasonCode>,
) {
    scratchpad.action_queue.push_back(DriverAction::CloseSocket);
    reset_connection_state(settings, session, scratchpad);
    scratchpad.read_queue.push_back(Event::Disconnected(reason));
}

/// Abandons a connection attempt whose socket has not been reported connected
/// yet.
///
/// A still-queued `OpenSocket` is withdrawn so the driver never opens the
/// socket; otherwise the driver is already connecting and `CloseSocket` asks it
/// to abandon the attempt.
pub(crate) fn abandon_socket_request<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
) {
    let queued = scratchpad.action_queue.len();
    scratchpad
        .action_queue
        .retain(|action| !matches!(action, DriverAction::OpenSocket));
    if scratchpad.action_queue.len() == queued {
        scratchpad.action_queue.push_back(DriverAction::CloseSocket);
    }
    reset_connection_state(settings, session, scratchpad);
    scratchpad.read_queue.push_back(Event::Disconnected(None));
}

/// Handles the driver reporting that it lost the socket (`SocketClosed` or
/// `SocketError`).
///
/// The driver has already dropped the socket, so no `CloseSocket` is queued.
/// Bytes not yet written are discarded: they belonged to the lost connection
/// and must never reach the next one.
pub(crate) fn on_socket_lost<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
) {
    scratchpad.write_queue.clear();
    reset_connection_state(settings, session, scratchpad);
    scratchpad.read_queue.push_back(Event::Disconnected(None));
}

/// Enqueues an acknowledgement packet, tearing the connection down if it cannot
/// be sent.
///
/// An acknowledgement that cannot be encoded or exceeds the broker's maximum
/// packet size leaves the QoS exchange unresolvable, so the only correct
/// response is to fail the connection.
pub(crate) fn enqueue_ack_or_fail_protocol<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    packet: &ControlPacket,
) -> Result<(), ConnectionFailed> {
    if enqueue_packet(scratchpad, packet).is_err() {
        return Err(fail_connection(
            settings,
            session,
            scratchpad,
            DisconnectReasonCode::ProtocolError,
        ));
    }

    Ok(())
}

pub(crate) fn pubrel(packet_id: NonZero<u16>) -> ControlPacket {
    ControlPacket::PubRel(
        PubRel::builder()
            .packet_id(packet_id)
            .reason_code(PubRelReasonCode::Success)
            .build(),
    )
}

pub(crate) fn puback(packet_id: NonZero<u16>, reason_code: PubAckReasonCode) -> ControlPacket {
    ControlPacket::PubAck(
        PubAck::builder()
            .packet_id(packet_id)
            .reason_code(reason_code)
            .build(),
    )
}

pub(crate) fn pubrec(packet_id: NonZero<u16>, reason_code: PubRecReasonCode) -> ControlPacket {
    ControlPacket::PubRec(
        PubRec::builder()
            .packet_id(packet_id)
            .reason_code(reason_code)
            .build(),
    )
}

pub(crate) fn pubcomp(packet_id: NonZero<u16>, reason_code: PubCompReasonCode) -> ControlPacket {
    ControlPacket::PubComp(
        PubComp::builder()
            .packet_id(packet_id)
            .reason_code(reason_code)
            .build(),
    )
}
