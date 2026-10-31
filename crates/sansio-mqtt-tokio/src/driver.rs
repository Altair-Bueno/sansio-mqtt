use bytes::Bytes;
use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::CommandDiscriminants;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::MqttProtocol;
use std::future::pending;
use std::io;
use std::pin::Pin;
use tokio::io::AsyncBufRead;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::time::Instant;
use tokio::time::sleep_until;

/// Runs an MQTT protocol state machine over a tokio socket.
///
/// The application owns the loop: it hands commands over with
/// [`submit`](Self::submit) and drives the connection by awaiting
/// [`next`](Self::next), typically both from the same `tokio::select!`.
///
/// Socket I/O, keep-alive and every other timer only make progress while
/// [`next`](Self::next) is being awaited, so the application must keep
/// polling it for the connection to stay alive.
pub struct Driver<Protocol, Socket, MakeSocket, MakeSocketFuture> {
    client: Protocol,
    connect: MakeSocket,
    connection: Connection<Socket, MakeSocketFuture>,
    /// Bytes handed out by the client that are not yet written to the socket.
    outgoing: Bytes,
    needs_flush: bool,
}

/// The socket as seen by the driver. Every in-flight operation lives here so
/// that cancelling [`Driver::next`] never loses progress.
enum Connection<Socket, MakeSocketFuture> {
    Closed,
    Opening(Pin<Box<MakeSocketFuture>>),
    Open(Socket),
    Closing(Socket),
}

/// What woke the driver up while it was idle.
enum Input<Socket> {
    Timeout,
    Read(io::Result<ReadOutcome>),
    Connected(io::Result<Socket>),
}

/// What happened to the socket while the driver was waiting on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadOutcome {
    /// The peer closed the connection (zero-length read).
    Closed,
    /// Bytes were handed to the client and consumed from the socket.
    Read,
}

impl<Protocol, Socket, MakeSocket, MakeSocketFuture>
    Driver<Protocol, Socket, MakeSocket, MakeSocketFuture>
where
    Protocol: for<'bytes> MqttProtocol<'bytes, Instant>,
    Socket: AsyncBufRead + AsyncWrite + Unpin,
    MakeSocket: FnMut() -> MakeSocketFuture,
    MakeSocketFuture: Future<Output = io::Result<Socket>>,
{
    /// Creates a driver for `client`. `connect` is called every time the
    /// client asks for a new connection.
    pub fn new(client: Protocol, connect: MakeSocket) -> Self {
        Self {
            client,
            connect,
            connection: Connection::Closed,
            outgoing: Bytes::new(),
            needs_flush: false,
        }
    }

    /// Hands `command` to the client.
    ///
    /// This performs no I/O: an accepted command only takes effect on the
    /// wire once [`next`](Self::next) is awaited, and its outcome (e.g.
    /// [`Event::PublishAcknowledged`]) is reported as an [`Event`].
    ///
    /// # Errors
    ///
    /// Returns the client's [`Error`] when it rejects the command, in which
    /// case the client state is unchanged.
    pub fn submit(&mut self, command: Command) -> Result<(), Error> {
        let kind = CommandDiscriminants::from(&command);
        tracing::debug!(command = %kind, "submitting command");
        self.client.handle_write(command)
    }

    /// Drives socket I/O and timers until the client produces the next
    /// [`Event`].
    ///
    /// The driver never stops on its own: with no connection and no pending
    /// timer this waits until a submitted command (e.g. [`Command::Connect`])
    /// gives it something to do, so await it alongside the command source.
    ///
    /// # Cancel safety
    ///
    /// This method is cancel safe: every in-flight write, connection attempt
    /// and shutdown is kept in the driver and resumed by the next call.
    pub async fn next(&mut self) -> Event {
        loop {
            self.finish_closing().await;
            self.write_outgoing().await;

            if let Some(action) = self.client.poll_event() {
                self.perform_action(action);
                continue;
            }
            if let Some(event) = self.client.poll_read() {
                return event;
            }

            let input = self.wait_for_input().await;
            self.handle_input(input);
        }
    }

    async fn finish_closing(&mut self) {
        let Connection::Closing(socket) = &mut self.connection else {
            return;
        };
        if let Err(error) = socket.shutdown().await {
            tracing::warn!(%error, "failed to shut down socket cleanly");
        }
        self.connection = Connection::Closed;
        tracing::debug!("socket closed");
    }

    /// Writes everything the client has queued, then flushes once.
    async fn write_outgoing(&mut self) {
        loop {
            if self.outgoing.is_empty() {
                match self.client.poll_write() {
                    Some(bytes) => self.outgoing = bytes,
                    None => break,
                }
            }

            let Connection::Open(socket) = &mut self.connection else {
                debug_assert!(false, "client produced bytes while no socket is open");
                tracing::error!("client produced bytes while no socket is open, dropping them");
                self.outgoing.clear();
                continue;
            };

            let result = match socket.write_buf(&mut self.outgoing).await {
                Ok(0) => Err(io::ErrorKind::WriteZero.into()),
                Ok(_) => Ok(()),
                Err(error) => Err(error),
            };
            match result {
                Ok(()) => self.needs_flush = true,
                Err(error) => {
                    tracing::warn!(%error, "failed to write to the socket");
                    self.lose_socket(DriverEvent::SocketError);
                }
            }
        }

        if !self.needs_flush {
            return;
        }
        let Connection::Open(socket) = &mut self.connection else {
            self.needs_flush = false;
            return;
        };
        let result = socket.flush().await;
        self.needs_flush = false;
        if let Err(error) = result {
            tracing::warn!(%error, "failed to flush the socket");
            self.lose_socket(DriverEvent::SocketError);
        }
    }

    fn perform_action(&mut self, action: DriverAction) {
        tracing::debug!(?action, "performing driver action");
        match action {
            DriverAction::OpenSocket => {
                debug_assert!(
                    matches!(self.connection, Connection::Closed),
                    "client requested a socket while one is open"
                );
                self.connection = Connection::Opening(Box::pin((self.connect)()));
            }
            DriverAction::CloseSocket => {
                if let Connection::Open(socket) =
                    std::mem::replace(&mut self.connection, Connection::Closed)
                {
                    self.connection = Connection::Closing(socket);
                }
            }
            action => tracing::warn!(?action, "ignoring unsupported driver action"),
        }
    }

    async fn wait_for_input(&mut self) -> Input<Socket> {
        let deadline = self.client.poll_timeout();
        match &mut self.connection {
            Connection::Open(socket) => tokio::select! {
                () = maybe_wait(deadline) => Input::Timeout,
                outcome = read_into(&mut self.client, socket) => Input::Read(outcome),
            },
            Connection::Opening(connecting) => tokio::select! {
                () = maybe_wait(deadline) => Input::Timeout,
                result = connecting.as_mut() => Input::Connected(result),
            },
            Connection::Closed | Connection::Closing(_) => {
                maybe_wait(deadline).await;
                Input::Timeout
            }
        }
    }

    fn handle_input(&mut self, input: Input<Socket>) {
        match input {
            Input::Timeout => {
                log_client_error(self.client.handle_timeout(Instant::now()), "timeout");
            }
            Input::Read(Ok(ReadOutcome::Read)) => {}
            Input::Read(Ok(ReadOutcome::Closed)) => {
                tracing::debug!("connection was closed by the peer");
                self.lose_socket(DriverEvent::SocketClosed);
            }
            Input::Read(Err(error)) => {
                tracing::warn!(%error, "failed to read from the socket");
                self.lose_socket(DriverEvent::SocketError);
            }
            Input::Connected(Ok(socket)) => {
                tracing::debug!("socket connected");
                match self.client.handle_event(DriverEvent::SocketConnected) {
                    Ok(()) => self.connection = Connection::Open(socket),
                    Err(error) => {
                        // The client did not want this socket, so it will
                        // never ask to close it: shut it down here, and do
                        // not report its closing, which nobody asked for.
                        tracing::warn!(%error, "client rejected the connected socket, closing it");
                        self.connection = Connection::Closing(socket);
                    }
                }
            }
            Input::Connected(Err(error)) => {
                tracing::warn!(%error, "failed to open socket");
                self.lose_socket(DriverEvent::SocketError);
            }
        }
    }

    /// Drops the socket and any unsent bytes, then tells the client why.
    fn lose_socket(&mut self, event: DriverEvent) {
        self.connection = Connection::Closed;
        self.outgoing.clear();
        self.needs_flush = false;
        log_client_error(self.client.handle_event(event), "socket lost");
    }
}

impl<Protocol, Socket, MakeSocket, MakeSocketFuture> std::fmt::Debug
    for Driver<Protocol, Socket, MakeSocket, MakeSocketFuture>
where
    Protocol: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let connection = match self.connection {
            Connection::Closed => "closed",
            Connection::Opening(_) => "opening",
            Connection::Open(_) => "open",
            Connection::Closing(_) => "closing",
        };
        f.debug_struct("Driver")
            .field("client", &self.client)
            .field("connection", &connection)
            .field("outgoing_len", &self.outgoing.len())
            .finish_non_exhaustive()
    }
}

fn log_client_error(result: Result<(), Error>, operation: &'static str) {
    if let Err(error) = result {
        tracing::warn!(%error, operation, "client rejected driver input");
    }
}

/// Waits for bytes on the socket, hands them to the client and consumes them.
///
/// Cancel safe: the only await point is `fill_buf`, which is itself cancel
/// safe; feeding and consuming happen synchronously afterwards.
async fn read_into<Protocol, Socket>(
    client: &mut Protocol,
    socket: &mut Socket,
) -> io::Result<ReadOutcome>
where
    Protocol: for<'bytes> MqttProtocol<'bytes, Instant>,
    Socket: AsyncBufRead + Unpin,
{
    let bytes = socket.fill_buf().await?;
    if bytes.is_empty() {
        return Ok(ReadOutcome::Closed);
    }

    let received_at = Instant::now();
    let consumed = bytes.len();
    tracing::trace!(bytes_len = consumed, "read bytes from socket");
    // The client buffers any partial packet itself, so the whole read is
    // always consumed.
    let result = client.handle_read(IncomingData { bytes, received_at });
    socket.consume(consumed);
    log_client_error(result, "incoming bytes");

    Ok(ReadOutcome::Read)
}

async fn maybe_wait(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => pending().await,
    }
}
