use sansio_mqtt_v5_protocol::{
    BinaryData, ClientMessage, ConnectionOptions, SubscribeOptions, Subscription,
};
use sansio_mqtt_v5_tokio::ConnectOptions;
use sansio_mqtt_v5_types::{Payload, Qos, RetainHandling, Topic, Utf8String};
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, CopyDataSource, GenericImage, ImageExt};

/// Starts an anonymous Mosquitto 2 broker, returns the container (keep alive for test duration)
/// and the mapped host TCP port.
pub async fn anonymous_broker() -> (ContainerAsync<GenericImage>, u16) {
    let container = GenericImage::new("eclipse-mosquitto", "2")
        .with_exposed_port(1883.tcp())
        .with_wait_for(WaitFor::message_on_stderr("mosquitto version"))
        .with_copy_to(
            "/mosquitto/config/mosquitto.conf",
            CopyDataSource::Data(include_bytes!("mosquitto/anonymous.conf").to_vec()),
        )
        .start()
        .await
        .expect("mosquitto starts");
    let port = container
        .get_host_port_ipv4(1883)
        .await
        .expect("gets host port");
    (container, port)
}

/// Starts an authenticated Mosquitto 2 broker (requires testuser/testpassword).
pub async fn authenticated_broker() -> (ContainerAsync<GenericImage>, u16) {
    let container = GenericImage::new("eclipse-mosquitto", "2")
        .with_exposed_port(1883.tcp())
        .with_wait_for(WaitFor::message_on_stderr("mosquitto version"))
        .with_copy_to(
            "/mosquitto/config/mosquitto.conf",
            CopyDataSource::Data(include_bytes!("mosquitto/authenticated.conf").to_vec()),
        )
        .with_copy_to(
            "/mosquitto/config/passwd",
            CopyDataSource::Data(include_bytes!("mosquitto/passwd").to_vec()),
        )
        .start()
        .await
        .expect("mosquitto starts");
    let port = container
        .get_host_port_ipv4(1883)
        .await
        .expect("gets host port");
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
pub fn authenticated_connect_options(port: u16, client_id: &str, user: &str, pass: &str) -> ConnectOptions {
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
