use crate::client::ClientSettings;
use crate::convert;
use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::session_ops;
use crate::state::ClientState;
use crate::state::StateHandler;
use crate::state::connected::Connected;
use crate::state::disconnected::Disconnected;
use crate::state::fail_with_protocol_error;
use crate::state::on_socket_lost;
use bytes::Bytes;
use core::num::NonZero;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_v5_types::AuthReasonCode;
use sansio_mqtt_v5_types::ConnAck;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::ControlPacket;

/// A connection attempt is in progress: `OpenSocket` has been requested and
/// CONNACK has not arrived yet.
///
/// The connect options live in `scratchpad.pending_connect_options` for the
/// whole client lifetime, so this state carries none of its own.
#[derive(Debug)]
pub(crate) struct Connecting {
    phase: ConnectingPhase,
}

/// How far the connection attempt has progressed.
#[derive(Debug)]
enum ConnectingPhase {
    /// `OpenSocket` was queued; the driver has not reported the outcome yet.
    AwaitingSocket {
        /// The CONNECT packet, encoded when `Command::Connect` was validated
        /// and sent as-is once the socket is connected: settings are immutable
        /// and the options cannot change before then.
        connect: Bytes,
    },
    /// The socket is connected and CONNECT has been queued.
    AwaitingConnAck,
}

impl Connecting {
    /// A connection attempt that sends the already encoded `connect` once
    /// the requested socket is connected.
    pub(crate) fn awaiting_socket(connect: Bytes) -> Self {
        Self {
            phase: ConnectingPhase::AwaitingSocket { connect },
        }
    }

    pub(crate) fn has_open_socket(&self) -> bool {
        matches!(self.phase, ConnectingPhase::AwaitingConnAck)
    }
}

/// Handles the `SocketConnected` that answers the `OpenSocket` queued by
/// `Command::Connect`.
///
/// Resets negotiated limits, enqueues the CONNECT packet encoded when
/// `Command::Connect` was accepted, and resets keepalive tracking.
///
/// No Maximum Packet Size has been negotiated on the new socket yet, so the
/// encoded CONNECT needs no further validation.
fn on_socket_connected<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    connect: Bytes,
) -> (ClientState, Result<(), Error>)
where
    T: Time,
{
    limits::reset_negotiated_limits(settings, session, scratchpad);
    scratchpad.write_queue.push_back(connect);
    scratchpad.keep_alive_saw_network_activity = false;
    scratchpad.keep_alive_ping_outstanding = false;
    (
        ClientState::Connecting(Connecting {
            phase: ConnectingPhase::AwaitingConnAck,
        }),
        Ok(()),
    )
}

/// Handles a CONNACK.
///
/// On a successful reason code, populates the negotiated limits, recomputes the
/// effective ones, arms keep-alive and transitions to Connected. Any other
/// reason code closes the connection.
fn on_connack<T>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<T>,
    connack: ConnAck,
    received_at: T,
) -> (ClientState, Result<(), Error>)
where
    T: Time,
{
    // [MQTT-3.2.2-2] Session Present reports whether the server resumed an
    // existing Session.
    let session_present = match connack.kind {
        ConnAckKind::ResumePreviousSession => true,
        ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        } => false,
        // [MQTT-3.2.2-7] A CONNACK with a non-Success Reason Code means the
        // server has closed the Network Connection.
        ConnAckKind::Other { reason_code } => {
            queues::close_without_disconnect(
                settings,
                session,
                scratchpad,
                Some(convert::connack_reason_to_protocol(reason_code)),
            );
            return (ClientState::Disconnected(Disconnected), Ok(()));
        }
    };

    scratchpad.negotiated_receive_maximum = connack
        .properties
        .receive_maximum
        .unwrap_or(NonZero::<u16>::MAX);
    scratchpad.negotiated_maximum_packet_size = connack.properties.maximum_packet_size;
    scratchpad.negotiated_topic_alias_maximum = connack.properties.topic_alias_maximum.unwrap_or(0);
    scratchpad.negotiated_server_keep_alive = connack.properties.server_keep_alive;
    scratchpad.negotiated_maximum_qos = connack.properties.maximum_qos;
    scratchpad.negotiated_retain_available = connack.properties.retain_available.unwrap_or(true);
    scratchpad.negotiated_wildcard_subscription_available = connack
        .properties
        .wildcard_subscription_available
        .unwrap_or(true);
    scratchpad.negotiated_subscription_identifiers_available = connack
        .properties
        .subscription_identifiers_available
        .unwrap_or(true);
    scratchpad.negotiated_shared_subscription_available = connack
        .properties
        .shared_subscription_available
        .unwrap_or(true);
    limits::recompute_effective_limits(settings, scratchpad);

    // [MQTT-3.1.2-4] The server may override the session expiry interval in
    // CONNACK. Update session_should_persist based on the server's negotiated
    // value: Some(0) or None → do not persist; Some(n > 0) → persist.
    scratchpad.session_should_persist = match connack.properties.session_expiry_interval {
        Some(0) => false,
        Some(_) => true,
        None => scratchpad
            .pending_connect_options
            .as_ref()
            .and_then(|options| options.session_expiry)
            .is_some_and(|interval| !interval.is_zero()),
    };

    // [MQTT-3.1.2-22] The client enforces the keep-alive it sent in CONNECT,
    // unless the server specifies its own in CONNACK, which the client MUST
    // use (0 disables keep-alive for this connection).
    scratchpad.keep_alive_interval_secs = match scratchpad.negotiated_server_keep_alive {
        Some(server_keep_alive) => NonZero::new(server_keep_alive),
        None => settings.keep_alive,
    };
    scratchpad.keep_alive_saw_network_activity = false;
    scratchpad.keep_alive_ping_outstanding = false;

    if session_present {
        // [MQTT-3.2.2-2] Session Present=1 is only valid when CONNECT had Clean
        // Start=0.
        let clean_start = scratchpad
            .pending_connect_options
            .as_ref()
            .is_some_and(|options| options.clean_start);
        if clean_start {
            return fail_with_protocol_error(settings, session, scratchpad);
        }
        // [MQTT-4.4.0-1] [MQTT-4.4.0-2] Session Present=1 resumes in-flight QoS
        // transactions and replay path.
        if session_ops::replay_outbound_inflight_with_dup(session, scratchpad).is_err() {
            return fail_with_protocol_error(settings, session, scratchpad);
        }
    }

    scratchpad.read_queue.push_back(Event::Connected);

    if !session_present {
        // [MQTT-3.2.2-2] Session Present=0 means the server discarded any prior
        // session, so nothing in flight can still be delivered.
        session_ops::emit_publish_dropped_for_all_inflight(session, scratchpad);
        session_ops::reset_session_state(session);
    }

    // CONNACK ends the connection attempt, and with it the connect deadline
    // (MQTT v5.0 §3.2). [MQTT-3.1.2-22] Arm the keep-alive timer from the
    // CONNACK arrival instant so the first deadline fires one interval after
    // the session was established.
    scratchpad.next_timeout = None;
    if let Some(interval_secs) = scratchpad.keep_alive_interval_secs {
        scratchpad.arm_keep_alive_deadline(received_at, u64::from(interval_secs.get()));
    }

    (ClientState::Connected(Connected), Ok(()))
}

impl<T> StateHandler<T> for Connecting
where
    T: Time,
{
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        packet: ControlPacket,
        received_at: T,
    ) -> (ClientState, Result<(), Error>) {
        if !self.has_open_socket() {
            // No socket yet, so no packet can legitimately have arrived.
            return (
                ClientState::Connecting(self),
                Err(Error::InvalidStateTransition),
            );
        }
        match packet {
            ControlPacket::ConnAck(connack) => {
                on_connack(settings, session, scratchpad, connack, received_at)
            }
            // [MQTT-3.15.4-1] AUTH is only valid mid-handshake when the CONNECT
            // requested enhanced authentication and asks to continue it.
            ControlPacket::Auth(auth)
                if scratchpad
                    .pending_connect_options
                    .as_ref()
                    .is_some_and(|options| options.authentication.is_some())
                    && matches!(auth.reason_code, AuthReasonCode::ContinueAuthentication) =>
            {
                (ClientState::Connecting(self), Ok(()))
            }
            _ => fail_with_protocol_error(settings, session, scratchpad),
        }
    }

    fn handle_write(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        msg: Command,
        _at: T,
    ) -> (ClientState, Result<(), Error>) {
        match msg {
            // A user-requested disconnect is the same teardown as `close`.
            Command::Disconnect => self.close(settings, session, scratchpad),
            _ => (
                ClientState::Connecting(self),
                Err(Error::InvalidStateTransition),
            ),
        }
    }

    fn handle_event(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        evt: DriverEvent,
        _at: T,
    ) -> (ClientState, Result<(), Error>) {
        match (self.phase, evt) {
            (ConnectingPhase::AwaitingSocket { connect }, DriverEvent::SocketConnected) => {
                on_socket_connected(settings, session, scratchpad, connect)
            }
            // The socket is already open: the protocol did not ask for
            // another one.
            (phase @ ConnectingPhase::AwaitingConnAck, DriverEvent::SocketConnected) => (
                ClientState::Connecting(Connecting { phase }),
                Err(Error::InvalidStateTransition),
            ),
            // `OpenSocket` failed, or the open socket was lost before CONNACK.
            (_, DriverEvent::SocketClosed | DriverEvent::SocketError) => {
                on_socket_lost(settings, session, scratchpad)
            }
            (phase, _) => (
                ClientState::Connecting(Connecting { phase }),
                Err(Error::InvalidStateTransition),
            ),
        }
    }

    fn handle_timeout(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
        _now: T,
    ) -> (ClientState, Result<(), Error>) {
        // The connect deadline armed by `Command::Connect` elapsed before
        // CONNACK. "If the Client does not receive a CONNACK packet from the
        // Server within a reasonable amount of time, the Client SHOULD close
        // the Network Connection" (MQTT v5.0 §3.2).
        match self.phase {
            ConnectingPhase::AwaitingSocket { .. } => {
                queues::abandon_socket_request(settings, session, scratchpad);
            }
            ConnectingPhase::AwaitingConnAck => {
                queues::close_without_disconnect(settings, session, scratchpad, None);
            }
        }
        (ClientState::Disconnected(Disconnected), Ok(()))
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<T>,
    ) -> (ClientState, Result<(), Error>) {
        match self.phase {
            // Nothing has been sent, so there is nothing to DISCONNECT from.
            ConnectingPhase::AwaitingSocket { .. } => {
                queues::abandon_socket_request(settings, session, scratchpad);
            }
            ConnectingPhase::AwaitingConnAck => {
                queues::graceful_disconnect(settings, session, scratchpad);
            }
        }
        (ClientState::Disconnected(Disconnected), Ok(()))
    }
}
