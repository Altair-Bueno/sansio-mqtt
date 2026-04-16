use crate::TimerQueueError;
use sansio_mqtt_v5_contract::TimerKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimerEntry {
    key: TimerKey,
    deadline: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerQueue<const N: usize> {
    entries: [Option<TimerEntry>; N],
}

impl<const N: usize> TimerQueue<N> {
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: [None; N] }
    }

    pub fn insert(&mut self, key: TimerKey, deadline: u32) -> Result<(), TimerQueueError> {
        if let Some(index) = self.find_index(key) {
            self.entries[index] = Some(TimerEntry { key, deadline });
            return Ok(());
        }

        if let Some(index) = self.first_free_index() {
            self.entries[index] = Some(TimerEntry { key, deadline });
            return Ok(());
        }

        Err(TimerQueueError::Full)
    }

    pub fn cancel(&mut self, key: TimerKey) -> bool {
        if let Some(index) = self.find_index(key) {
            self.entries[index] = None;
            return true;
        }

        false
    }

    pub fn expired(&mut self, now: u32) -> Option<TimerKey> {
        let index = self.next_expired_index(now)?;
        let entry = self.entries[index].take()?;
        Some(entry.key)
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<u32> {
        let mut next: Option<u32> = None;
        let mut index = 0;
        while index < N {
            if let Some(entry) = self.entries[index] {
                next = match next {
                    Some(current) if current <= entry.deadline => Some(current),
                    _ => Some(entry.deadline),
                };
            }
            index += 1;
        }
        next
    }

    fn find_index(&self, key: TimerKey) -> Option<usize> {
        let mut index = 0;
        while index < N {
            if matches!(self.entries[index], Some(entry) if entry.key == key) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn first_free_index(&self) -> Option<usize> {
        let mut index = 0;
        while index < N {
            if self.entries[index].is_none() {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn next_expired_index(&self, now: u32) -> Option<usize> {
        let mut best_index: Option<usize> = None;
        let mut index = 0;
        while index < N {
            if let Some(entry) = self.entries[index] {
                if entry.deadline <= now {
                    best_index = match best_index {
                        Some(current_index)
                            if self.entries[current_index]
                                .is_some_and(|current| current.deadline <= entry.deadline) =>
                        {
                            Some(current_index)
                        }
                        _ => Some(index),
                    };
                }
            }
            index += 1;
        }
        best_index
    }
}

impl<const N: usize> Default for TimerQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}
