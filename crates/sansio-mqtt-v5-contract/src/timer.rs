#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKey {
    Keepalive,
    PingRespTimeout,
    AckTimeout(u16),
    ConnectTimeout,
}
