use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::ClientState;
use crate::state::StateHandler;
use crate::types::ClientSettings;
use crate::types::DriverEventIn;
use crate::types::DriverEventOut;
use crate::types::Error;
use crate::types::IncomingData;
use crate::types::ProtocolTime;
use crate::types::UserWriteIn;
use crate::types::UserWriteOut;
use bytes::Buf;
use sansio::Protocol;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::ParserSettings;
use winnow::Parser;
use winnow::error::ErrMode;
use winnow::stream::Partial;

#[derive(Debug)]
pub struct Client<Time> {
    settings: ClientSettings,
    session: ClientSession,
    scratchpad: ClientScratchpad<Time>,
    state: ClientState,
}

impl<Time> Default for Client<Time> {
    fn default() -> Self {
        Self::with_settings(Default::default())
    }
}

impl<Time> Client<Time> {
    pub fn with_settings_and_session(settings: ClientSettings, session: ClientSession) -> Self {
        let mut client = Self {
            settings,
            session,
            scratchpad: ClientScratchpad::default(),
            state: ClientState::Start(crate::state::Start),
        };
        limits::recompute_effective_limits(&client.settings, &mut client.scratchpad);
        client
    }

    pub fn with_settings(settings: ClientSettings) -> Self {
        Self::with_settings_and_session(settings, Default::default())
    }

    /// Borrows the session state, for snapshotting a live connection.
    ///
    /// [`ClientSession`] is `Clone`, so an application that must survive a hard
    /// reboot can persist a clone of this periodically and restore it through
    /// [`Client::with_settings_and_session`].
    pub fn session(&self) -> &ClientSession {
        &self.session
    }

    /// Takes the session state out of the client, consuming it.
    ///
    /// [MQTT-4.1.0-1] Session State outlives the Network Connection when
    /// Session Expiry Interval is greater than zero; persisting the
    /// returned value and handing it to
    /// [`Client::with_settings_and_session`] resumes the session, replaying
    /// any unacknowledged QoS1/QoS2 PUBLISH with DUP=1.
    pub fn into_session(self) -> ClientSession {
        self.session
    }

    /// The limits the inbound parser is held to.
    ///
    /// Only `max_remaining_bytes` is negotiated (it is additionally clamped by
    /// the Maximum Packet Size the client advertised); the rest are local
    /// policy and are read straight from [`ClientSettings`].
    fn parser_settings(&self) -> ParserSettings {
        ParserSettings {
            max_bytes_string: self.settings.max_bytes_string,
            max_bytes_binary_data: self.settings.max_bytes_binary_data,
            max_remaining_bytes: self.scratchpad.effective_client_max_remaining_bytes,
            max_subscriptions_len: self.settings.max_subscriptions_len,
            max_user_properties_len: self.settings.max_user_properties_len,
            max_subscription_identifiers_len: self.settings.max_subscription_identifiers_len,
        }
    }

    #[inline(always)]
    fn dispatch<F>(&mut self, f: F) -> Result<(), Error>
    where
        F: FnOnce(
            ClientState,
            &ClientSettings,
            &mut ClientSession,
            &mut ClientScratchpad<Time>,
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

impl<Time> Client<Time>
where
    Time: ProtocolTime,
{
    /// Parses and dispatches every whole control packet in `bytes`, returning
    /// how many bytes were consumed.
    ///
    /// A trailing partial packet is left unconsumed for the caller to retain.
    fn consume_packets(&mut self, bytes: &[u8], received_at: Time) -> Result<usize, Error> {
        let parser_settings = self.parser_settings();
        let mut slice: &[u8] = bytes;

        while !slice.is_empty() {
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
                    // [MQTT-4.13.1-1] Malformed Control Packet is a protocol error and requires
                    // disconnect.
                    let _ = self.dispatch(|_s, set, ses, sp| {
                        queues::fail_protocol_and_disconnect(
                            set,
                            ses,
                            sp,
                            DisconnectReasonCode::MalformedPacket,
                        );
                        (
                            ClientState::Disconnected(crate::state::Disconnected),
                            Err(Error::MalformedPacket),
                        )
                    });
                    return Err(Error::MalformedPacket);
                }
            }
        }

        Ok(bytes.len() - slice.len())
    }
}

impl<Time> Protocol<IncomingData<Time>, UserWriteIn, DriverEventIn> for Client<Time>
where
    Time: ProtocolTime,
{
    type Rout = UserWriteOut;
    type Wout = bytes::Bytes;
    type Eout = DriverEventOut;
    type Error = Error;
    type Time = Time;

    #[tracing::instrument(skip_all)]
    fn handle_read(&mut self, msg: IncomingData<Time>) -> Result<(), Self::Error> {
        let received_at = msg.received_at;

        if self.scratchpad.read_buffer.is_empty() {
            // Nothing pending: parse straight out of the driver's buffer so the
            // common case of whole packets per read copies nothing.
            let consumed = self.consume_packets(&msg.bytes, received_at)?;
            self.scratchpad
                .read_buffer
                .extend_from_slice(&msg.bytes[consumed..]);
        } else {
            self.scratchpad.read_buffer.extend_from_slice(&msg.bytes);
            let mut buffer = core::mem::take(&mut self.scratchpad.read_buffer);
            let consumed = self.consume_packets(&buffer, received_at)?;
            // Drop the consumed prefix by moving the start pointer; the leading
            // capacity is reclaimed by the next `extend_from_slice`.
            buffer.advance(consumed);
            self.scratchpad.read_buffer = buffer;
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn handle_write(&mut self, msg: UserWriteIn) -> Result<(), Self::Error> {
        // Keep-alive activity is tracked in `queues::enqueue_packet`, at the one
        // point where a packet actually reaches the write queue.
        self.dispatch(|s, set, ses, sp| s.handle_write(set, ses, sp, msg))
    }

    #[tracing::instrument(skip_all)]
    fn handle_event(&mut self, evt: DriverEventIn) -> Result<(), Self::Error> {
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
