use sansio_mqtt_v5_contract::ProtocolError;

use heapless::FnvIndexSet;

pub const DEFAULT_PACKET_ID_TRACKING_CAPACITY: usize = 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct ClientState<const N: usize = DEFAULT_PACKET_ID_TRACKING_CAPACITY> {
    next_packet_id: u16,
    in_use: FnvIndexSet<u16, N>,
}

impl<const N: usize> ClientState<N> {
    #[must_use]
    pub fn new(next_packet_id: u16) -> Self {
        Self {
            next_packet_id,
            in_use: FnvIndexSet::new(),
        }
    }

    pub fn allocate_packet_id(&mut self) -> Result<u16, ProtocolError> {
        let mut attempts = 0u16;
        loop {
            let candidate = self.next_packet_id;
            self.bump_next_packet_id();

            if candidate != 0 && !self.in_use.contains(&candidate) {
                return self
                    .in_use
                    .insert(candidate)
                    .map(|_| candidate)
                    .map_err(|_| ProtocolError::PacketIdExhausted);
            }

            if attempts == u16::MAX {
                break;
            }
            attempts = attempts.wrapping_add(1);
        }

        Err(ProtocolError::PacketIdExhausted)
    }

    fn bump_next_packet_id(&mut self) {
        self.next_packet_id = self.next_packet_id.wrapping_add(1);
    }
}

impl<const N: usize> Default for ClientState<N> {
    fn default() -> Self {
        Self::new(1)
    }
}
