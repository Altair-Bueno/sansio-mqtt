use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::time::Instant;

use sansio::Protocol;
use sansio_mqtt_v5_contract::{
    ConnectOptions, ProtocolError, PublishRequest, SessionAction, SubscribeRequest,
};
use sansio_mqtt_v5_protocol::{MqttProtocol, ProtocolEvent};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::timer::TimerMap;
use crate::transport;

const CHANNEL_CAPACITY: usize = 32;

#[derive(Clone)]
pub struct TokioClient {
    request_tx: mpsc::Sender<ProtocolEvent>,
}

impl TokioClient {
    pub async fn connect(
        addr: SocketAddr,
        options: ConnectOptions,
    ) -> io::Result<(Self, mpsc::Receiver<SessionAction>)> {
        let stream = TcpStream::connect(addr).await?;
        let transport = transport::spawn(stream, CHANNEL_CAPACITY);

        let (request_tx, request_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (session_tx, session_rx) = mpsc::channel(CHANNEL_CAPACITY);

        tokio::spawn(async move {
            let _ = run_driver(transport, request_rx, session_tx, options).await;
        });

        Ok((Self { request_tx }, session_rx))
    }

    pub async fn publish(&self, request: PublishRequest) -> io::Result<()> {
        self.request_tx
            .send(ProtocolEvent::Publish(request))
            .await
            .map_err(channel_closed)
    }

    pub async fn subscribe(&self, request: SubscribeRequest) -> io::Result<()> {
        self.request_tx
            .send(ProtocolEvent::Subscribe(request))
            .await
            .map_err(channel_closed)
    }

    pub async fn disconnect(&self) -> io::Result<()> {
        self.request_tx
            .send(ProtocolEvent::Disconnect)
            .await
            .map_err(channel_closed)
    }
}

async fn run_driver(
    mut transport: transport::Transport,
    mut request_rx: mpsc::Receiver<ProtocolEvent>,
    session_tx: mpsc::Sender<SessionAction>,
    connect_options: ConnectOptions,
) -> io::Result<()> {
    let mut protocol = MqttProtocol::new();
    let mut timers = TimerMap::<u32>::new();
    let start = Instant::now();
    let mut scheduled_deadline: Option<u32> = None;
    let mut connected = false;
    let mut pending_requests = VecDeque::<ProtocolEvent>::new();

    Protocol::handle_event(&mut protocol, ProtocolEvent::Connect(connect_options))
        .map_err(protocol_error)?;
    drain_protocol(&mut protocol, &transport.write_tx, &session_tx).await?;
    reschedule_protocol_timeout(&mut protocol, &mut timers, &mut scheduled_deadline, start);

    loop {
        tokio::select! {
            maybe_frame = transport.read_rx.recv() => {
                let Some(frame) = maybe_frame else {
                    return Ok(());
                };

                let message = heapless::Vec::<u8, 256>::from_slice(&frame)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "mqtt frame too large"))?;
                let received_connack = is_connack_packet(&frame);
                Protocol::handle_read(&mut protocol, message).map_err(protocol_error)?;

                if received_connack {
                    connected = true;

                    while let Some(request) = pending_requests.pop_front() {
                        Protocol::handle_event(&mut protocol, request).map_err(protocol_error)?;
                    }
                }
            }
            maybe_request = request_rx.recv() => {
                let Some(request) = maybe_request else {
                    return Ok(());
                };

                if connected || !should_defer_request(&request) {
                    Protocol::handle_event(&mut protocol, request).map_err(protocol_error)?;
                } else {
                    pending_requests.push_back(request);
                }
            }
            expired_deadline = timers.next_expired(), if scheduled_deadline.is_some() => {
                if Some(expired_deadline) == scheduled_deadline {
                    scheduled_deadline = None;
                }

                Protocol::handle_timeout(&mut protocol, elapsed_ms(start)).map_err(protocol_error)?;
            }
        }

        drain_protocol(&mut protocol, &transport.write_tx, &session_tx).await?;
        reschedule_protocol_timeout(&mut protocol, &mut timers, &mut scheduled_deadline, start);
    }
}

async fn drain_protocol(
    protocol: &mut MqttProtocol,
    write_tx: &mpsc::Sender<Vec<u8>>,
    session_tx: &mpsc::Sender<SessionAction>,
) -> io::Result<()> {
    while let Some(frame) = Protocol::poll_write(protocol) {
        write_tx
            .send(frame.as_slice().to_vec())
            .await
            .map_err(channel_closed)?;
    }

    while let Some(action) = Protocol::poll_event(protocol) {
        session_tx.send(action).await.map_err(channel_closed)?;
    }

    Ok(())
}

fn reschedule_protocol_timeout(
    protocol: &mut MqttProtocol,
    timers: &mut TimerMap<u32>,
    scheduled_deadline: &mut Option<u32>,
    start: Instant,
) {
    let next_deadline = Protocol::poll_timeout(protocol);

    if *scheduled_deadline == next_deadline {
        return;
    }

    if let Some(previous) = scheduled_deadline.take() {
        let _ = timers.cancel(&previous);
    }

    if let Some(deadline_ms) = next_deadline {
        let now_ms = elapsed_ms(start);
        let delay_ms = deadline_ms.saturating_sub(now_ms);
        timers.schedule(deadline_ms, delay_ms);
        *scheduled_deadline = Some(deadline_ms);
    }
}

fn elapsed_ms(start: Instant) -> u32 {
    let elapsed = start.elapsed().as_millis();
    u32::try_from(elapsed).unwrap_or(u32::MAX)
}

fn protocol_error(error: ProtocolError) -> io::Error {
    io::Error::other(format!("protocol error: {error:?}"))
}

fn channel_closed<T>(_: T) -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "channel closed")
}

fn is_connack_packet(frame: &[u8]) -> bool {
    frame.first().is_some_and(|first| (first >> 4) == 2)
}

fn should_defer_request(request: &ProtocolEvent) -> bool {
    matches!(
        request,
        ProtocolEvent::Publish(_) | ProtocolEvent::Subscribe(_)
    )
}
