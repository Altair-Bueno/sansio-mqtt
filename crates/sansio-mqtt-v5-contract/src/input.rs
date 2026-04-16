use crate::timer::TimerKey;
use crate::{ConnectOptions, PublishRequest, SubscribeRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input<'a> {
    BytesReceived(&'a [u8]),
    TimerFired(TimerKey),
    UserConnect(ConnectOptions),
    UserPublish(PublishRequest),
    UserSubscribe(SubscribeRequest),
    UserDisconnect,
}
