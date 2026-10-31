use crate::limits;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::ClientState;
use crate::state::StateHandler;
use bytes::Buf;
use core::num::NonZero;
use sansio::Protocol;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::MqttProtocol;
use sansio_mqtt_protocol::Qos;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::ParserSettings;
use sansio_mqtt_v5_types::variable_byte_integer;
use winnow::Parser;
use winnow::error::ErrMode;
use winnow::stream::Partial;

/// Configuration for a [`Client`].
///
/// Every limit here is the single source of truth: the values that end up in
/// the CONNECT packet, the caps enforced on outbound traffic, and the caps fed
/// to the inbound parser are all derived from this struct alone (there is no
/// longer a separate per-connect override).
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct ClientSettings {
    /// Receive Maximum advertised in CONNECT
    /// ([MQTT-3.1.2-23]).
    pub receive_maximum: Option<NonZero<u16>>,
    /// Maximum Packet Size, counting the whole packet, that the client
    /// accepts from the server.
    ///
    /// Always advertised in CONNECT and enforced on every inbound packet, so
    /// a compliant server never sends a packet the client cannot process
    /// [MQTT-3.1.2-24]. A larger packet is answered with DISCONNECT 0x95
    /// (Packet too large). Defaults to
    /// [`DEFAULT_MAXIMUM_PACKET_SIZE`](Self::DEFAULT_MAXIMUM_PACKET_SIZE).
    #[builder(default = ClientSettings::DEFAULT_MAXIMUM_PACKET_SIZE)]
    pub maximum_packet_size: NonZero<u32>,
    /// Topic Alias Maximum advertised in CONNECT, bounding the Topic Aliases
    /// the client will accept from the server.
    pub topic_alias_maximum: Option<u16>,
    /// Request Response Information flag sent in CONNECT.
    pub request_response_information: Option<bool>,
    /// Request Problem Information flag sent in CONNECT.
    pub request_problem_information: Option<bool>,
    /// Keep Alive sent in CONNECT when
    /// [`sansio_mqtt_protocol::ConnectOptions::keep_alive`] is `None`.
    pub keep_alive: Option<NonZero<u16>>,
    /// Local cap on the QoS of outbound PUBLISH packets.
    ///
    /// `Some(Qos::ExactlyOnce)` and `None` both mean "no local cap"; the
    /// server-advertised Maximum QoS is still enforced regardless.
    pub max_outgoing_qos: Option<Qos>,
    /// Whether outbound PUBLISH may set the RETAIN flag.
    #[builder(default = true)]
    pub allow_retain: bool,
    /// Whether outbound SUBSCRIBE may use wildcard Topic Filters.
    #[builder(default = true)]
    pub allow_wildcard_subscriptions: bool,
    /// Whether outbound SUBSCRIBE may use `$share/` Topic Filters.
    #[builder(default = true)]
    pub allow_shared_subscriptions: bool,
    /// Whether outbound SUBSCRIBE may carry a Subscription Identifier.
    #[builder(default = true)]
    pub allow_subscription_identifiers: bool,
    /// Maximum number of User Property entries accepted in any inbound
    /// property section.
    #[builder(default = 32)]
    pub max_user_properties: usize,
    /// Maximum number of Subscription Identifiers accepted in a single
    /// inbound PUBLISH.
    #[builder(default = 32)]
    pub max_subscription_identifiers: usize,
    /// Maximum number of Topic Filters accepted in a single inbound
    /// SUBSCRIBE.
    #[builder(default = 32)]
    pub max_subscriptions: u32,
}

impl ClientSettings {
    /// Default [`maximum_packet_size`](Self::maximum_packet_size): 1 MiB.
    pub const DEFAULT_MAXIMUM_PACKET_SIZE: NonZero<u32> = NonZero::new(1024 * 1024).unwrap();
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// MQTT v5.0 client state machine.
///
/// Implements [`sansio_mqtt_protocol::MqttProtocol`] (checked at compile time
/// by a private assertion below): feed it [`IncomingData`] read
/// from the network, [`Command`]s from the application, and [`DriverEvent`]s
/// from the driver, then drain [`Event`]s, encoded bytes, and
/// [`DriverAction`]s.
#[derive(Debug)]
pub struct Client<T> {
    settings: ClientSettings,
    session: ClientSession,
    scratchpad: ClientScratchpad<T>,
    state: ClientState,
}

impl<T> Default for Client<T> {
    fn default() -> Self {
        Self::new(ClientSettings::default())
    }
}

impl<T> Client<T> {
    /// Creates a new client in the initial (disconnected) state.
    pub fn new(settings: ClientSettings) -> Self {
        let mut client = Self {
            settings,
            session: ClientSession::default(),
            scratchpad: ClientScratchpad::default(),
            state: ClientState::Start(crate::state::Start),
        };
        limits::recompute_effective_limits(&client.settings, &mut client.scratchpad);
        client
    }

    /// The limits the inbound parser is held to.
    ///
    /// String and binary-data wire maxima are the MQTT wire format's own
    /// ceiling (`u16::MAX`); only the Remaining Length cap is derived from
    /// [`ClientSettings::maximum_packet_size`], and the counters come
    /// straight from [`ClientSettings`].
    fn parser_settings(&self) -> ParserSettings {
        ParserSettings::builder()
            .max_bytes_string(u16::MAX)
            .max_bytes_binary_data(u16::MAX)
            // Remaining Length never exceeds the whole packet size; the exact
            // check is `exceeds_maximum_packet_size`.
            .max_remaining_bytes(u64::from(self.settings.maximum_packet_size.get()))
            .max_subscriptions_len(self.settings.max_subscriptions)
            .max_user_properties_len(self.settings.max_user_properties)
            .max_subscription_identifiers_len(self.settings.max_subscription_identifiers)
            .build()
    }

    #[inline(always)]
    fn dispatch<F>(&mut self, f: F) -> Result<(), Error>
    where
        F: FnOnce(
            ClientState,
            &ClientSettings,
            &mut ClientSession,
            &mut ClientScratchpad<T>,
        ) -> (ClientState, Result<(), Error>),
    {
        let state = core::mem::take(&mut self.state);
        let (next, result) = f(
            state,
            &self.settings,
            &mut self.session,
            &mut self.scratchpad,
        );
        self.state = next;
        result
    }
}

impl<T> Client<T>
where
    T: Time,
{
    /// Parses and dispatches every whole control packet in `bytes`, returning
    /// how many bytes were consumed.
    ///
    /// A trailing partial packet is left unconsumed for the caller to retain.
    /// Parsing stops as soon as a packet tears the connection down: whatever
    /// follows belonged to the closed connection.
    fn consume_packets(&mut self, bytes: &[u8], received_at: T) -> Result<usize, Error> {
        let parser_settings = self.parser_settings();
        let mut slice: &[u8] = bytes;

        while !slice.is_empty() && self.state.has_open_socket() {
            match self.exceeds_maximum_packet_size(slice) {
                None => break,
                Some(true) => {
                    // MQTT v5.0 §3.1.2.11.4: a packet over the Client's
                    // Maximum Packet Size is a Protocol Error answered with
                    // DISCONNECT 0x95. Rejected from the fixed header alone,
                    // before the body is buffered.
                    self.fail_connection(DisconnectReasonCode::PacketTooLarge)?;
                    break;
                }
                Some(false) => {}
            }

            let mut input = Partial::new(slice);

            match ControlPacket::parser::<_, ErrMode<()>, ErrMode<()>>(&parser_settings)
                .parse_next(&mut input)
            {
                Ok(packet) => {
                    slice = input.into_inner();
                    self.dispatch(|s, set, ses, sp| {
                        s.handle_control_packet(set, ses, sp, packet, received_at)
                    })?;
                }
                Err(ErrMode::Incomplete(_)) => break,
                Err(ErrMode::Backtrack(_) | ErrMode::Cut(_)) => {
                    // [MQTT-4.13.1-1] Malformed Control Packet requires
                    // DISCONNECT with Reason Code 0x81 and closing the Network
                    // Connection.
                    self.fail_connection(DisconnectReasonCode::MalformedPacket)?;
                    break;
                }
            }
        }

        Ok(bytes.len() - slice.len())
    }

    /// Checks the fixed header at the start of `bytes` against the Maximum
    /// Packet Size, which counts the whole packet, fixed header included.
    ///
    /// Returns `None` while the fixed header is incomplete. A malformed
    /// Remaining Length is reported as `Some(false)` and left to the packet
    /// parser to reject.
    fn exceeds_maximum_packet_size(&self, bytes: &[u8]) -> Option<bool> {
        let mut input = Partial::new(bytes.get(1..)?);
        let before = input.len();
        let remaining_length = match variable_byte_integer::<_, ErrMode<()>>(&mut input) {
            Ok(remaining_length) => remaining_length,
            Err(ErrMode::Incomplete(_)) => return None,
            Err(ErrMode::Backtrack(()) | ErrMode::Cut(())) => return Some(false),
        };
        let fixed_header_len = 1 + (before - input.len()) as u64;

        Some(
            fixed_header_len + remaining_length
                > u64::from(self.settings.maximum_packet_size.get()),
        )
    }

    fn fail_connection(&mut self, reason: DisconnectReasonCode) -> Result<(), Error> {
        self.dispatch(|_s, set, ses, sp| crate::state::fail_with(set, ses, sp, reason))
    }
}

impl<'bytes, T> Protocol<IncomingData<'bytes, T>, Command, DriverEvent> for Client<T>
where
    T: Time,
{
    type Rout = Event;
    type Wout = bytes::Bytes;
    type Eout = DriverAction;
    type Error = Error;
    type Time = T;

    #[tracing::instrument(skip_all)]
    fn handle_read(&mut self, msg: IncomingData<'bytes, T>) -> Result<(), Self::Error> {
        // Bytes can only come from a socket the driver reported connected and
        // the protocol has not closed since.
        if !self.state.has_open_socket() {
            return Err(Error::InvalidStateTransition);
        }

        let received_at = msg.received_at;

        if self.scratchpad.read_buffer.is_empty() {
            // Nothing pending: parse straight out of the driver's buffer so the
            // common case of whole packets per read copies nothing.
            let consumed = self.consume_packets(msg.bytes, received_at)?;
            if self.state.has_open_socket() {
                self.scratchpad
                    .read_buffer
                    .extend_from_slice(&msg.bytes[consumed..]);
            }
        } else {
            self.scratchpad.read_buffer.extend_from_slice(msg.bytes);
            let mut buffer = core::mem::take(&mut self.scratchpad.read_buffer);
            let consumed = self.consume_packets(&buffer, received_at)?;
            if self.state.has_open_socket() {
                // Drop the consumed prefix by moving the start pointer; the
                // leading capacity is reclaimed by the next
                // `extend_from_slice`.
                buffer.advance(consumed);
                self.scratchpad.read_buffer = buffer;
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn handle_write(&mut self, msg: Command) -> Result<(), Self::Error> {
        // Keep-alive activity is tracked in `queues::enqueue_packet`, at the
        // one point where a packet actually reaches the write queue.
        self.dispatch(|s, set, ses, sp| s.handle_write(set, ses, sp, msg))
    }

    #[tracing::instrument(skip_all)]
    fn handle_event(&mut self, evt: DriverEvent) -> Result<(), Self::Error> {
        self.dispatch(|s, set, ses, sp| s.handle_event(set, ses, sp, evt))
    }

    #[tracing::instrument(skip_all)]
    fn handle_timeout(&mut self, now: Self::Time) -> Result<(), Self::Error> {
        self.dispatch(|s, set, ses, sp| s.handle_timeout(set, ses, sp, now))
    }

    #[tracing::instrument(skip_all)]
    fn close(&mut self) -> Result<(), Self::Error> {
        self.dispatch(|s, set, ses, sp| s.close(set, ses, sp))
    }

    fn poll_read(&mut self) -> Option<Self::Rout> {
        self.scratchpad.read_queue.pop_front()
    }

    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.scratchpad.write_queue.pop_front()
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.scratchpad.action_queue.pop_front()
    }

    fn poll_timeout(&mut self) -> Option<Self::Time> {
        self.scratchpad.next_timeout
    }
}

impl<'bytes, T> MqttProtocol<'bytes, T> for Client<T>
where
    T: Time,
{
    fn version(&self) -> semver::Version {
        use semver::*;

        Version {
            major: 5,
            minor: 0,
            patch: 0,
            pre: Prerelease::EMPTY,
            build: BuildMetadata::EMPTY,
        }
    }
}
