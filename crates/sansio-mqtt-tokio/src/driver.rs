use bytes::Bytes;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::MqttProtocol;
use std::future::pending;
use std::io;
use tokio::io::AsyncBufRead;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::sleep_until;

#[derive(Debug)]
pub struct Driver<Protocol, Socket, MakeSocket> {
    client: Protocol,
    maybe_socket: Option<Socket>,
    connect: MakeSocket,
    commands: mpsc::Receiver<Command>,
    commands_closed: bool,
    events: mpsc::Sender<Event>,
}

impl<Protocol, Socket, MakeSocket> Driver<Protocol, Socket, MakeSocket> {
    pub fn new(
        client: Protocol,
        connect: MakeSocket,
        commands: mpsc::Receiver<Command>,
        events: mpsc::Sender<Event>,
    ) -> Self {
        Self {
            client,
            connect,
            commands,
            events,
            commands_closed: false,
            maybe_socket: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Idle,
    Quit,
}

impl<Protocol, Socket, MakeSocket, MakeSocketFuture> Driver<Protocol, Socket, MakeSocket>
where
    Protocol: for<'bytes> MqttProtocol<'bytes, Instant>,
    Socket: AsyncBufRead + AsyncWrite + Unpin,
    MakeSocket: FnMut() -> MakeSocketFuture,
    MakeSocketFuture: Future<Output = io::Result<Socket>>,
{
    #[tracing::instrument(level = "debug", skip_all, err)]
    pub async fn run(mut self) -> Result<(), Error> {
        tracing::debug!("driver started");
        while self.process().await? != Flow::Quit {}
        tracing::debug!("driver stopped");

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip_all, err)]
    async fn process(&mut self) -> Result<Flow, Error> {
        let deadline = self.client.poll_timeout();

        tracing::trace!(
            connected = self.maybe_socket.is_some(),
            commands_closed = self.commands_closed,
            ?deadline,
            "waiting for socket, command or timeout"
        );
        tokio::select! {
            result = maybe_read(&mut self.maybe_socket) => {
                match result {
                    Ok(bytes) => {
                        tracing::trace!(len = bytes.len(), "read bytes from socket");
                        let consumed = bytes.len();
                        self.client.handle_read(IncomingData{ bytes, received_at: Instant::now() })?;
                        self.maybe_socket.as_mut().ok_or(Error::ProtocolError)?.consume(consumed);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "socket read failed, reporting socket error to the protocol");
                        self.client.handle_event(DriverEvent::SocketError)?;
                    }
                }
            }
            maybe_command = self.commands.recv(), if !self.commands_closed => {
                match maybe_command {
                    Some(command) => {
                        tracing::debug!(?command, "received command from the application");
                        self.client.handle_write(command)?;
                    }
                    None => {
                        tracing::debug!("command channel closed, disconnecting");
                        self.commands_closed = true;
                        // `close` is defined for every protocol state, so it
                        // stays correct when the application already sent an
                        // explicit `Command::Disconnect` before dropping the
                        // sender.
                        self.client.close()?;
                    }}
            },
            () = maybe_wait(deadline) => {
                tracing::trace!(?deadline, "protocol deadline elapsed");
                self.client.handle_timeout(Instant::now())?
            },
        };

        // pump all events until the protocol is idle
        let mut flow;
        loop {
            flow = Flow::Idle;

            if let Some(bytes) = self.client.poll_write() {
                tracing::trace!(len = bytes.len(), "protocol produced bytes to send");
                flow = self.send_bytes(bytes).await?;
                if flow == Flow::Quit {
                    break;
                }
            }
            if let Some(event) = self.client.poll_read() {
                tracing::debug!(?event, "protocol produced an event for the application");
                flow = self.submit_event(event).await?;
                if flow == Flow::Quit {
                    break;
                }
            }
            if let Some(action) = self.client.poll_event() {
                tracing::debug!(?action, "protocol requested a driver action");
                flow = self.perform_action(action).await?;
                if flow == Flow::Quit {
                    break;
                }
            }

            if flow == Flow::Idle {
                break;
            }
        }

        tracing::trace!(?flow, "protocol is idle");

        if self.commands_closed && self.maybe_socket.is_none() {
            tracing::debug!("command channel closed and socket gone, quitting");
            return Ok(Flow::Quit);
        }

        Ok(flow)
    }

    #[tracing::instrument(level = "trace", skip_all, fields(len = bytes.len()), err)]
    async fn send_bytes(&mut self, bytes: Bytes) -> Result<Flow, Error> {
        let socket = self.maybe_socket.as_mut().ok_or(Error::ProtocolError)?;

        if let Err(error) = socket.write_all(&bytes).await {
            tracing::warn!(%error, "socket write failed, reporting socket error to the protocol");
            self.client.handle_event(DriverEvent::SocketError)?;
        } else {
            tracing::trace!("wrote bytes to socket");
        }

        if let Err(error) = socket.flush().await {
            tracing::warn!(%error, "socket flush failed, reporting socket error to the protocol");
            self.client.handle_event(DriverEvent::SocketError)?;
        } else {
            tracing::trace!("flushed socket");
        }

        Ok(Flow::Continue)
    }

    #[tracing::instrument(level = "trace", skip(self), err)]
    async fn submit_event(&mut self, event: Event) -> Result<Flow, Error> {
        if let Err(error) = self.events.send(event).await {
            tracing::warn!(%error, "event channel closed, quitting");
            Ok(Flow::Quit)
        } else {
            Ok(Flow::Continue)
        }
    }

    #[tracing::instrument(level = "debug", skip(self), err)]
    async fn perform_action(&mut self, action: DriverAction) -> Result<Flow, Error> {
        match action {
            DriverAction::CloseSocket => {
                let mut socket = self.maybe_socket.take().ok_or(Error::ProtocolError)?;

                if let Err(error) = socket.flush().await {
                    tracing::warn!(%error, "failed to flush socket before closing it");
                    self.client.handle_event(DriverEvent::SocketError)?;
                }
                if let Err(error) = socket.shutdown().await {
                    tracing::warn!(%error, "failed to shut down socket before closing it");
                    self.client.handle_event(DriverEvent::SocketError)?;
                }
                drop(socket);
                tracing::debug!("socket closed");
                self.client.handle_event(DriverEvent::SocketClosed)?;
            }
            DriverAction::OpenSocket => match (self.connect)().await {
                Ok(socket) => {
                    tracing::debug!("socket connected");
                    self.maybe_socket = Some(socket);
                    self.client.handle_event(DriverEvent::SocketConnected)?;
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to open socket, reporting socket error to the protocol");
                    self.client.handle_event(DriverEvent::SocketError)?;
                }
            },
            DriverAction::Quit => {
                tracing::debug!("protocol requested shutdown");
                return Ok(Flow::Quit);
            }
            _ => tracing::warn!(?action, "ignoring unsupported driver action"),
        }
        Ok(Flow::Continue)
    }
}

async fn maybe_read<S>(socket: &mut Option<S>) -> io::Result<&[u8]>
where
    S: AsyncBufRead + Unpin,
{
    let Some(socket) = socket else {
        return pending().await;
    };

    let fill = socket.fill_buf().await?;

    Ok(fill)
}

async fn maybe_wait(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => pending().await,
    }
}
