use core::time::Duration;
use std::net::{SocketAddr, ToSocketAddrs};

use sansio_mqtt_v5_protocol::{ClientMessage, SubscribeOptions};
use sansio_mqtt_v5_tokio::{connect, ConnectOptions, Event};
use sansio_mqtt_v5_types::{Payload, Qos, Topic, Utf8String};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

type AnyError = Box<dyn std::error::Error + Send + Sync>;

const RECONNECT_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeConfig {
    broker: String,
    topic: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), AnyError> {
    let config = load_runtime_config()?;
    let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    loop {
        let broker_addr = resolve_broker_addr(&config.broker)?;
        let connect_options = ConnectOptions {
            addr: broker_addr,
            ..ConnectOptions::default()
        };

        let (client, mut event_loop) = match connect(connect_options).await {
            Ok(handles) => handles,
            Err(err) => {
                eprintln!("failed to connect to {}: {err}", config.broker);
                wait_or_exit().await?;
                continue;
            }
        };

        let mut connected = false;

        loop {
            tokio::select! {
                polled = event_loop.poll() => {
                    match polled {
                        Ok(Event::Connected) => {
                            connected = true;
                            let subscribe = build_subscribe(&config.topic)?;
                            if let Err(err) = client.subscribe(subscribe).await {
                                eprintln!("subscribe command failed: {err}");
                                break;
                            }
                        }
                        Ok(Event::Disconnected) => {
                            break;
                        }
                        Ok(Event::Message(message)) => {
                            stdout.write_all(message.payload.as_ref().as_ref()).await?;
                            stdout.write_all(b"\n").await?;
                            stdout.flush().await?;
                        }
                        Ok(Event::PublishAcknowledged { .. })
                        | Ok(Event::PublishCompleted { .. })
                        | Ok(Event::PublishDropped { .. }) => {}
                        Err(err) => {
                            eprintln!("event loop failed: {err}");
                            break;
                        }
                    }
                }
                line = stdin_lines.next_line(), if should_read_stdin(connected) => {
                    match line? {
                        Some(content) => {
                            let message = build_publish_message(&config.topic, &content)?;
                            if let Err(err) = client.publish(message).await {
                                eprintln!("publish command failed: {err}");
                                break;
                            }
                        }
                        None => {
                            let _ = client.disconnect().await;
                            return Ok(());
                        }
                    }
                }
                signal_result = tokio::signal::ctrl_c() => {
                    signal_result?;
                    let _ = client.disconnect().await;
                    return Ok(());
                }
            }
        }

        wait_or_exit().await?;
    }
}

async fn wait_or_exit() -> Result<(), AnyError> {
    tokio::select! {
        _ = tokio::time::sleep(RECONNECT_DELAY) => Ok(()),
        signal_result = tokio::signal::ctrl_c() => {
            signal_result?;
            Ok(())
        }
    }
}

fn should_read_stdin(connected: bool) -> bool {
    connected
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

fn resolve_broker_addr(broker: &str) -> Result<SocketAddr, AnyError> {
    broker
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other("BROKER did not resolve to any address").into())
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
    fn stdin_is_only_read_while_connected() {
        assert!(!should_read_stdin(false));
        assert!(should_read_stdin(true));
    }

    #[test]
    fn resolves_dns_or_ip_with_port() {
        let addr = resolve_broker_addr("localhost:1883").expect("localhost resolves");
        assert_eq!(addr.port(), 1883);
    }
}
