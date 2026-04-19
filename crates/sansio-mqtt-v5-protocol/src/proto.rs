use crate::limits;
use crate::queues;
use crate::scratchpad::ClientScratchpad;
use crate::session::ClientSession;
use crate::state::{ClientState, StateHandler};
use crate::types::*;
use bytes::Bytes;
use bytes::BytesMut;
use sansio::Protocol;
use sansio_mqtt_v5_types::ControlPacket;
use sansio_mqtt_v5_types::DisconnectReasonCode;
use sansio_mqtt_v5_types::ParserSettings;
use winnow::error::ErrMode;
use winnow::stream::Partial;
use winnow::Parser;

#[derive(Debug)]
pub struct Client<Time>
where
    Time: 'static,
{
    settings: ClientSettings,
    session: ClientSession,
    scratchpad: ClientScratchpad<Time>,
    state: ClientState,
}

impl<Time> Default for Client<Time>
where
    Time: 'static,
{
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

    fn parser_settings(&self) -> ParserSettings {
        ParserSettings {
            max_bytes_string: self.scratchpad.effective_client_max_bytes_string,
            max_bytes_binary_data: self.scratchpad.effective_client_max_bytes_binary_data,
            max_remaining_bytes: self.scratchpad.effective_client_max_remaining_bytes,
            max_subscriptions_len: self.scratchpad.effective_client_max_subscriptions_len,
            max_user_properties_len: self.scratchpad.effective_client_max_user_properties_len,
            max_subscription_identifiers_len: self
                .scratchpad
                .effective_client_max_subscription_identifiers_len,
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

impl<Time> Protocol<Bytes, UserWriteIn, DriverEventIn> for Client<Time>
where
    Time: Copy + Ord + 'static,
{
    type Rout = UserWriteOut;
    type Wout = Bytes;
    type Eout = DriverEventOut;
    type Error = Error;
    type Time = Time;

    #[tracing::instrument(skip_all)]
    fn handle_read(&mut self, msg: Bytes) -> Result<(), Self::Error> {
        let packet_bytes = if self.scratchpad.read_buffer.is_empty() {
            msg
        } else {
            let mut combined = core::mem::take(&mut self.scratchpad.read_buffer);
            combined.extend_from_slice(&msg);
            combined.freeze()
        };

        let parser_settings = self.parser_settings();
        let mut slice: &[u8] = packet_bytes.as_ref();

        while !slice.is_empty() {
            let mut input = Partial::new(slice);

            match ControlPacket::parser::<_, ErrMode<()>, ErrMode<()>>(&parser_settings)
                .parse_next(&mut input)
            {
                Ok(packet) => {
                    slice = input.into_inner();
                    self.scratchpad.keep_alive_saw_network_activity = true;
                    if matches!(packet, ControlPacket::PingResp(_)) {
                        self.scratchpad.keep_alive_ping_outstanding = false;
                    }
                    self.dispatch(|s, set, ses, sp| s.handle_control_packet(set, ses, sp, packet))?;
                }
                Err(ErrMode::Incomplete(_)) => {
                    break;
                }
                Err(ErrMode::Backtrack(_)) | Err(ErrMode::Cut(_)) => {
                    // [MQTT-4.13.1-1] Malformed Control Packet is a protocol error and requires disconnect.
                    self.dispatch(|_s, set, ses, sp| {
                        let _ = queues::fail_protocol_and_disconnect(
                            set,
                            ses,
                            sp,
                            DisconnectReasonCode::MalformedPacket,
                        );
                        (
                            ClientState::Disconnected(crate::state::Disconnected),
                            Err(Error::MalformedPacket),
                        )
                    })?;
                    return Err(Error::MalformedPacket);
                }
            }
        }

        self.scratchpad.read_buffer = BytesMut::from(slice);

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn handle_write(&mut self, msg: UserWriteIn) -> Result<(), Self::Error> {
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
