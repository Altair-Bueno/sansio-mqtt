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
            if let Some(event) = self.try_deliver_event()? {
                return Ok(event);
            }

            self.flush_protocol_writes().await?;
            self.handle_protocol_actions().await?;

            if let Some(event) = self.try_deliver_event()? {
                return Ok(event);
            }

            let timeout = self.protocol.poll_timeout();
            if let Some(deadline) = timeout {
                tokio::select! {
                    read_result = self.stream.read(&mut self.read_buffer) => {
                        self.handle_read_result(read_result)?;
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
                        self.handle_read_result(read_result)?;
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

    fn try_deliver_event(&mut self) -> Result<Option<Event>, EventLoopError> {
        let Some(out) = self.protocol.poll_read() else {
            return Ok(None);
        };
        Ok(Some(Event::from_protocol_output(out)))
    }

    fn handle_read_result(&mut self, result: std::io::Result<usize>) -> Result<(), EventLoopError> {
        match result {
            Ok(0) => self.protocol.handle_event(DriverEventIn::SocketClosed)?,
            Ok(n) => self.protocol.handle_read(IncomingData {
                bytes: self.read_buffer[..n].to_vec().into(),
                received_at: tokio::time::Instant::now(),
            })?,
            Err(e) => {
                // Notify the protocol so it transitions to a clean state before
                // returning the IO error to the caller.
                let _ = self.protocol.handle_event(DriverEventIn::SocketError);
                return Err(e.into());
            }
        }
        Ok(())
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
