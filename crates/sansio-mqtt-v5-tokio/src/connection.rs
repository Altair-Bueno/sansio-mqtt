use std::num::NonZero;

use sansio::Protocol;
use sansio_mqtt_v5_protocol::Client as ProtocolClient;
use sansio_mqtt_v5_protocol::ClientMessage;
use sansio_mqtt_v5_protocol::ConnectionOptions;
use sansio_mqtt_v5_protocol::DriverEventIn;
use sansio_mqtt_v5_protocol::DriverEventOut;
use sansio_mqtt_v5_protocol::SubscribeOptions;
use sansio_mqtt_v5_protocol::UnsubscribeOptions;
use sansio_mqtt_v5_protocol::UserWriteIn;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::Instant;

use crate::connect::ConnectOptions;
use crate::error::ConnectError;
use crate::error::ConnectionError;

enum SocketState {
    Active(TcpStream),
    Offline { attempt: u32, wake_at: Instant },
    Terminal,
}

pub struct Connection {
    state: SocketState,
    protocol: ProtocolClient<Instant>,
    options: ConnectOptions,
    rng: u64,
    read_buffer: [u8; 4096],
}

impl Connection {
    pub async fn connect(options: ConnectOptions) -> Result<Self, ConnectError> {
        let mut stream = TcpStream::connect(options.addr).await?;

        let mut protocol = ProtocolClient::<Instant>::with_settings(options.protocol_config.clone());

        let conn_opts =
            Self::apply_receive_maximum(options.connection.clone(), options.max_in_queued_messages);
        protocol.handle_write(UserWriteIn::Connect(conn_opts))?;

        match protocol.poll_event() {
            Some(DriverEventOut::OpenSocket) => {}
            Some(other) => return Err(ConnectError::UnexpectedDriverAction(other)),
            None => {}
        }
        protocol.handle_event(DriverEventIn::SocketConnected)?;

        Self::flush_writes_to(&mut stream, &mut protocol).await?;

        let rng = options
            .backoff
            .as_ref()
            .map(|b| b.seed.max(1))
            .unwrap_or(1);

        Ok(Self {
            state: SocketState::Active(stream),
            protocol,
            options,
            rng,
            read_buffer: [0u8; 4096],
        })
    }

    fn apply_receive_maximum(mut conn: ConnectionOptions, max_in: usize) -> ConnectionOptions {
        if let Some(cap) = NonZero::new(max_in.min(u16::MAX as usize) as u16) {
            conn.receive_maximum = Some(match conn.receive_maximum {
                Some(existing) => existing.min(cap),
                None => cap,
            });
        }
        conn
    }

    async fn flush_writes_to(
        stream: &mut TcpStream,
        protocol: &mut ProtocolClient<Instant>,
    ) -> Result<(), ConnectError> {
        while let Some(frame) = protocol.poll_write() {
            stream.write_all(&frame).await?;
        }
        Ok(())
    }

    pub(crate) async fn flush_writes(
        stream: &mut TcpStream,
        protocol: &mut ProtocolClient<Instant>,
    ) -> Result<(), ConnectionError> {
        while let Some(frame) = protocol.poll_write() {
            stream.write_all(&frame).await?;
        }
        Ok(())
    }

    pub fn publish(&mut self, message: ClientMessage) -> Result<(), ConnectionError> {
        if self.protocol.outbound_inflight_count() >= self.options.max_out_queued_messages {
            return Err(ConnectionError::QueueFull);
        }
        self.protocol
            .handle_write(UserWriteIn::PublishMessage(message))?;
        Ok(())
    }

    pub fn subscribe(&mut self, options: SubscribeOptions) -> Result<(), ConnectionError> {
        self.protocol
            .handle_write(UserWriteIn::Subscribe(options))?;
        Ok(())
    }

    pub fn unsubscribe(&mut self, options: UnsubscribeOptions) -> Result<(), ConnectionError> {
        self.protocol
            .handle_write(UserWriteIn::Unsubscribe(options))?;
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), ConnectionError> {
        self.protocol.handle_write(UserWriteIn::Disconnect)?;
        Ok(())
    }
}
