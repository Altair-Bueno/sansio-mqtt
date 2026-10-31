use sansio_mqtt_protocol::Command;
use sansio_mqtt_protocol::CommandDiscriminants;
use sansio_mqtt_protocol::DriverAction;
use sansio_mqtt_protocol::DriverActionDiscriminants;
use sansio_mqtt_protocol::DriverEvent;
use sansio_mqtt_protocol::Error;
use sansio_mqtt_protocol::Event;
use sansio_mqtt_protocol::EventDiscriminants;
use sansio_mqtt_protocol::IncomingData;
use sansio_mqtt_protocol::MqttProtocol;
use std::future::pending;
use std::io;
use std::ops::ControlFlow;
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

/// What happened to the socket while the driver was waiting on it.
#[derive(Debug)]
enum ReadOutcome {
    /// The peer closed the connection (zero-length read).
    Closed,
    /// Bytes were handed to the client and consumed from the socket.
    Read(Result<(), Error>),
}

impl<Protocol, Socket, MakeSocket, MakeSocketFuture> Driver<Protocol, Socket, MakeSocket>
where
    Protocol: for<'bytes> MqttProtocol<'bytes, Instant>,
    Socket: AsyncBufRead + AsyncWrite + Unpin,
    MakeSocket: FnMut() -> MakeSocketFuture,
    MakeSocketFuture: Future<Output = io::Result<Socket>>,
{
    /// Runs the driver until the protocol requests [`DriverAction::Quit`], or
    /// until the command channel is closed and the socket has been released.
    ///
    /// Errors returned by the protocol are logged and do not stop the driver:
    /// the protocol queues its own recovery (e.g. DISCONNECT and
    /// [`DriverAction::CloseSocket`]) which the driver keeps performing.
    #[tracing::instrument(level = "trace", skip_all)]
    pub async fn run(&mut self) {
        while self.process().await.is_continue() {}
    }

    #[tracing::instrument(level = "trace", skip_all, ret)]
    async fn process(&mut self) -> ControlFlow<(), ()> {
        let maybe_deadline = self.client.poll_timeout();

        tokio::select! {
            () = maybe_wait(maybe_deadline) => self.handle_timeout(),
            read_result = read_into(&mut self.client, self.maybe_socket.as_mut()) => self.handle_read_result(read_result),
            maybe_command = self.commands.recv(), if !self.commands_closed => self.handle_maybe_command(maybe_command),
        }

        self.pump().await?;

        if self.commands_closed && self.maybe_socket.is_none() {
            tracing::debug!("command channel closed and socket released, stopping driver");
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn handle_timeout(&mut self) {
        log_client_error(self.client.handle_timeout(Instant::now()), "timeout");
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn handle_read_result(&mut self, result: io::Result<ReadOutcome>) {
        match result {
            Err(error) => {
                tracing::warn!(%error, "failed to read bytes from the connection");
                self.report_socket_error();
            }
            Ok(ReadOutcome::Closed) => {
                tracing::debug!("connection was closed by the peer");
                // The protocol does not answer `SocketClosed` with
                // `CloseSocket`, so release the socket here; otherwise every
                // later read would return EOF again.
                self.maybe_socket = None;
                log_client_error(
                    self.client.handle_event(DriverEvent::SocketClosed),
                    "socket closed",
                );
            }
            Ok(ReadOutcome::Read(result)) => log_client_error(result, "incoming bytes"),
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn handle_maybe_command(&mut self, maybe_command: Option<Command>) {
        match maybe_command {
            Some(command) => {
                let kind = CommandDiscriminants::from(&command);
                tracing::debug!(command = %kind, "received command from the application");
                if let Err(error) = self.client.handle_write(command) {
                    tracing::warn!(%error, command = %kind, "client rejected a command");
                }
            }
            None => {
                tracing::debug!("command channel closed, closing the client");
                self.commands_closed = true;
                // `close` is defined for every protocol state, so it stays
                // correct when the application already sent an explicit
                // `Command::Disconnect` before dropping the sender.
                log_client_error(self.client.close(), "close");
            }
        }
    }

    fn report_socket_error(&mut self) {
        log_client_error(
            self.client.handle_event(DriverEvent::SocketError),
            "socket error",
        );
    }

    #[tracing::instrument(level = "trace", skip_all, fields(bytes_len = bytes.len()))]
    async fn send_bytes(&mut self, bytes: &[u8]) {
        let Some(socket) = self.maybe_socket.as_mut() else {
            tracing::error!("client produced bytes while no socket is open, dropping them");
            return;
        };

        let result = match socket.write_all(bytes).await {
            Ok(()) => socket.flush().await,
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            tracing::warn!(%error, "failed to send bytes");
            self.report_socket_error();
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn submit_event(&mut self, event: Event) {
        let kind = EventDiscriminants::from(&event);
        if let Err(error) = self.events.send(event).await {
            tracing::warn!(%error, event = %kind, "failed to relay incoming event");
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn perform_open_socket(&mut self) {
        if self.maybe_socket.is_some() {
            tracing::error!("client requested a socket while one is already open, ignoring");
            return;
        }

        match (self.connect)().await {
            Ok(socket) => {
                tracing::debug!("socket connected");
                self.maybe_socket = Some(socket);
                log_client_error(
                    self.client.handle_event(DriverEvent::SocketConnected),
                    "socket connected",
                );
            }
            Err(error) => {
                tracing::warn!(%error, "failed to open socket");
                self.report_socket_error();
            }
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn perform_close_socket(&mut self) {
        let Some(mut socket) = self.maybe_socket.take() else {
            tracing::debug!("client requested to close an already closed socket, ignoring");
            return;
        };

        if let Err(error) = socket.shutdown().await {
            tracing::warn!(%error, "failed to shut down socket cleanly");
        }
        drop(socket);
        tracing::debug!("socket closed");

        log_client_error(
            self.client.handle_event(DriverEvent::SocketClosed),
            "socket closed",
        );
    }

    #[tracing::instrument(level = "trace", skip_all, ret)]
    async fn perform_action(&mut self, action: DriverAction) -> ControlFlow<(), ()> {
        match action {
            DriverAction::OpenSocket => self.perform_open_socket().await,
            DriverAction::CloseSocket => self.perform_close_socket().await,
            DriverAction::Quit => {
                tracing::debug!("client requested shutdown");
                return ControlFlow::Break(());
            }
            action => {
                tracing::warn!(
                    action = %DriverActionDiscriminants::from(action),
                    "received an unsupported action to perform, ignoring"
                );
            }
        }
        ControlFlow::Continue(())
    }

    /// Drains everything the client has queued until it is idle.
    #[tracing::instrument(level = "trace", skip_all, ret)]
    async fn pump(&mut self) -> ControlFlow<(), ()> {
        loop {
            if let Some(bytes) = self.client.poll_write() {
                self.send_bytes(&bytes).await;
            } else if let Some(event) = self.client.poll_read() {
                self.submit_event(event).await;
            } else if let Some(action) = self.client.poll_event() {
                self.perform_action(action).await?;
            } else {
                return ControlFlow::Continue(());
            }
        }
    }
}

fn log_client_error(result: Result<(), Error>, operation: &'static str) {
    if let Err(error) = result {
        tracing::warn!(%error, operation, "client reported an error");
    }
}

/// Waits for bytes on the socket (forever if there is none), hands them to the
/// client and consumes them.
///
/// Cancel safe: the only await point is `fill_buf`, which is itself cancel
/// safe; feeding and consuming happen synchronously afterwards.
async fn read_into<Protocol, Socket>(
    client: &mut Protocol,
    maybe_socket: Option<&mut Socket>,
) -> io::Result<ReadOutcome>
where
    Protocol: for<'bytes> MqttProtocol<'bytes, Instant>,
    Socket: AsyncBufRead + Unpin,
{
    let Some(socket) = maybe_socket else {
        return pending().await;
    };

    let bytes = socket.fill_buf().await?;
    if bytes.is_empty() {
        return Ok(ReadOutcome::Closed);
    }

    let received_at = Instant::now();
    let consumed = bytes.len();
    tracing::trace!(bytes_len = consumed, ?received_at, "read bytes from socket");
    // The client buffers any partial packet itself, so the whole read is
    // always consumed.
    let result = client.handle_read(IncomingData { bytes, received_at });
    socket.consume(consumed);

    Ok(ReadOutcome::Read(result))
}

async fn maybe_wait(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => pending().await,
    }
}
