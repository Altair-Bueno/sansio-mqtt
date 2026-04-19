use sansio::Protocol;
use sansio_mqtt_v5_protocol::{
    Client as ProtocolClient, ClientSettings, ConnectionOptions, DriverEventIn, DriverEventOut,
    UserWriteIn,
};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::{Client, ConnectError, EventLoop};

#[derive(Clone, Debug)]
pub struct ConnectOptions {
    pub addr: std::net::SocketAddr,
    pub connection: ConnectionOptions,
    pub protocol_config: ClientSettings,
    pub command_channel_capacity: usize,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            addr: std::net::SocketAddr::from(([127, 0, 0, 1], 1883)),
            connection: ConnectionOptions::default(),
            protocol_config: ClientSettings::default(),
            command_channel_capacity: 16,
        }
    }
}

pub async fn connect(options: ConnectOptions) -> Result<(Client, EventLoop), ConnectError> {
    let mut stream = TcpStream::connect(options.addr).await?;
    let mut protocol =
        ProtocolClient::<tokio::time::Instant>::with_settings(options.protocol_config);

    protocol.handle_write(UserWriteIn::Connect(options.connection))?;

    while let Some(action) = protocol.poll_event() {
        if !matches!(action, DriverEventOut::OpenSocket) {
            return Err(ConnectError::UnexpectedDriverAction(action));
        }
    }

    protocol.handle_event(DriverEventIn::SocketConnected)?;

    while let Some(frame) = protocol.poll_write() {
        stream.write_all(&frame).await?;
    }

    let (tx, rx) = mpsc::channel(options.command_channel_capacity.max(1));
    let client = Client::new(tx);
    let event_loop = EventLoop::new(stream, protocol, rx);

    Ok((client, event_loop))
}
