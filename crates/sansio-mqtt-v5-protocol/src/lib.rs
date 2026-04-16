#![no_std]
#![forbid(unsafe_code)]

mod client;
mod error;
mod timer_queue;

pub use client::ClientState;
pub use error::TimerQueueError;
pub use timer_queue::TimerQueue;

use heapless::{Deque, String, Vec};
use sansio::Protocol;
use sansio_mqtt_v5_contract::{
    Action, ConnectOptions, Input, ProtocolError, PublishRequest, Qos, SessionAction,
    SubscribeRequest, SESSION_ACTION_PAYLOAD_CAPACITY, SESSION_ACTION_TOPIC_CAPACITY,
};
use sansio_mqtt_v5_state_machine::StateMachine;

const TIMER_CAPACITY: usize = 8;
const WRITE_QUEUE_CAPACITY: usize = 16;
const EVENT_QUEUE_CAPACITY: usize = 16;

type Frame = Vec<u8, 256>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)] // Request payloads are intentionally inline in owned event variants.
pub enum ProtocolEvent {
    Connect(ConnectOptions),
    Publish(PublishRequest),
    Subscribe(SubscribeRequest),
    Disconnect,
}

pub struct MqttProtocol {
    client: ClientState,
    timers: TimerQueue<TIMER_CAPACITY>,
    machine: StateMachine,
    write_queue: Deque<Frame, WRITE_QUEUE_CAPACITY>,
    event_queue: Deque<SessionAction, EVENT_QUEUE_CAPACITY>,
    now_ms: u32,
}

impl MqttProtocol {
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: ClientState::new(1),
            timers: TimerQueue::new(),
            machine: StateMachine::new_default(),
            write_queue: Deque::new(),
            event_queue: Deque::new(),
            now_ms: 0,
        }
    }

    #[must_use]
    pub const fn client(&self) -> &ClientState {
        &self.client
    }

    #[must_use]
    pub const fn timers(&self) -> &TimerQueue<TIMER_CAPACITY> {
        &self.timers
    }

    fn dispatch_input(&mut self, input: Input<'_>) -> Result<(), ProtocolError> {
        let actions = self.machine.handle(input);
        self.apply_actions(&actions)
    }

    fn apply_actions(&mut self, actions: &[Action]) -> Result<(), ProtocolError> {
        for action in actions {
            match action {
                Action::SendBytes(bytes) => {
                    if self.write_queue.push_back(bytes.clone()).is_err() {
                        return Err(ProtocolError::UnexpectedPacket);
                    }
                }
                Action::SessionAction(session_action) => {
                    if self.event_queue.push_back(session_action.clone()).is_err() {
                        return Err(ProtocolError::UnexpectedPacket);
                    }
                }
                Action::ScheduleTimer { key, delay_ms } => {
                    let deadline = self.now_ms.wrapping_add(*delay_ms);
                    self.timers
                        .insert(*key, deadline)
                        .map_err(|_| ProtocolError::UnexpectedPacket)?;
                }
                Action::CancelTimer(key) => {
                    self.timers.cancel(*key);
                }
            }
        }

        Ok(())
    }
}

impl Default for MqttProtocol {
    fn default() -> Self {
        Self::new()
    }
}

impl Protocol<Frame, (), ProtocolEvent> for MqttProtocol {
    type Rout = ();
    type Wout = Frame;
    type Eout = SessionAction;
    type Error = ProtocolError;
    type Time = u32;

    fn handle_read(&mut self, msg: Frame) -> Result<(), Self::Error> {
        let decoded = decode_input(msg.as_slice())?;
        self.dispatch_input(decoded)
    }

    fn poll_read(&mut self) -> Option<Self::Rout> {
        None
    }

    fn handle_write(&mut self, _msg: ()) -> Result<(), Self::Error> {
        Ok(())
    }

    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.write_queue.pop_front()
    }

    fn handle_event(&mut self, evt: ProtocolEvent) -> Result<(), Self::Error> {
        match evt {
            ProtocolEvent::Connect(options) => self.dispatch_input(Input::UserConnect(options)),
            ProtocolEvent::Publish(request) => self.dispatch_input(Input::UserPublish(request)),
            ProtocolEvent::Subscribe(request) => self.dispatch_input(Input::UserSubscribe(request)),
            ProtocolEvent::Disconnect => self.dispatch_input(Input::UserDisconnect),
        }
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.event_queue.pop_front()
    }

    fn handle_timeout(&mut self, now: Self::Time) -> Result<(), Self::Error> {
        self.now_ms = now;

        while let Some(key) = self.timers.expired(now) {
            self.dispatch_input(Input::TimerFired(key))?;
        }

        Ok(())
    }

    fn poll_timeout(&mut self) -> Option<Self::Time> {
        self.timers.next_deadline()
    }

    fn close(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub trait ProtocolAnchor: Protocol<Frame, (), ProtocolEvent> {}

impl<T> ProtocolAnchor for T where T: Protocol<Frame, (), ProtocolEvent> {}

fn decode_input(bytes: &[u8]) -> Result<Input<'static>, ProtocolError> {
    if bytes.is_empty() {
        return Err(ProtocolError::DecodeError);
    }

    let packet_type = bytes[0] >> 4;
    let header_flags = bytes[0] & 0x0F;
    match packet_type {
        2 => {
            if header_flags != 0 {
                return Err(ProtocolError::DecodeError);
            }
            Ok(Input::PacketConnAck)
        }
        3 => decode_publish(bytes),
        4 => {
            if header_flags != 0 {
                return Err(ProtocolError::DecodeError);
            }
            decode_ack_packet_id(bytes).map(|packet_id| Input::PacketPubAck { packet_id })
        }
        5 => {
            if header_flags != 0 {
                return Err(ProtocolError::DecodeError);
            }
            decode_ack_packet_id(bytes).map(|packet_id| Input::PacketPubRec { packet_id })
        }
        6 => {
            if header_flags != 0b0010 {
                return Err(ProtocolError::DecodeError);
            }
            decode_ack_packet_id(bytes).map(|packet_id| Input::PacketPubRel { packet_id })
        }
        7 => {
            if header_flags != 0 {
                return Err(ProtocolError::DecodeError);
            }
            decode_ack_packet_id(bytes).map(|packet_id| Input::PacketPubComp { packet_id })
        }
        9 => {
            if header_flags != 0 {
                return Err(ProtocolError::DecodeError);
            }
            decode_suback(bytes)
        }
        13 => {
            if header_flags != 0 {
                return Err(ProtocolError::DecodeError);
            }
            Ok(Input::PacketPingResp)
        }
        _ => Err(ProtocolError::DecodeError),
    }
}

fn decode_ack_packet_id(bytes: &[u8]) -> Result<u16, ProtocolError> {
    let (remaining_len, remaining_len_bytes) = decode_variable_byte_integer(&bytes[1..])?;
    let variable_header_start = 1usize
        .checked_add(remaining_len_bytes)
        .ok_or(ProtocolError::DecodeError)?;
    let packet_end = variable_header_start
        .checked_add(remaining_len)
        .ok_or(ProtocolError::DecodeError)?;
    if packet_end != bytes.len() {
        return Err(ProtocolError::DecodeError);
    }
    if remaining_len < 2 {
        return Err(ProtocolError::DecodeError);
    }

    read_u16(bytes, variable_header_start)
}

fn decode_suback(bytes: &[u8]) -> Result<Input<'static>, ProtocolError> {
    let (remaining_len, remaining_len_bytes) = decode_variable_byte_integer(&bytes[1..])?;
    let variable_header_start = 1usize
        .checked_add(remaining_len_bytes)
        .ok_or(ProtocolError::DecodeError)?;
    let packet_end = variable_header_start
        .checked_add(remaining_len)
        .ok_or(ProtocolError::DecodeError)?;
    if packet_end != bytes.len() {
        return Err(ProtocolError::DecodeError);
    }
    if remaining_len < 3 {
        return Err(ProtocolError::DecodeError);
    }

    let packet_id = read_u16(bytes, variable_header_start)?;
    let properties_len_start = variable_header_start
        .checked_add(2)
        .ok_or(ProtocolError::DecodeError)?;
    let (property_len, property_len_bytes) =
        decode_variable_byte_integer(&bytes[properties_len_start..])?;
    let reason_start = properties_len_start
        .checked_add(property_len_bytes)
        .ok_or(ProtocolError::DecodeError)?
        .checked_add(property_len)
        .ok_or(ProtocolError::DecodeError)?;
    if reason_start > packet_end {
        return Err(ProtocolError::DecodeError);
    }

    let mut reason_codes = Vec::new();
    for code in bytes[reason_start..packet_end].iter().copied() {
        reason_codes
            .push(code)
            .map_err(|_| ProtocolError::DecodeError)?;
    }

    Ok(Input::PacketSubAck {
        packet_id,
        reason_codes,
    })
}

fn decode_publish(bytes: &[u8]) -> Result<Input<'static>, ProtocolError> {
    // [MQTT-3.3.1-1] QoS is encoded in bits 2 and 1 of the fixed header.
    let qos = match (bytes[0] >> 1) & 0x03 {
        0 => Qos::AtMost,
        1 => Qos::AtLeast,
        2 => Qos::Exactly,
        _ => return Err(ProtocolError::DecodeError),
    };

    let (remaining_len, remaining_len_bytes) = decode_variable_byte_integer(&bytes[1..])?;
    let variable_header_start = 1usize
        .checked_add(remaining_len_bytes)
        .ok_or(ProtocolError::DecodeError)?;
    let packet_end = variable_header_start
        .checked_add(remaining_len)
        .ok_or(ProtocolError::DecodeError)?;
    if packet_end != bytes.len() {
        return Err(ProtocolError::DecodeError);
    }

    let topic_len = usize::from(read_u16(bytes, variable_header_start)?);
    let topic_start = variable_header_start
        .checked_add(2)
        .ok_or(ProtocolError::DecodeError)?;
    let topic_end = topic_start
        .checked_add(topic_len)
        .ok_or(ProtocolError::DecodeError)?;
    if topic_end > packet_end {
        return Err(ProtocolError::DecodeError);
    }

    let mut topic = String::<SESSION_ACTION_TOPIC_CAPACITY>::new();
    let topic_str = core::str::from_utf8(&bytes[topic_start..topic_end])
        .map_err(|_| ProtocolError::DecodeError)?;
    topic
        .push_str(topic_str)
        .map_err(|_| ProtocolError::DecodeError)?;

    let mut cursor = topic_end;

    let packet_id = match qos {
        Qos::AtMost => None,
        Qos::AtLeast | Qos::Exactly => {
            let id = read_u16(bytes, cursor)?;
            cursor = cursor.checked_add(2).ok_or(ProtocolError::DecodeError)?;
            Some(id)
        }
    };

    let (property_len, property_len_bytes) = decode_variable_byte_integer(
        bytes
            .get(cursor..packet_end)
            .ok_or(ProtocolError::DecodeError)?,
    )?;
    cursor = cursor
        .checked_add(property_len_bytes)
        .ok_or(ProtocolError::DecodeError)?;
    let payload_start = cursor
        .checked_add(property_len)
        .ok_or(ProtocolError::DecodeError)?;
    if payload_start > packet_end {
        return Err(ProtocolError::DecodeError);
    }

    let mut payload = Vec::<u8, SESSION_ACTION_PAYLOAD_CAPACITY>::new();
    for byte in bytes[payload_start..packet_end].iter().copied() {
        payload.push(byte).map_err(|_| ProtocolError::DecodeError)?;
    }

    Ok(Input::PacketPublish {
        topic,
        payload,
        qos,
        packet_id,
    })
}

fn decode_variable_byte_integer(bytes: &[u8]) -> Result<(usize, usize), ProtocolError> {
    let mut multiplier = 1usize;
    let mut value = 0usize;

    for (index, encoded_byte) in bytes.iter().copied().take(4).enumerate() {
        value = value
            .checked_add(usize::from(encoded_byte & 0x7F) * multiplier)
            .ok_or(ProtocolError::DecodeError)?;
        if encoded_byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
        multiplier = multiplier
            .checked_mul(128)
            .ok_or(ProtocolError::DecodeError)?;
    }

    Err(ProtocolError::DecodeError)
}

fn read_u16(bytes: &[u8], start: usize) -> Result<u16, ProtocolError> {
    let end = start.checked_add(2).ok_or(ProtocolError::DecodeError)?;
    let field = bytes.get(start..end).ok_or(ProtocolError::DecodeError)?;
    Ok((u16::from(field[0]) << 8) | u16::from(field[1]))
}
