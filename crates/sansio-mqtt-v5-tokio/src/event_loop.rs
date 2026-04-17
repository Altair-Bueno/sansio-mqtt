use sansio::Protocol;
use sansio_mqtt_v5_protocol::{
    Client as ProtocolClient, DriverEventIn, DriverEventOut, UserWriteIn,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::{Event, EventLoopError};

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

            self.flush_protocol_writes().await?;
            self.handle_protocol_actions().await?;

            if let Some(out) = self.protocol.poll_read() {
                return Ok(Event::from_protocol_output(out));
            }

            let timeout = self.protocol.poll_timeout();
            if let Some(deadline) = timeout {
                tokio::select! {
                    read_result = self.stream.read(&mut self.read_buffer) => {
                        let read = read_result?;
                        if read == 0 {
                            self.protocol.handle_event(DriverEventIn::SocketClosed)?;
                        } else {
                            self.protocol.handle_read(self.read_buffer[..read].to_vec().into())?;
                        }
                    }
                    command = self.command_rx.recv() => {
                        if let Some(command) = command {
                            self.protocol.handle_write(command)?;
                        }
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        self.protocol.handle_timeout(tokio::time::Instant::now())?;
                    }
                }
            } else {
                tokio::select! {
                    read_result = self.stream.read(&mut self.read_buffer) => {
                        let read = read_result?;
                        if read == 0 {
                            self.protocol.handle_event(DriverEventIn::SocketClosed)?;
                        } else {
                            self.protocol.handle_read(self.read_buffer[..read].to_vec().into())?;
                        }
                    }
                    command = self.command_rx.recv() => {
                        if let Some(command) = command {
                            self.protocol.handle_write(command)?;
                        }
                    }
                }
            }
        }
    }

    async fn flush_protocol_writes(&mut self) -> Result<(), EventLoopError> {
        while let Some(frame) = self.protocol.poll_write() {
            self.stream.write_all(&frame).await?;
        }
        Ok(())
    }

    async fn handle_protocol_actions(&mut self) -> Result<(), EventLoopError> {
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

        Ok(())
    }
}
