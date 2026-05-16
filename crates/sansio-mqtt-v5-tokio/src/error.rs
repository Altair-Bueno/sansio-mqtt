use sansio_mqtt_v5_protocol::DriverEventOut;

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] sansio_mqtt_v5_protocol::Error),
    #[error("unexpected driver action: {0:?}")]
    UnexpectedDriverAction(DriverEventOut),
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] sansio_mqtt_v5_protocol::Error),
    #[error("unexpected driver action: {0:?}")]
    UnexpectedDriverAction(DriverEventOut),
    #[error("broker sent a fatal DISCONNECT")]
    ProtocolRequestedQuit,
    #[error("outbound message queue is full")]
    QueueFull,
    #[error("connection is terminated and no backoff is configured")]
    Disconnected,
}
