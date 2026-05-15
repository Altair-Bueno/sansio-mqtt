use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::session_ops;
use crate::state::connected::Connected;
use crate::state::disconnected::Disconnected;
use crate::state::ClientState;
use crate::state::StateHandler;
use crate::types::ClientSettings;
use crate::types::ConnectionOptions;
use crate::types::DriverEventIn;
use crate::types::DriverEventOut;
use crate::types::Error;
use crate::types::UserWriteIn;
use crate::types::UserWriteOut;
use core::num::NonZero;
use core::ops::Add;
use core::time::Duration;
use sansio_mqtt_v5_types::BinaryData;
use sansio_mqtt_v5_types::ConnAckKind;
use sansio_mqtt_v5_types::ConnackReasonCode;
use sansio_mqtt_v5_types::Connect;
use sansio_mqtt_v5_types::ConnectProperties;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::Disconnect;
use sansio_mqtt_v5_types::DisconnectProperties;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::Utf8String;
use sansio_mqtt_v5_types::Will as ConnectWill;
use sansio_mqtt_v5_types::WillProperties;

#[derive(Debug)]
pub(crate) struct Connecting {
    pub(crate) pending_connect_options: ConnectionOptions,
    /// Set to `true` after the CONNECT packet has been sent (i.e., after
    /// SocketConnected fires and the CONNECT is enqueued). Used to reject a
    /// second SocketConnected in Connecting state.
    pub(crate) connect_sent: bool,
}

/// Builds a CONNECT packet from [`ClientSettings`] and [`ConnectionOptions`].
///
/// Constructs the MQTT CONNECT packet, mapping will properties, enforcing
/// limits from both user-supplied options and client settings.
fn build_connect(settings: &ClientSettings, options: &ConnectionOptions) -> Result<Connect, Error> {
    let will = options
        .will
        .as_ref()
        .map(|will| {
            let payload =
                BinaryData::try_new(will.payload.clone()).map_err(|_| Error::ProtocolError)?;
            let message_expiry_interval = will
                .message_expiry_interval
                .map(|interval| u32::try_from(interval.as_secs()).map_err(|_| Error::ProtocolError))
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
        keep_alive: options.keep_alive.or(settings.default_keep_alive),
        properties: ConnectProperties {
            session_expiry_interval: options.session_expiry_interval,
            receive_maximum: [
                options.receive_maximum,
                settings.max_incoming_receive_maximum,
            ]
            .into_iter()
            .flatten()
            .min(),
            maximum_packet_size: [
                options.maximum_packet_size,
                settings.max_incoming_packet_size,
            ]
            .into_iter()
            .flatten()
            .min(),
            topic_alias_maximum: options
                .topic_alias_maximum
                .or(settings.max_incoming_topic_alias_maximum)
                .map(|topic_alias_maximum| {
                    topic_alias_maximum.min(
                        settings
                            .max_incoming_topic_alias_maximum
                            .unwrap_or(u16::MAX),
                    )
                }),
            request_response_information: options
                .request_response_information
                .or(settings.default_request_response_information),
            request_problem_information: options
                .request_problem_information
                .or(settings.default_request_problem_information),
            authentication: options.authentication.clone(),
            user_properties: options.user_properties.clone(),
        },
    })
}

/// Handles a `SocketConnected` event while in the Connecting state.
///
/// Resets negotiated limits, builds and enqueues the CONNECT packet, and
/// resets keepalive tracking flags. On error, stays in Connecting.
pub(crate) fn on_socket_connected<Time>(
    connecting: Connecting,
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
) -> (ClientState, Result<(), Error>)
where
    Time: Ord + Add<Duration, Output = Time> + Copy,
{
    limits::reset_negotiated_limits(settings, session, scratchpad);
    let connect = match build_connect(settings, &connecting.pending_connect_options) {
        Ok(packet) => packet,
        Err(e) => return (ClientState::Connecting(connecting), Err(e)),
    };
    match queues::enqueue_packet(scratchpad, &ControlPacket::Connect(connect)) {
        Ok(()) => {
            scratchpad.keep_alive_saw_network_activity = false;
            scratchpad.keep_alive_ping_outstanding = false;
            (
                ClientState::Connecting(Connecting {
                    connect_sent: true,
                    ..connecting
                }),
                Ok(()),
            )
        }
        Err(e) => (ClientState::Connecting(connecting), Err(e)),
    }
}

/// Handles `SocketClosed` or `SocketError` events while in the Connecting
/// state.
///
/// Clears the read buffer, resets keepalive, negotiated limits, and session
/// state, then emits `Disconnected`. On error, also enqueues `CloseSocket` and
/// returns `ProtocolError`.
fn on_socket_closed_or_error<Time>(
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    is_error: bool,
) -> (ClientState, Result<(), Error>)
where
    Time: Ord + Add<Duration, Output = Time> + Copy,
{
    scratchpad.read_buffer.clear();
    session_ops::reset_keepalive(scratchpad);
    limits::reset_negotiated_limits(settings, session, scratchpad);
    session_ops::maybe_reset_session_state(session, scratchpad);
    if is_error {
        // Socket error does not emit Disconnected; only enqueues CloseSocket.
        scratchpad
            .action_queue
            .push_back(DriverEventOut::CloseSocket);
        (
            ClientState::Disconnected(Disconnected),
            Err(Error::ProtocolError),
        )
    } else {
        scratchpad
            .read_queue
            .push_back(UserWriteOut::Disconnected(None));
        (ClientState::Disconnected(Disconnected), Ok(()))
    }
}

/// Handles a successful CONNACK (reason code Success or ResumePreviousSession).
///
/// Populates negotiated scratchpad fields from CONNACK properties, recomputes
/// effective limits, sets keep-alive from server or options, resets keepalive
/// tracking, then transitions to Connected and emits `UserWriteOut::Connected`.
fn on_connack_success<Time>(
    connecting: Connecting,
    settings: &ClientSettings,
    session: &mut ClientSession,
    scratchpad: &mut ClientScratchpad<Time>,
    connack: sansio_mqtt_v5_types::ConnAck,
) -> (ClientState, Result<(), Error>)
where
    Time: Ord + Add<Duration, Output = Time> + Copy,
{
    scratchpad.negotiated_receive_maximum = connack
        .properties
        .receive_maximum
        .unwrap_or(NonZero::new(u16::MAX).expect("u16::MAX is always non-zero"));
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
    scratchpad.pending_connect_options = connecting.pending_connect_options;
    limits::recompute_effective_limits(settings, scratchpad);

    // [MQTT-3.1.2-4] The server may override the session expiry interval in
    // CONNACK. Update session_should_persist based on the server's negotiated
    // value: Some(0) or None → do not persist; Some(n > 0) → persist.
    scratchpad.session_should_persist = match connack.properties.session_expiry_interval {
        Some(0) => false,
        Some(_) => true,
        None => {
            scratchpad
                .pending_connect_options
                .session_expiry_interval
                .unwrap_or(0)
                > 0
        }
    };

    // [MQTT-3.1.2-22] If the server specifies a keep-alive of 0 in CONNACK, it
    // disables keep-alive for this connection. The client MUST use the server's
    // value when present.
    scratchpad.keep_alive_interval_secs = match scratchpad.negotiated_server_keep_alive {
        Some(server_keep_alive) => NonZero::new(server_keep_alive),
        None => scratchpad.pending_connect_options.keep_alive,
    };
    scratchpad.keep_alive_saw_network_activity = false;
    scratchpad.keep_alive_ping_outstanding = false;

    let mut connected_emitted = false;

    match connack.kind {
        ConnAckKind::ResumePreviousSession => {
            // [MQTT-3.2.2-2] Session Present=1 is only valid when CONNECT had Clean
            // Start=0.
            if scratchpad.pending_connect_options.clean_start {
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
            // [MQTT-4.4.0-1] [MQTT-4.4.0-2] Session Present=1 resumes in-flight QoS
            // transactions and replay path.
            if session_ops::replay_outbound_inflight_with_dup(session, scratchpad).is_err() {
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
        }
        ConnAckKind::Other {
            reason_code: ConnackReasonCode::Success,
        } => {
            scratchpad.read_queue.push_back(UserWriteOut::Connected);
            connected_emitted = true;
            session_ops::emit_publish_dropped_for_all_inflight(session, scratchpad);
            session_ops::reset_session_state(session);
        }
        _ => unreachable!("successful CONNACK kind already matched"),
    }

    if !connected_emitted {
        scratchpad.read_queue.push_back(UserWriteOut::Connected);
    }

    (ClientState::Connected(Connected), Ok(()))
}

impl<Time> StateHandler<Time> for Connecting
where
    Time: Ord + Add<Duration, Output = Time> + Copy,
{
    fn handle_control_packet(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet: ControlPacket,
    ) -> (ClientState, Result<(), Error>) {
        match packet {
            ControlPacket::ConnAck(connack) => {
                if matches!(
                    connack.kind,
                    ConnAckKind::ResumePreviousSession
                        | ConnAckKind::Other {
                            reason_code: ConnackReasonCode::Success
                        }
                ) {
                    on_connack_success(self, settings, session, scratchpad, connack)
                } else {
                    limits::reset_negotiated_limits(settings, session, scratchpad);
                    scratchpad
                        .action_queue
                        .push_back(DriverEventOut::CloseSocket);
                    (
                        ClientState::Disconnected(Disconnected),
                        Err(Error::ProtocolError),
                    )
                }
            }
            ControlPacket::Auth(auth) => {
                if self.pending_connect_options.authentication.is_none() {
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

                if !matches!(
                    auth.reason_code,
                    sansio_mqtt_v5_types::AuthReasonCode::ContinueAuthentication
                ) {
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

                (ClientState::Connecting(self), Ok(()))
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
            UserWriteIn::Disconnect => {
                scratchpad.pending_connect_options = self.pending_connect_options;
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
        scratchpad: &mut ClientScratchpad<Time>,
        evt: DriverEventIn,
    ) -> (ClientState, Result<(), Error>) {
        match evt {
            DriverEventIn::SocketConnected => {
                if self.connect_sent {
                    // CONNECT was already sent; a second SocketConnected is invalid.
                    (
                        ClientState::Connecting(self),
                        Err(Error::InvalidStateTransition),
                    )
                } else {
                    // CONNECT not yet sent (transition came from handle_write(Connect));
                    // send CONNECT now.
                    on_socket_connected(self, settings, session, scratchpad)
                }
            }
            DriverEventIn::SocketClosed => {
                scratchpad.pending_connect_options = self.pending_connect_options;
                on_socket_closed_or_error(settings, session, scratchpad, false)
            }
            DriverEventIn::SocketError => {
                scratchpad.pending_connect_options = self.pending_connect_options;
                on_socket_closed_or_error(settings, session, scratchpad, true)
            }
        }
    }

    fn handle_timeout(
        self,
        _settings: &ClientSettings,
        _session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        _now: Time,
    ) -> (ClientState, Result<(), Error>) {
        // [MQTT-3.1.4-5] A timeout in the Connecting state means the server did not
        // respond with CONNACK within the caller-imposed deadline. Close the
        // socket and signal the error.
        scratchpad.pending_connect_options = self.pending_connect_options;
        scratchpad
            .action_queue
            .push_back(DriverEventOut::CloseSocket);
        (
            ClientState::Disconnected(Disconnected),
            Err(Error::ConnectTimeout),
        )
    }

    fn close(
        self,
        settings: &ClientSettings,
        session: &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>) {
        scratchpad.pending_connect_options = self.pending_connect_options;
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
