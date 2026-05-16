use std::num::NonZero;

use sansio::Protocol;
use sansio_mqtt_v5_protocol::Client as ProtocolClient;
use sansio_mqtt_v5_protocol::ClientMessage;
use sansio_mqtt_v5_protocol::ConnectionOptions;
use sansio_mqtt_v5_protocol::DriverEventIn;
use sansio_mqtt_v5_protocol::DriverEventOut;
use sansio_mqtt_v5_protocol::IncomingData;
use sansio_mqtt_v5_protocol::SubscribeOptions;
use sansio_mqtt_v5_protocol::UnsubscribeOptions;
use sansio_mqtt_v5_protocol::UserWriteIn;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::Instant;

use crate::backoff::compute_delay;
use crate::connect::ConnectOptions;
use crate::error::ConnectError;
use crate::error::ConnectionError;
use crate::event::Event;

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

        let mut protocol =
            ProtocolClient::<Instant>::with_settings(options.protocol_config.clone());

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

        let rng = options.backoff.as_ref().map(|b| b.seed.max(1)).unwrap_or(1);

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

    pub async fn poll(&mut self) -> Result<Event, ConnectionError> {
        loop {
            // Drain buffered protocol output before touching the socket
            if let Some(output) = self.protocol.poll_read() {
                return Ok(Event::from_protocol_output(output));
            }

            if matches!(&self.state, SocketState::Terminal) {
                return Err(ConnectionError::Disconnected);
            }

            if matches!(&self.state, SocketState::Offline { .. }) {
                // Copy wake_at (Instant is Copy) so the immutable borrow is released
                // before the async sleep and before calling &mut self methods.
                let wake_at = if let SocketState::Offline { wake_at, .. } = &self.state {
                    *wake_at
                } else {
                    unreachable!()
                };
                tokio::time::sleep_until(wake_at).await;
                // No borrow of self active here — safe to call &mut self method.
                self.attempt_reconnect().await?;
                continue;
            }

            // --- Active state ---
            // Use a local enum to communicate required state transitions out of the
            // borrow block; we cannot assign self.state while `stream` borrows it.
            enum Transition {
                None,
                Disconnect {
                    reason: Option<sansio_mqtt_v5_protocol::DisconnectReasonCode>,
                    quit: bool,
                },
            }
            let mut transition = Transition::None;

            if let SocketState::Active(stream) = &mut self.state {
                // Flush pending writes (self.protocol is a distinct field — valid).
                while let Some(frame) = self.protocol.poll_write() {
                    if let Err(e) = stream.write_all(&frame).await {
                        return Err(ConnectionError::Io(e));
                    }
                }

                // Handle at most one pending driver event per poll cycle.
                // Additional events are picked up on subsequent outer-loop
                // iterations, which keeps the borrow structure simple.
                if let Some(event) = self.protocol.poll_event() {
                    match event {
                        DriverEventOut::CloseSocket => {
                            stream.shutdown().await.ok();
                            self.protocol.handle_event(DriverEventIn::SocketClosed)?;
                            let reason =
                                match self.protocol.poll_read().map(Event::from_protocol_output) {
                                    Some(Event::Disconnected(r)) => r,
                                    _ => None,
                                };
                            transition = Transition::Disconnect {
                                reason,
                                quit: false,
                            };
                        }
                        DriverEventOut::Quit => {
                            transition = Transition::Disconnect {
                                reason: None,
                                quit: true,
                            };
                        }
                        other => return Err(ConnectionError::UnexpectedDriverAction(other)),
                    }
                }

                if matches!(transition, Transition::None) {
                    // Drain again after events.
                    if let Some(output) = self.protocol.poll_read() {
                        return Ok(Event::from_protocol_output(output));
                    }

                    let timeout = self.protocol.poll_timeout();
                    tokio::select! {
                        result = stream.read(&mut self.read_buffer) => {
                            match result {
                                Ok(0) => {
                                    // EOF: remote closed the TCP connection.
                                    // Inform the protocol, then drain the
                                    // Disconnected event it enqueues so that
                                    // the state transition is handled through
                                    // the normal Transition::Disconnect path
                                    // (which sets self.state = Terminal).
                                    self.protocol.handle_event(DriverEventIn::SocketClosed)?;
                                    let reason = match self
                                        .protocol
                                        .poll_read()
                                        .map(Event::from_protocol_output)
                                    {
                                        Some(Event::Disconnected(r)) => r,
                                        _ => None,
                                    };
                                    transition = Transition::Disconnect { reason, quit: false };
                                }
                                Ok(n) => {
                                    self.protocol.handle_read(IncomingData {
                                        bytes: bytes::Bytes::copy_from_slice(&self.read_buffer[..n]),
                                        received_at: Instant::now(),
                                    })?;
                                }
                                Err(e) => {
                                    self.protocol.handle_event(DriverEventIn::SocketError).ok();
                                    return Err(ConnectionError::Io(e));
                                }
                            }
                        }
                        _ = maybe_sleep_until(timeout) => {
                            self.protocol.handle_timeout(Instant::now())?;
                        }
                    }
                }
            }
            // `stream` borrow released — state transitions are now safe.

            match transition {
                Transition::None => {}
                Transition::Disconnect { quit: true, .. } => {
                    self.state = SocketState::Terminal;
                    return Err(ConnectionError::ProtocolRequestedQuit);
                }
                Transition::Disconnect {
                    reason,
                    quit: false,
                } => {
                    self.state = match &self.options.backoff {
                        Some(b) => {
                            let delay = compute_delay(b, 0, &mut self.rng);
                            SocketState::Offline {
                                attempt: 0,
                                wake_at: Instant::now() + delay,
                            }
                        }
                        None => SocketState::Terminal,
                    };
                    return Ok(Event::Disconnected(reason));
                }
            }
        }
    }

    async fn attempt_reconnect(&mut self) -> Result<(), ConnectionError> {
        let backoff = match self.options.backoff.clone() {
            Some(b) => b,
            None => {
                self.state = SocketState::Terminal;
                return Ok(());
            }
        };
        let current_attempt = if let SocketState::Offline { attempt, .. } = &self.state {
            *attempt
        } else {
            0
        };

        match TcpStream::connect(self.options.addr).await {
            Ok(mut new_stream) => {
                let conn_opts = Self::apply_receive_maximum(
                    self.options.connection.clone(),
                    self.options.max_in_queued_messages,
                );
                self.protocol
                    .handle_write(UserWriteIn::Connect(conn_opts))?;
                match self.protocol.poll_event() {
                    Some(DriverEventOut::OpenSocket) => {}
                    Some(other) => return Err(ConnectionError::UnexpectedDriverAction(other)),
                    None => {}
                }
                self.protocol.handle_event(DriverEventIn::SocketConnected)?;
                Self::flush_writes(&mut new_stream, &mut self.protocol).await?;
                self.state = SocketState::Active(new_stream);
                // Next poll() iteration drains poll_read() which returns
                // Event::Connected once the CONNACK arrives via
                // the Active socket-read path.
            }
            Err(_) => {
                let next_attempt = current_attempt.saturating_add(1);
                let delay = compute_delay(&backoff, next_attempt, &mut self.rng);
                self.state = SocketState::Offline {
                    attempt: next_attempt,
                    wake_at: Instant::now() + delay,
                };
            }
        }
        Ok(())
    }
}

async fn maybe_sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(d) => tokio::time::sleep_until(d).await,
        None => std::future::pending().await,
    }
}
