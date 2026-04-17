use core::time::Duration;

use sansio::Protocol;
use sansio_mqtt_v5_protocol::{
    ClientMessage, ConnectionOptions, DriverEventIn, SubscribeOptions, UserWriteIn, UserWriteOut,
};
use sansio_mqtt_v5_tokio::{Client, EventLoop, Poll};
use sansio_mqtt_v5_types::{Payload, Qos, Topic, Utf8String};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::Instant;

type AnyError = Box<dyn std::error::Error + Send + Sync>;

const RECONNECT_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeConfig {
    broker: String,
    topic: String,
}

#[derive(Debug)]
struct DriverState {
    connected: bool,
    subscribed: bool,
    shutting_down: bool,
    reconnect_at: Option<Instant>,
}

impl Default for DriverState {
    fn default() -> Self {
        Self {
            connected: false,
            subscribed: false,
            shutting_down: false,
            reconnect_at: Some(Instant::now()),
        }
    }
}

impl DriverState {
    fn should_read_stdin(&self) -> bool {
        self.connected && !self.shutting_down
    }

    fn schedule_reconnect(&mut self, now: Instant) {
        if !self.shutting_down {
            self.reconnect_at = Some(now + RECONNECT_DELAY);
        }
    }

    fn on_disconnected(&mut self, now: Instant) {
        self.connected = false;
        self.subscribed = false;
        self.schedule_reconnect(now);
    }

    fn on_connected(&mut self) {
        self.connected = true;
    }

    fn should_reconnect(&self, now: Instant) -> bool {
        self.reconnect_at.is_some_and(|deadline| deadline <= now)
    }

    fn mark_reconnect_attempted(&mut self) {
        self.reconnect_at = None;
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), AnyError> {
    let config = load_runtime_config()?;

    let mut client = Client::default();
    let mut driver_state = DriverState::default();
    let mut stream: Option<TcpStream> = None;
    let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    let mut read_buffer = [0_u8; 4096];

    loop {
        if driver_state.should_reconnect(Instant::now()) {
            enqueue_connect(&mut client)?;
            driver_state.mark_reconnect_attempted();
        }

        while let Some(item) = EventLoop::new(&mut client).poll() {
            match item {
                Poll::Read(output) => {
                    handle_protocol_output(
                        output,
                        &mut client,
                        &config.topic,
                        &mut driver_state,
                        &mut stdout,
                    )
                    .await?;
                }
                Poll::Write(frame) => {
                    if let Some(socket) = stream.as_mut() {
                        socket.write_all(&frame).await?;
                    }
                }
                Poll::Event(event) => {
                    if handle_driver_event(
                        event,
                        &mut client,
                        &config.broker,
                        &mut stream,
                        &mut driver_state,
                    )
                    .await?
                    {
                        return Ok(());
                    }
                }
            }
        }

        tokio::select! {
            read_result = async {
                match stream.as_mut() {
                    Some(socket) => socket.read(&mut read_buffer).await,
                    None => unreachable!("socket read branch guarded by stream.is_some()"),
                }
            }, if stream.is_some() => {
                let bytes_read = read_result?;
                if bytes_read == 0 {
                    client
                        .handle_event(DriverEventIn::SocketClosed)
                        .map_err(|err| std::io::Error::other(format!("socket closed event failed: {err}")))?;
                    driver_state.on_disconnected(Instant::now());
                    if let Some(mut socket) = stream.take() {
                        let _ = socket.shutdown().await;
                    }
                } else {
                    client
                        .handle_read(read_buffer[..bytes_read].to_vec().into())
                        .map_err(|err| std::io::Error::other(format!("failed to parse broker bytes: {err}")))?;
                }
            }
            line = stdin_lines.next_line(), if driver_state.should_read_stdin() => {
                match line? {
                    Some(content) => {
                        let publish = build_publish_message(&config.topic, &content)?;
                        client
                            .handle_write(UserWriteIn::PublishMessage(publish))
                            .map_err(|err| std::io::Error::other(format!("publish command rejected: {err}")))?;
                    }
                    None => {
                        driver_state.shutting_down = true;
                        driver_state.reconnect_at = None;
                        request_disconnect(&mut client);
                    }
                }
            }
            signal_result = tokio::signal::ctrl_c(), if !driver_state.shutting_down => {
                signal_result?;
                driver_state.shutting_down = true;
                driver_state.reconnect_at = None;
                request_disconnect(&mut client);
            }
            _ = async {
                if let Some(deadline) = driver_state.reconnect_at {
                    tokio::time::sleep_until(deadline).await;
                } else {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            } => {}
        }

        if driver_state.shutting_down && stream.is_none() && !driver_state.connected {
            return Ok(());
        }
    }
}

async fn handle_protocol_output(
    output: UserWriteOut,
    client: &mut Client,
    topic: &str,
    driver_state: &mut DriverState,
    stdout: &mut tokio::io::Stdout,
) -> Result<(), AnyError> {
    match output {
        UserWriteOut::Connected => {
            driver_state.on_connected();
            if !driver_state.subscribed {
                let subscribe = build_subscribe(topic)?;
                client
                    .handle_write(UserWriteIn::Subscribe(subscribe))
                    .map_err(|err| {
                        std::io::Error::other(format!("subscribe command rejected: {err}"))
                    })?;
                driver_state.subscribed = true;
            }
        }
        UserWriteOut::Disconnected => {
            driver_state.on_disconnected(Instant::now());
        }
        UserWriteOut::ReceivedMessage(message) => {
            stdout.write_all(message.payload.as_ref().as_ref()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
        UserWriteOut::PublishAcknowledged { .. }
        | UserWriteOut::PublishCompleted { .. }
        | UserWriteOut::PublishDropped { .. } => {}
    }

    Ok(())
}

async fn handle_driver_event(
    event: sansio_mqtt_v5_protocol::DriverEventOut,
    client: &mut Client,
    broker: &str,
    stream: &mut Option<TcpStream>,
    driver_state: &mut DriverState,
) -> Result<bool, AnyError> {
    match event {
        sansio_mqtt_v5_protocol::DriverEventOut::OpenSocket => {
            match TcpStream::connect(broker).await {
                Ok(socket) => {
                    *stream = Some(socket);
                    client
                        .handle_event(DriverEventIn::SocketConnected)
                        .map_err(|err| {
                            std::io::Error::other(format!("socket connected event failed: {err}"))
                        })?;
                }
                Err(connect_error) => {
                    eprintln!("failed to connect to {broker}: {connect_error}");
                    let _ = client.handle_event(DriverEventIn::SocketError);
                    driver_state.on_disconnected(Instant::now());
                    *stream = None;
                }
            }
        }
        sansio_mqtt_v5_protocol::DriverEventOut::CloseSocket => {
            if let Some(mut socket) = stream.take() {
                let _ = socket.shutdown().await;
            }
        }
        sansio_mqtt_v5_protocol::DriverEventOut::Quit => {
            return Ok(true);
        }
    }

    Ok(false)
}

fn load_runtime_config() -> Result<RuntimeConfig, AnyError> {
    load_runtime_config_from(|name| std::env::var(name).ok())
}

fn load_runtime_config_from(
    get_env: impl Fn(&str) -> Option<String>,
) -> Result<RuntimeConfig, AnyError> {
    let broker = get_env("BROKER")
        .ok_or_else(|| std::io::Error::other("BROKER env variable must be set"))?;
    let topic =
        get_env("TOPIC").ok_or_else(|| std::io::Error::other("TOPIC env variable must be set"))?;

    Ok(RuntimeConfig { broker, topic })
}

fn enqueue_connect(client: &mut Client) -> Result<(), AnyError> {
    client
        .handle_write(UserWriteIn::Connect(ConnectionOptions::default()))
        .map_err(|err| std::io::Error::other(format!("connect command rejected: {err}")))?;
    Ok(())
}

fn request_disconnect(client: &mut Client) {
    let _ = client.handle_write(UserWriteIn::Disconnect);
}

fn build_topic(topic: &str) -> Result<Topic, AnyError> {
    let utf8 = Utf8String::try_from(topic.to_owned())
        .map_err(|err| std::io::Error::other(format!("invalid topic utf-8 string: {err}")))?;

    Topic::try_from(utf8)
        .map_err(|err| std::io::Error::other(format!("invalid topic name: {err}")))
        .map_err(Into::into)
}

fn build_subscribe(topic: &str) -> Result<SubscribeOptions, AnyError> {
    let filter = Utf8String::try_from(topic.to_owned())
        .map_err(|err| std::io::Error::other(format!("invalid topic filter: {err}")))?;
    let subscriptions = vec![filter]
        .try_into()
        .map_err(|_| std::io::Error::other("subscriptions must not be empty"))?;

    Ok(SubscribeOptions {
        subscriptions,
        qos: Qos::AtMostOnce,
        no_local: false,
        retain_as_published: false,
        retain_handling: 0,
        subscription_identifier: None,
        user_properties: Vec::new(),
    })
}

fn build_publish_message(topic: &str, line: &str) -> Result<ClientMessage, AnyError> {
    Ok(ClientMessage {
        topic: build_topic(topic)?,
        qos: Qos::AtMostOnce,
        payload: Payload::from(line.as_bytes().to_vec()),
        ..ClientMessage::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_requires_broker_variable() {
        let result = load_runtime_config_from(|name| match name {
            "TOPIC" => Some("topic/demo".to_owned()),
            _ => None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn config_requires_topic_variable() {
        let result = load_runtime_config_from(|name| match name {
            "BROKER" => Some("localhost:1883".to_owned()),
            _ => None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn stdin_gating_depends_on_connection_state() {
        let mut state = DriverState::default();
        state.connected = false;
        state.shutting_down = false;
        assert!(!state.should_read_stdin());

        state.connected = true;
        assert!(state.should_read_stdin());

        state.shutting_down = true;
        assert!(!state.should_read_stdin());
    }

    #[test]
    fn disconnect_schedules_reconnect_when_not_shutting_down() {
        let mut state = DriverState::default();
        let now = Instant::now();

        state.shutting_down = false;
        state.on_disconnected(now);

        assert!(state.reconnect_at.is_some());
        assert!(state.should_reconnect(now + RECONNECT_DELAY));
    }
}
