use std::collections::HashMap;
use std::future::poll_fn;
use std::hash::Hash;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use tokio_util::time::{delay_queue, DelayQueue};

#[derive(Debug)]
pub struct TimerMap<K>
where
    K: Eq + Hash + Clone,
{
    queue: DelayQueue<K>,
    keys: HashMap<K, delay_queue::Key>,
}

impl<K> Default for TimerMap<K>
where
    K: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K> TimerMap<K>
where
    K: Eq + Hash + Clone,
{
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: DelayQueue::new(),
            keys: HashMap::new(),
        }
    }

    pub fn schedule(&mut self, key: K, delay_ms: u32) {
        if let Some(old_key) = self.keys.remove(&key) {
            let _ = self.queue.remove(&old_key);
        }

        let entry_key = self
            .queue
            .insert(key.clone(), Duration::from_millis(u64::from(delay_ms)));
        self.keys.insert(key, entry_key);
    }

    pub fn cancel(&mut self, key: &K) -> bool {
        let Some(queue_key) = self.keys.remove(key) else {
            return false;
        };

        let _ = self.queue.remove(&queue_key);
        true
    }

    pub async fn next_expired(&mut self) -> K {
        let expired = poll_fn(|cx| match Pin::new(&mut self.queue).poll_expired(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(item),
            Poll::Ready(None) | Poll::Pending => Poll::Pending,
        })
        .await;

        let key = expired.into_inner();
        self.keys.remove(&key);
        key
    }
}
