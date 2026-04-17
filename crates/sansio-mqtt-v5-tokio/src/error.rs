#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    Closed,
}

#[derive(Debug)]
pub enum ConnectError {
    Io(std::io::Error),
    Protocol(sansio_mqtt_v5_protocol::Error),
    UnexpectedDriverAction(sansio_mqtt_v5_protocol::DriverEventOut),
}

#[derive(Debug)]
pub enum EventLoopError {
    Io(std::io::Error),
    Protocol(sansio_mqtt_v5_protocol::Error),
    UnexpectedDriverAction(sansio_mqtt_v5_protocol::DriverEventOut),
    ProtocolRequestedQuit,
}

impl core::fmt::Display for ClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Closed => f.write_str("event loop is closed"),
        }
    }
}

impl core::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Protocol(err) => write!(f, "protocol error: {err}"),
            Self::UnexpectedDriverAction(action) => {
                write!(
                    f,
                    "unexpected protocol driver action during connect: {action:?}"
                )
            }
        }
    }
}

impl core::fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Protocol(err) => write!(f, "protocol error: {err}"),
            Self::UnexpectedDriverAction(action) => {
                write!(
                    f,
                    "unexpected protocol driver action while running: {action:?}"
                )
            }
            Self::ProtocolRequestedQuit => f.write_str("protocol requested quit while running"),
        }
    }
}

impl std::error::Error for ClientError {}
impl std::error::Error for ConnectError {}
impl std::error::Error for EventLoopError {}

impl From<std::io::Error> for ConnectError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<sansio_mqtt_v5_protocol::Error> for ConnectError {
    fn from(value: sansio_mqtt_v5_protocol::Error) -> Self {
        Self::Protocol(value)
    }
}

impl From<std::io::Error> for EventLoopError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<sansio_mqtt_v5_protocol::Error> for EventLoopError {
    fn from(value: sansio_mqtt_v5_protocol::Error) -> Self {
        Self::Protocol(value)
    }
}
