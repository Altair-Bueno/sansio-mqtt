use sansio::Protocol;
use sansio_mqtt_v5_protocol::{
    Client, ClientMessage, ConnectionOptions, DriverEventIn, DriverEventOut, SubscribeOptions,
    UserWriteIn, UserWriteOut, Will,
};
use sansio_mqtt_v5_types::{Payload, Qos, Topic, Utf8String};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

type AnyError = Box<dyn std::error::Error + Send + Sync>;

const BROKER_ADDR: &str = "test.mosquitto.org:1883";
const TOPIC: &str = "sansio-mqtt/v5/tokio/prototype";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), AnyError> {
    println!("connecting to {BROKER_ADDR}");
    println!("subscribing and publishing on topic: {TOPIC}");
    println!("type lines and press Enter to publish; Ctrl+C or EOF to exit");

    let mut client = Client::<tokio::time::Instant>::default();
    client
        .handle_write(UserWriteIn::Connect(ConnectionOptions {
            will: Some(Will {
                topic: Topic::try_from(Utf8String::try_from(TOPIC).expect("valid utf8"))
                    .expect("valid topic"),
                payload: Payload::from(&b"will payload"[..]),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .map_err(|err| std::io::Error::other(format!("connect command rejected: {err}")))?;

    let mut saw_open_socket = false;
    while let Some(event) = client.poll_event() {
        match event {
            DriverEventOut::OpenSocket => saw_open_socket = true,
            other => println!("protocol event before socket open: {other:?}"),
        }
    }
    if !saw_open_socket {
        println!("protocol did not request OpenSocket explicitly; continuing");
    }

    let mut stream = TcpStream::connect(BROKER_ADDR).await?;
    client
        .handle_event(DriverEventIn::SocketConnected)
        .map_err(|err| std::io::Error::other(format!("socket connected event failed: {err}")))?;
    flush_writes(&mut client, &mut stream).await?;

    let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();
    let mut read_buf = [0_u8; 4096];
    let mut connected = false;
    let mut subscribed = false;
    let mut shutting_down = false;

    loop {
        let queue = drain_protocol_outputs(&mut client, &mut connected);
        if queue.close_socket || queue.quit {
            break;
        }

        if connected && !subscribed {
            let subscribe = build_subscribe(TOPIC)?;
            client
                .handle_write(UserWriteIn::Subscribe(subscribe))
                .map_err(|err| {
                    std::io::Error::other(format!("subscribe command rejected: {err}"))
                })?;
            flush_writes(&mut client, &mut stream).await?;
            subscribed = true;
            println!("subscribed to {TOPIC}");
        }

        tokio::select! {
            read_result = stream.read(&mut read_buf) => {
                let n = read_result?;
                if n == 0 {
                    client
                        .handle_event(DriverEventIn::SocketClosed)
                        .map_err(|err| std::io::Error::other(format!("socket closed event failed: {err}")))?;
                    break;
                }

                client
                    .handle_read(read_buf[..n].to_vec().into())
                    .map_err(|err| std::io::Error::other(format!("failed to parse broker bytes: {err}")))?;
                flush_writes(&mut client, &mut stream).await?;
            }
            line_result = stdin_lines.next_line(), if !shutting_down => {
                match line_result? {
                    Some(line) => {
                        if !connected {
                            println!("not connected yet; ignoring stdin line");
                            continue;
                        }

                        let publish = build_publish_message(TOPIC, &line)?;
                        match client.handle_write(UserWriteIn::PublishMessage(publish)) {
                            Ok(()) => {
                                flush_writes(&mut client, &mut stream).await?;
                            }
                            Err(err) => {
                                println!("publish rejected by protocol: {err}");
                            }
                        }
                    }
                    None => {
                        println!("stdin EOF received; disconnecting");
                        shutting_down = true;
                        request_disconnect(&mut client);
                        flush_writes(&mut client, &mut stream).await?;
                    }
                }
            }
            signal_result = tokio::signal::ctrl_c(), if !shutting_down => {
                signal_result?;
                println!("Ctrl+C received; disconnecting");
                shutting_down = true;
                request_disconnect(&mut client);
                flush_writes(&mut client, &mut stream).await?;
            }
        }
    }

    let _ = stream.shutdown().await;
    println!("example finished");
    Ok(())
}

fn request_disconnect(client: &mut Client<tokio::time::Instant>) {
    if let Err(err) = client.handle_write(UserWriteIn::Disconnect) {
        println!("disconnect rejected by protocol: {err}");
    }
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
        no_local: true,
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

struct QueueOutcome {
    close_socket: bool,
    quit: bool,
}

fn drain_protocol_outputs(
    client: &mut Client<tokio::time::Instant>,
    connected: &mut bool,
) -> QueueOutcome {
    while let Some(output) = client.poll_read() {
        match output {
            UserWriteOut::Connected => {
                *connected = true;
                println!("connected");
            }
            UserWriteOut::Disconnected => {
                *connected = false;
                println!("disconnected");
            }
            UserWriteOut::ReceivedMessage(message) => {
                let topic: &str = message.topic.as_ref().as_ref();
                let payload = String::from_utf8_lossy(message.payload.as_ref().as_ref());
                println!("[{topic}] {payload}");
            }
            UserWriteOut::PublishAcknowledged {
                packet_id,
                reason_code,
            } => {
                println!("publish acknowledged packet={packet_id} reason={reason_code:?}");
            }
            UserWriteOut::PublishCompleted {
                packet_id,
                reason_code,
            } => {
                println!("publish completed packet={packet_id} reason={reason_code:?}");
            }
            UserWriteOut::PublishDropped { packet_id, reason } => {
                println!("publish dropped packet={packet_id} reason={reason:?}");
            }
        }
    }

    let mut close_socket = false;
    let mut quit = false;
    while let Some(event) = client.poll_event() {
        match event {
            DriverEventOut::OpenSocket => {
                println!("protocol requested OpenSocket while running");
            }
            DriverEventOut::CloseSocket => {
                close_socket = true;
            }
            DriverEventOut::Quit => {
                quit = true;
            }
        }
    }

    QueueOutcome { close_socket, quit }
}

async fn flush_writes(
    client: &mut Client<tokio::time::Instant>,
    stream: &mut TcpStream,
) -> Result<(), AnyError> {
    while let Some(frame) = client.poll_write() {
        stream.write_all(&frame).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_subscribe_uses_topic_constant() {
        let subscribe = build_subscribe(TOPIC).expect("subscribe options build");
        assert_eq!(subscribe.subscriptions.len(), 1);
        assert_eq!(subscribe.subscriptions[0].as_ref(), TOPIC);
    }

    #[test]
    fn build_publish_uses_qos0_payload_and_topic() {
        let message = build_publish_message(TOPIC, "hello").expect("publish message build");
        assert_eq!(message.qos, Qos::AtMostOnce);
        assert_eq!(message.topic.as_ref().as_ref(), TOPIC);
        assert_eq!(message.payload.as_ref().as_ref(), b"hello");
    }
}
