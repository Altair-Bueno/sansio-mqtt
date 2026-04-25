use sansio_mqtt_v5_protocol::{
    BinaryData, ClientMessage, ConnectionOptions, SubscribeOptions, Subscription,
};
use sansio_mqtt_v5_tokio::ConnectOptions;
use sansio_mqtt_v5_types::{Payload, Qos, RetainHandling, Topic, Utf8String};
use std::io::Write;
use std::process::{Child, Command, Stdio};
use tokio::net::TcpListener;

/// A running Mosquitto process that is killed when dropped.
pub struct MosquittoProcess {
    child: Child,
    // Temp files that must outlive the process.
    _config_file: tempfile::NamedTempFile,
    _passwd_file: Option<tempfile::NamedTempFile>,
}

impl Drop for MosquittoProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Find a free TCP port on 127.0.0.1.
async fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind to free port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

/// Start mosquitto with the given config content and return the process handle.
fn start_mosquitto(
    config_content: &str,
    passwd_file: Option<tempfile::NamedTempFile>,
) -> MosquittoProcess {
    let mut config_file = tempfile::Builder::new()
        .suffix(".conf")
        .tempfile()
        .expect("create temp config file");
    config_file
        .write_all(config_content.as_bytes())
        .expect("write config");
    config_file.flush().expect("flush config");

    let child = Command::new("/opt/homebrew/opt/mosquitto/sbin/mosquitto")
        .arg("-c")
        .arg(config_file.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mosquitto");

    MosquittoProcess {
        child,
        _config_file: config_file,
        _passwd_file: passwd_file,
    }
}

/// Wait until mosquitto is accepting connections on the given port (up to 2s).
async fn wait_for_mosquitto(port: u16) {
    for _ in 0..40 {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .is_ok()
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("mosquitto did not start within 2s on port {port}");
}

/// Starts an anonymous Mosquitto 2 broker, returns the process handle (keep alive for test
/// duration) and the mapped host TCP port.
pub async fn anonymous_broker() -> (MosquittoProcess, u16) {
    let port = free_port().await;
    let config = format!("listener {port}\nallow_anonymous true\n", port = port);
    let process = start_mosquitto(&config, None);
    wait_for_mosquitto(port).await;
    (process, port)
}

/// Starts an authenticated Mosquitto 2 broker (requires testuser/testpassword).
pub async fn authenticated_broker() -> (MosquittoProcess, u16) {
    let port = free_port().await;

    let mut passwd_file = tempfile::Builder::new()
        .suffix(".passwd")
        .tempfile()
        .expect("create temp passwd file");
    passwd_file
        .write_all(include_bytes!("mosquitto/passwd"))
        .expect("write passwd");
    passwd_file.flush().expect("flush passwd");

    let config = format!(
        "listener {port}\nallow_anonymous false\npassword_file {passwd}\n",
        port = port,
        passwd = passwd_file.path().display()
    );

    let process = start_mosquitto(&config, Some(passwd_file));
    wait_for_mosquitto(port).await;
    (process, port)
}

/// Default connect options: clean_start=true, no session persistence.
pub fn connect_options(port: u16, client_id: &str) -> ConnectOptions {
    ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from(client_id).expect("valid client id"),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    }
}

/// Connect options for a persistent session: clean_start=true, session_expiry=300s.
pub fn persistent_connect_options(port: u16, client_id: &str) -> ConnectOptions {
    ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from(client_id).expect("valid client id"),
            session_expiry_interval: Some(300),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    }
}

/// Connect options to resume an existing session: clean_start=false, session_expiry=300s.
pub fn resume_connect_options(port: u16, client_id: &str) -> ConnectOptions {
    ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: false,
            client_identifier: Utf8String::try_from(client_id).expect("valid client id"),
            session_expiry_interval: Some(300),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    }
}

/// Connect options with username/password credentials.
pub fn authenticated_connect_options(
    port: u16,
    client_id: &str,
    user: &str,
    pass: &str,
) -> ConnectOptions {
    ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from(client_id).expect("valid client id"),
            user_name: Some(Utf8String::try_from(user).expect("valid username")),
            password: Some(BinaryData::new(pass.as_bytes().to_vec())),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    }
}

/// Subscribe at QoS 0 (broker delivers at most-once; receiver gets Event::Message).
pub fn sub(topic: &str) -> SubscribeOptions {
    SubscribeOptions {
        subscription: Subscription {
            topic_filter: Utf8String::try_from(topic).expect("valid topic filter"),
            qos: Qos::AtMostOnce,
            no_local: false,
            retain_as_published: false,
            retain_handling: RetainHandling::SendRetained,
        },
        extra_subscriptions: vec![],
        subscription_identifier: None,
        user_properties: vec![],
    }
}

/// Subscribe at QoS 1 (broker queues messages for offline sessions).
pub fn sub_qos1(topic: &str) -> SubscribeOptions {
    SubscribeOptions {
        subscription: Subscription {
            topic_filter: Utf8String::try_from(topic).expect("valid topic filter"),
            qos: Qos::AtLeastOnce,
            no_local: false,
            retain_as_published: false,
            retain_handling: RetainHandling::SendRetained,
        },
        extra_subscriptions: vec![],
        subscription_identifier: None,
        user_properties: vec![],
    }
}

/// Build a ClientMessage for publishing.
pub fn msg(topic: &str, payload: &[u8], qos: Qos) -> ClientMessage {
    ClientMessage {
        topic: Topic::try_new(topic.as_bytes().to_vec()).expect("valid topic"),
        payload: Payload::from(payload),
        qos,
        ..ClientMessage::default()
    }
}
