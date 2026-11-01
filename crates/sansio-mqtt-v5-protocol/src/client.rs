use crate::limits;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::settings::ClientSettings;
use crate::state::ClientState;
use crate::state::StateHandler as _;
use bytes::Buf as _;
use sansio::Protocol;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::MqttProtocol;
use sansio_mqtt_protocol::Time;
use sansio_mqtt_protocol::Timestamped;
use sansio_mqtt_v5_wire::ControlPacket;
use sansio_mqtt_v5_wire::DisconnectReasonCode;
use sansio_mqtt_v5_wire::ParserSettings;
use sansio_mqtt_v5_wire::variable_byte_integer;
use winnow::Parser as _;
use winnow::error::ErrMode;
use winnow::stream::Partial;

/// MQTT v5.0 client state machine.
///
/// Implements [`sansio_mqtt_protocol::MqttProtocol`] (checked at compile time
/// by a private assertion below): feed it [`IncomingData`] read
/// from the network, and [`Command`]s from the application and
/// [`DriverEvent`]s from the driver, each [`Timestamped`] with the instant it
/// happened, then drain [`Event`]s, encoded bytes, [`DriverAction`]s and the
/// next deadline.
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
    #[must_use]
    pub fn new(settings: ClientSettings) -> Self {
        let mut client = Self {
            settings,
            session: ClientSession::default(),
            scratchpad: ClientScratchpad::default(),
            state: ClientState::Disconnected(crate::state::Disconnected),
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
            .max_remaining_bytes(self.settings.maximum_packet_size.get())
            .max_subscriptions_len(self.settings.max_subscriptions)
            .max_user_properties_len(self.settings.max_user_properties)
            .max_subscription_identifiers_len(self.settings.max_subscription_identifiers)
            .build()
    }

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
                Err(ErrMode::Backtrack(()) | ErrMode::Cut(())) => {
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
    #[expect(
        clippy::expect_used,
        reason = "the Remaining Length spans at most four bytes [MQTT-1.5.5-1], which always fits in a `u64`"
    )]
    fn exceeds_maximum_packet_size(&self, bytes: &[u8]) -> Option<bool> {
        let mut input = Partial::new(bytes.get(1..)?);
        let before = input.len();
        let remaining_length = match variable_byte_integer::<_, ErrMode<()>>(&mut input) {
            Ok(remaining_length) => remaining_length,
            Err(ErrMode::Incomplete(_)) => return None,
            Err(ErrMode::Backtrack(()) | ErrMode::Cut(())) => return Some(false),
        };
        let fixed_header_len = 1 + u64::try_from(before - input.len())
            .expect("a Remaining Length is at most four bytes long [MQTT-1.5.5-1]");

        Some(
            fixed_header_len + u64::from(remaining_length)
                > u64::from(self.settings.maximum_packet_size.get()),
        )
    }

    fn fail_connection(&mut self, reason: DisconnectReasonCode) -> Result<(), Error> {
        self.dispatch(|_s, set, ses, sp| crate::state::fail_with(set, ses, sp, reason))
    }
}

impl<'bytes, T>
    Protocol<IncomingData<'bytes, T>, Timestamped<Command, T>, Timestamped<DriverEvent, T>>
    for Client<T>
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
    fn handle_write(&mut self, msg: Timestamped<Command, T>) -> Result<(), Self::Error> {
        // Keep-alive activity is tracked in `queues::enqueue_packet`, at the
        // one point where a packet actually reaches the write queue.
        let Timestamped { value, at } = msg;
        self.dispatch(|s, set, ses, sp| s.handle_write(set, ses, sp, value, at))
    }

    #[tracing::instrument(skip_all)]
    fn handle_event(&mut self, evt: Timestamped<DriverEvent, T>) -> Result<(), Self::Error> {
        let Timestamped { value, at } = evt;
        self.dispatch(|s, set, ses, sp| s.handle_event(set, ses, sp, value, at))
    }

    #[tracing::instrument(skip_all)]
    fn handle_timeout(&mut self, now: Self::Time) -> Result<(), Self::Error> {
        self.dispatch(|s, set, ses, sp| s.handle_timeout(set, ses, sp, now))
    }

    #[tracing::instrument(skip_all)]
    fn close(&mut self) -> Result<(), Self::Error> {
        self.dispatch(ClientState::close)
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

impl<T> MqttProtocol<'_, T> for Client<T>
where
    T: Time,
{
    fn version(&self) -> semver::Version {
        use semver::BuildMetadata;
        use semver::Prerelease;
        use semver::Version;

        Version {
            major: 5,
            minor: 0,
            patch: 0,
            pre: Prerelease::EMPTY,
            build: BuildMetadata::EMPTY,
        }
    }
}
