use sansio::Protocol;
use sansio_mqtt_v5_protocol::Client as ProtocolClient;
use sansio_mqtt_v5_protocol::DriverEventIn;
use sansio_mqtt_v5_protocol::DriverEventOut;
use sansio_mqtt_v5_protocol::IncomingData;
use sansio_mqtt_v5_protocol::UserWriteIn;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::Event;
use crate::EventLoopError;

#[derive(Debug)]
pub struct EventLoop {
    stream: TcpStream,
    protocol: ProtocolClient<tokio::time::Instant>,
    command_rx: mpsc::Receiver<UserWriteIn>,
    read_buffer: [u8; 4096],
}

impl EventLoop {
    pub(crate) fn new(
        stream: TcpStream,
        protocol: ProtocolClient<tokio::time::Instant>,
        command_rx: mpsc::Receiver<UserWriteIn>,
    ) -> Self {
        Self {
            stream,
            protocol,
            command_rx,
            read_buffer: [0; 4096],
        }
    }

    pub async fn poll(&mut self) -> Result<Event, EventLoopError> {
        loop {
            if let Some(out) = self.protocol.poll_read() {
                return Ok(Event::from_protocol_output(out));
            }

            while let Some(frame) = self.protocol.poll_write() {
                self.stream.write_all(&frame).await?;
            }

            while let Some(action) = self.protocol.poll_event() {
                match action {
                    DriverEventOut::CloseSocket => {
                        self.stream.shutdown().await?;
                        self.protocol.handle_event(DriverEventIn::SocketClosed)?;
                    }
                    DriverEventOut::Quit => {
                        return Err(EventLoopError::ProtocolRequestedQuit);
                    }
                    DriverEventOut::OpenSocket => {
                        return Err(EventLoopError::UnexpectedDriverAction(action));
                    }
                }
            }

            if let Some(out) = self.protocol.poll_read() {
                return Ok(Event::from_protocol_output(out));
            }

            let timeout = self.protocol.poll_timeout();
            tokio::select! {
                read_result = self.stream.read(&mut self.read_buffer) => {
                    match read_result {
                        Ok(0) => self.protocol.handle_event(DriverEventIn::SocketClosed)?,
                        Ok(n) => self.protocol.handle_read(IncomingData {
                            bytes: self.read_buffer[..n].to_vec().into(),
                            received_at: tokio::time::Instant::now(),
                        })?,
                        Err(e) => {
                            _ = self.protocol.handle_event(DriverEventIn::SocketError);
                            return Err(e.into());
                        }
                    }
                }
                command = self.command_rx.recv() => {
                    if let Some(command) = command {
                        self.protocol.handle_write(command)?;
                    }
                }
                _ = maybe_sleep_until(timeout) => {
                    self.protocol.handle_timeout(tokio::time::Instant::now())?;
                }
            }
        }
    }
}

async fn maybe_sleep_until(deadline: Option<tokio::time::Instant>) {
    if let Some(deadline) = deadline {
        tokio::time::sleep_until(deadline).await;
    } else {
        core::future::pending().await
    }
}
