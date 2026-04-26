pub use sansio_mqtt_v5_tokio::*;
use testcontainers::core::IntoContainerPort;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers::GenericImage;
use testcontainers::ImageExt;

const MOSQUITTO_IMAGE: &str = "eclipse-mosquitto";
const MOSQUITTO_TAG: &str = "2";
const MOSQUITTO_PORT: u16 = 1883;

/// Starts an anonymous Mosquitto 2 broker in a container, returns the container
/// handle (keep alive for test duration) and the mapped host TCP port.
pub async fn anonymous_broker() -> (ContainerAsync<GenericImage>, u16) {
    let container = GenericImage::new(MOSQUITTO_IMAGE, MOSQUITTO_TAG)
        .with_exposed_port(MOSQUITTO_PORT.tcp())
        .with_wait_for(WaitFor::message_on_stdout("mosquitto version"))
        .with_copy_to(
            "/mosquitto/config/mosquitto.conf",
            b"listener 1883\nallow_anonymous true\nlog_dest stdout\n".to_vec(),
        )
        .start()
        .await
        .expect("start mosquitto container");

    let port = container
        .get_host_port_ipv4(MOSQUITTO_PORT)
        .await
        .expect("get mosquitto port");

    (container, port)
}

/// Starts an authenticated Mosquitto 2 broker (requires testuser/testpassword).
pub async fn authenticated_broker() -> (ContainerAsync<GenericImage>, u16) {
    let config =
        "listener 1883\nallow_anonymous false\npassword_file /mosquitto/config/passwd\nlog_dest stdout\n";
    let passwd = include_bytes!("passwd");

    let container = GenericImage::new(MOSQUITTO_IMAGE, MOSQUITTO_TAG)
        .with_exposed_port(MOSQUITTO_PORT.tcp())
        .with_wait_for(WaitFor::message_on_stdout("mosquitto version"))
        .with_copy_to(
            "/mosquitto/config/mosquitto.conf",
            config.as_bytes().to_vec(),
        )
        .with_copy_to("/mosquitto/config/passwd", passwd.to_vec())
        .start()
        .await
        .expect("start authenticated mosquitto container");

    let port = container
        .get_host_port_ipv4(MOSQUITTO_PORT)
        .await
        .expect("get mosquitto port");

    (container, port)
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

/// Connect options for a persistent session: clean_start=true,
/// session_expiry=300s.
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

/// Connect options to resume an existing session: clean_start=false,
/// session_expiry=300s.
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

/// Subscribe at QoS 0 (broker delivers at most-once; receiver gets
/// Event::Message).
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
