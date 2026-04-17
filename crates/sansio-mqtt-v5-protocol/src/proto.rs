use core::num::NonZero;

use crate::types::*;
use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;
use bimap::BiBTreeMap;
use bytes::Bytes;
use bytes::BytesMut;
use sansio::Protocol;
use sansio_mqtt_v5_types::Topic;

#[derive(Debug, PartialEq, Default)]
enum ClientState {
    #[default]
    Start,
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, PartialEq)]
pub struct Client<Time>
where
    Time: 'static,
{
    config: Config,
    state: ClientState,

    // Buffer for accumulating incoming bytes until a full control packet can be parsed
    read_buffer: BytesMut,

    // Pending packages to be acknowledged indexed by packet identifier
    on_flight_sent: BTreeMap<NonZero<u16>, ClientMessage>,
    on_flight_received: BTreeMap<NonZero<u16>, ClientMessage>,

    // Output queues
    read_queue: VecDeque<UserWriteOut>,
    write_queue: VecDeque<Bytes>,
    action_queue: VecDeque<DriverEventOut>,
    next_timeout: Option<Time>,
}

impl<Time> Default for Client<Time> {
    fn default() -> Self {
        Self {
            config: Config::default(),
            state: ClientState::default(),
            read_buffer: BytesMut::new(),
            on_flight_sent: BTreeMap::new(),
            on_flight_received: BTreeMap::new(),
            read_queue: VecDeque::new(),
            write_queue: VecDeque::new(),
            action_queue: VecDeque::new(),
            next_timeout: None,
        }
    }
}

impl<Time> Client<Time> {
    pub fn with_config(config: Config) -> Self {
        Self {
            config,
            ..Self::default()
        }
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
        todo!()
    }

    #[tracing::instrument(skip_all)]
    fn handle_write(&mut self, msg: UserWriteIn) -> Result<(), Self::Error> {
        todo!()
    }

    #[tracing::instrument(skip_all)]
    fn handle_event(&mut self, evt: DriverEventIn) -> Result<(), Self::Error> {
        todo!()
    }

    #[tracing::instrument(skip_all)]
    fn handle_timeout(&mut self, now: Self::Time) -> Result<(), Self::Error> {
        todo!()
    }

    #[tracing::instrument(skip_all)]
    fn close(&mut self) -> Result<(), Self::Error> {
        todo!()
    }

    fn poll_read(&mut self) -> Option<Self::Rout> {
        self.read_queue.pop_front()
    }

    fn poll_write(&mut self) -> Option<Self::Wout> {
        self.write_queue.pop_front()
    }

    fn poll_event(&mut self) -> Option<Self::Eout> {
        self.action_queue.pop_front()
    }

    fn poll_timeout(&mut self) -> Option<Self::Time> {
        self.next_timeout
    }
}
