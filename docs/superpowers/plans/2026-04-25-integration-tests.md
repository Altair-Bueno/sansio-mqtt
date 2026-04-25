# Integration Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add feature-gated integration tests to `sansio-mqtt-v5-tokio` that
verify the full MQTT v5.0 client against a real Mosquitto 2 broker managed by
testcontainers.

**Architecture:** Tests live in `crates/sansio-mqtt-v5-tokio/tests/integration/`
and are compiled only when the `integration-tests` Cargo feature is enabled.
Each test starts its own container for full isolation. Shared helpers in
`common.rs` encapsulate container startup and `ConnectOptions` construction.

**Tech Stack:** testcontainers 0.23 (GenericImage + AsyncRunner), Eclipse
Mosquitto 2 image, tokio multi-thread runtime, Cargo `required-features`.

---

## File Structure

| File                                             | Action | Responsibility                                                          |
| ------------------------------------------------ | ------ | ----------------------------------------------------------------------- |
| `crates/sansio-mqtt-v5-tokio/Cargo.toml`         | Modify | Add feature, `[[test]]` entry, testcontainers dev-dep                   |
| `tests/integration/main.rs`                      | Create | `mod` declarations for all submodules                                   |
| `tests/integration/common.rs`                    | Create | Container helpers, `ConnectOptions` builders, subscribe/publish helpers |
| `tests/integration/core_flows.rs`                | Create | 5 core tests: connect, QoS 0/1/2, keep-alive                            |
| `tests/integration/session.rs`                   | Create | 3 session tests: clean start, resumption, will                          |
| `tests/integration/auth.rs`                      | Create | 3 auth tests: valid creds, invalid creds, anon rejected                 |
| `tests/integration/mosquitto/anonymous.conf`     | Create | Mosquitto config: allow anonymous, listen 1883                          |
| `tests/integration/mosquitto/authenticated.conf` | Create | Mosquitto config: require password file                                 |
| `tests/integration/mosquitto/passwd`             | Create | Pre-generated mosquitto_passwd file                                     |

---

## Task 1: Cargo.toml — Feature Flag, Dev-Dep, [[test]]

**Files:**

- Modify: `crates/sansio-mqtt-v5-tokio/Cargo.toml`

- [ ] **Step 1: Add feature, dev-dep, and [[test]] entry**

Replace the contents of `crates/sansio-mqtt-v5-tokio/Cargo.toml` with:

```toml
[package]
name = "sansio-mqtt-v5-tokio"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[features]
integration-tests = []

[dependencies]
sansio = { workspace = true }
sansio-mqtt-v5-protocol = { workspace = true, features = ["tokio"] }
sansio-mqtt-v5-types = { workspace = true }
bytes = { workspace = true }
tokio = { workspace = true, features = [
  "macros",
  "net",
  "io-util",
  "io-std",
  "signal",
  "rt",
  "sync",
  "time",
] }
tracing = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = [
  "macros",
  "net",
  "io-util",
  "io-std",
  "signal",
  "rt-multi-thread",
  "sync",
  "time",
] }
encode = { workspace = true }
tracing-subscriber = { workspace = true, features = ["fmt", "ansi"] }
testcontainers = { version = "0.23" }

[[test]]
name = "mqtt_integration"
path = "tests/integration/main.rs"
required-features = ["integration-tests"]
```

- [ ] **Step 2: Verify baseline tests still pass**

```bash
cargo test -p sansio-mqtt-v5-tokio
```

Expected: all existing tests pass (no `mqtt_integration` binary compiled yet
because feature is not enabled).

- [ ] **Step 3: Verify feature-gated compilation succeeds**

```bash
cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration --no-run
```

Expected: error about missing `tests/integration/main.rs` (binary would compile
if file existed). The important thing: no dependency errors.

- [ ] **Step 4: Commit**

```bash
git add crates/sansio-mqtt-v5-tokio/Cargo.toml
git commit -m "feat(tokio): add integration-tests feature gate and testcontainers dev-dep"
```

---

## Task 2: Mosquitto Configuration Files

**Files:**

- Create:
  `crates/sansio-mqtt-v5-tokio/tests/integration/mosquitto/anonymous.conf`
- Create:
  `crates/sansio-mqtt-v5-tokio/tests/integration/mosquitto/authenticated.conf`
- Create: `crates/sansio-mqtt-v5-tokio/tests/integration/mosquitto/passwd`

- [ ] **Step 1: Create anonymous.conf**

Create `crates/sansio-mqtt-v5-tokio/tests/integration/mosquitto/anonymous.conf`:

```
listener 1883
allow_anonymous true
```

- [ ] **Step 2: Create authenticated.conf**

Create
`crates/sansio-mqtt-v5-tokio/tests/integration/mosquitto/authenticated.conf`:

```
listener 1883
allow_anonymous false
password_file /mosquitto/config/passwd
```

- [ ] **Step 3: Generate the passwd file**

Run this command to generate a mosquitto password file for
`testuser`/`testpassword`:

```bash
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  podman run --rm eclipse-mosquitto:2 \
  sh -c 'mosquitto_passwd -b -c /tmp/p testuser testpassword && cat /tmp/p'
```

Save the output (one line beginning with `testuser:$7$...`) to:
`crates/sansio-mqtt-v5-tokio/tests/integration/mosquitto/passwd`

The file should look like:

```
testuser:$7$101$<base64-salt>$<base64-hash>
```

- [ ] **Step 4: Verify passwd file is non-empty**

```bash
cat crates/sansio-mqtt-v5-tokio/tests/integration/mosquitto/passwd
```

Expected: one line starting with `testuser:$7$`.

- [ ] **Step 5: Commit**

```bash
git add crates/sansio-mqtt-v5-tokio/tests/integration/mosquitto/
git commit -m "feat(tokio/integration): add Mosquitto config files for anonymous and authenticated brokers"
```

---

## Task 3: Test Scaffold — main.rs and common.rs

**Files:**

- Create: `crates/sansio-mqtt-v5-tokio/tests/integration/main.rs`
- Create: `crates/sansio-mqtt-v5-tokio/tests/integration/common.rs`

- [ ] **Step 1: Create main.rs**

Create `crates/sansio-mqtt-v5-tokio/tests/integration/main.rs`:

```rust
mod auth;
mod common;
mod core_flows;
mod session;
```

- [ ] **Step 2: Create common.rs**

Create `crates/sansio-mqtt-v5-tokio/tests/integration/common.rs`:

```rust
use sansio_mqtt_v5_protocol::{
    BinaryData, ClientMessage, ConnectionOptions, SubscribeOptions, Subscription,
};
use sansio_mqtt_v5_tokio::ConnectOptions;
use sansio_mqtt_v5_types::{Payload, Qos, RetainHandling, Topic, Utf8String};
use testcontainers::core::{CopyDataSource, IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};

/// Starts an anonymous Mosquitto 2 broker, returns the container (keep alive for test duration)
/// and the mapped host TCP port.
pub async fn anonymous_broker() -> (ContainerAsync<GenericImage>, u16) {
    let container = GenericImage::new("eclipse-mosquitto", "2")
        .with_exposed_port(1883.tcp())
        .with_copy_to(
            "/mosquitto/config/mosquitto.conf",
            CopyDataSource::Data(include_bytes!("mosquitto/anonymous.conf").to_vec()),
        )
        .with_wait_for(WaitFor::message_on_stderr("mosquitto version"))
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
        .with_copy_to(
            "/mosquitto/config/mosquitto.conf",
            CopyDataSource::Data(include_bytes!("mosquitto/authenticated.conf").to_vec()),
        )
        .with_copy_to(
            "/mosquitto/config/passwd",
            CopyDataSource::Data(include_bytes!("mosquitto/passwd").to_vec()),
        )
        .with_wait_for(WaitFor::message_on_stderr("mosquitto version"))
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
pub fn authenticated_connect_options(port: u16, user: &str, pass: &str) -> ConnectOptions {
    ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("auth-test-client").expect("valid"),
            user_name: Some(Utf8String::try_from(user).expect("valid username")),
            password: Some(BinaryData::new(pass.as_bytes())),
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
        topic: Topic::try_new(topic).expect("valid topic"),
        payload: Payload::from(payload),
        qos,
        ..ClientMessage::default()
    }
}
```

- [ ] **Step 3: Create stub files for the remaining modules so the binary
      compiles**

Create `crates/sansio-mqtt-v5-tokio/tests/integration/core_flows.rs`:

```rust
// tests added in Task 4
```

Create `crates/sansio-mqtt-v5-tokio/tests/integration/session.rs`:

```rust
// tests added in Task 5
```

Create `crates/sansio-mqtt-v5-tokio/tests/integration/auth.rs`:

```rust
// tests added in Task 6
```

- [ ] **Step 4: Verify the integration binary compiles**

```bash
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration --no-run
```

Expected: `Finished` with no errors. Zero tests listed (stubs are empty).

- [ ] **Step 5: Commit**

```bash
git add crates/sansio-mqtt-v5-tokio/tests/integration/
git commit -m "feat(tokio/integration): add test scaffold (main.rs, common.rs, module stubs)"
```

---

## Task 4: core_flows.rs — Connect, QoS 0/1/2, Keep-Alive

**Files:**

- Modify: `crates/sansio-mqtt-v5-tokio/tests/integration/core_flows.rs`

- [ ] **Step 1: Write the five core flow tests**

Replace `crates/sansio-mqtt-v5-tokio/tests/integration/core_flows.rs` with:

```rust
use std::time::Duration;

use sansio_mqtt_v5_protocol::{ConnectionOptions, Will};
use sansio_mqtt_v5_tokio::{connect, ConnectOptions, Event};
use sansio_mqtt_v5_types::{Payload, Qos, Topic, Utf8String};

use crate::common;

#[tokio::test]
async fn connect_and_disconnect() {
    let (_container, port) = common::anonymous_broker().await;

    let (client, mut event_loop) = connect(common::connect_options(port, "conn-disc"))
        .await
        .expect("connect");

    let event = event_loop.poll().await.expect("poll");
    assert!(matches!(event, Event::Connected), "expected Connected, got {event:?}");

    client.disconnect().await.expect("disconnect");

    let event = event_loop.poll().await.expect("poll after disconnect");
    assert!(
        matches!(event, Event::Disconnected(None)),
        "expected Disconnected(None), got {event:?}"
    );
}

#[tokio::test]
async fn publish_qos0() {
    let (_container, port) = common::anonymous_broker().await;

    // Subscriber: connect, subscribe at QoS 0, wait in background for the message.
    let (client_sub, mut el_sub) = connect(common::connect_options(port, "qos0-sub"))
        .await
        .expect("connect subscriber");
    assert!(matches!(
        el_sub.poll().await.expect("subscriber connected"),
        Event::Connected
    ));
    client_sub
        .subscribe(common::sub("test/qos0"))
        .await
        .expect("subscribe");
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    // Allow time for SUBSCRIBE/SUBACK round-trip before publishing.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Publisher: connect, publish QoS 0, give event loop time to flush the packet.
    let (client_pub, mut el_pub) = connect(common::connect_options(port, "qos0-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        el_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    client_pub
        .publish(common::msg("test/qos0", b"hello-qos0", Qos::AtMostOnce))
        .await
        .expect("publish");
    // Poll briefly so the PUBLISH is flushed; timeout is expected because QoS 0 has no response.
    let _ = tokio::time::timeout(Duration::from_millis(200), el_pub.poll()).await;

    let event = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("subscriber receives message within 3s")
        .expect("sub task joins")
        .expect("subscriber event");
    assert!(matches!(event, Event::Message(_)), "expected Message, got {event:?}");
}

#[tokio::test]
async fn publish_qos1() {
    let (_container, port) = common::anonymous_broker().await;

    // Subscriber at QoS 0 — broker delivers at most-once regardless of publish QoS.
    let (client_sub, mut el_sub) = connect(common::connect_options(port, "qos1-sub"))
        .await
        .expect("connect subscriber");
    assert!(matches!(
        el_sub.poll().await.expect("subscriber connected"),
        Event::Connected
    ));
    client_sub
        .subscribe(common::sub("test/qos1"))
        .await
        .expect("subscribe");
    let sub_task = tokio::spawn(async move { el_sub.poll().await });
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Publisher at QoS 1 — expects PublishAcknowledged from broker.
    let (client_pub, mut el_pub) = connect(common::connect_options(port, "qos1-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        el_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    client_pub
        .publish(common::msg("test/qos1", b"hello-qos1", Qos::AtLeastOnce))
        .await
        .expect("publish");
    let pub_event = el_pub.poll().await.expect("puback");
    assert!(
        matches!(pub_event, Event::PublishAcknowledged(_, _)),
        "expected PublishAcknowledged, got {pub_event:?}"
    );

    let sub_event = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("subscriber receives message within 3s")
        .expect("sub task joins")
        .expect("subscriber event");
    assert!(matches!(sub_event, Event::Message(_)), "expected Message, got {sub_event:?}");
}

#[tokio::test]
async fn publish_qos2() {
    let (_container, port) = common::anonymous_broker().await;

    // Subscriber at QoS 0.
    let (client_sub, mut el_sub) = connect(common::connect_options(port, "qos2-sub"))
        .await
        .expect("connect subscriber");
    assert!(matches!(
        el_sub.poll().await.expect("subscriber connected"),
        Event::Connected
    ));
    client_sub
        .subscribe(common::sub("test/qos2"))
        .await
        .expect("subscribe");
    let sub_task = tokio::spawn(async move { el_sub.poll().await });
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Publisher at QoS 2 — full PUBLISH→PUBREC→PUBREL→PUBCOMP flow.
    let (client_pub, mut el_pub) = connect(common::connect_options(port, "qos2-pub"))
        .await
        .expect("connect publisher");
    assert!(matches!(
        el_pub.poll().await.expect("publisher connected"),
        Event::Connected
    ));
    client_pub
        .publish(common::msg("test/qos2", b"hello-qos2", Qos::ExactlyOnce))
        .await
        .expect("publish");
    let pub_event = el_pub.poll().await.expect("pubcomp");
    assert!(
        matches!(pub_event, Event::PublishCompleted(_, _)),
        "expected PublishCompleted, got {pub_event:?}"
    );

    let sub_event = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("subscriber receives message within 3s")
        .expect("sub task joins")
        .expect("subscriber event");
    assert!(matches!(sub_event, Event::Message(_)), "expected Message, got {sub_event:?}");
}

#[tokio::test]
async fn keep_alive_maintains_connection() {
    let (_container, port) = common::anonymous_broker().await;

    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("keepalive-test").expect("valid"),
            keep_alive: core::num::NonZero::new(5),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };

    let (_client, mut event_loop) = connect(opts).await.expect("connect");
    assert!(matches!(
        event_loop.poll().await.expect("connected"),
        Event::Connected
    ));

    // Poll for 7 seconds. PINGREQ/PINGRESP are transparent — no Event emitted.
    // A timeout means the connection stayed alive. An Ok result means an unexpected
    // event arrived. An Err result means the connection dropped.
    let result = tokio::time::timeout(Duration::from_secs(7), event_loop.poll()).await;
    match result {
        Err(_elapsed) => { /* success: connection alive after 7s idle */ }
        Ok(Err(err)) => panic!("connection dropped during keep-alive window: {err:?}"),
        Ok(Ok(event)) => panic!("unexpected event during keep-alive window: {event:?}"),
    }
}
```

- [ ] **Step 2: Run the core_flows tests**

```bash
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration core_flows
```

Expected: `connect_and_disconnect`, `publish_qos0`, `publish_qos1`,
`publish_qos2` pass quickly; `keep_alive_maintains_connection` takes ~7 seconds.

- [ ] **Step 3: Commit**

```bash
git add crates/sansio-mqtt-v5-tokio/tests/integration/core_flows.rs
git commit -m "test(tokio/integration): add core_flows integration tests (connect, QoS 0/1/2, keep-alive)"
```

---

## Task 5: session.rs — Clean Start, Session Resumption, Will Message

**Files:**

- Modify: `crates/sansio-mqtt-v5-tokio/tests/integration/session.rs`

- [ ] **Step 1: Write the three session tests**

Replace `crates/sansio-mqtt-v5-tokio/tests/integration/session.rs` with:

```rust
use std::time::Duration;

use sansio_mqtt_v5_protocol::{ConnectionOptions, Will};
use sansio_mqtt_v5_tokio::{connect, ConnectOptions, Event};
use sansio_mqtt_v5_types::{Payload, Qos, Topic, Utf8String};

use crate::common;

#[tokio::test]
async fn clean_start_clears_session() {
    let (_container, port) = common::anonymous_broker().await;

    // Phase 1: establish a persistent session and subscribe at QoS 1.
    {
        let (client_a, mut el_a) =
            connect(common::persistent_connect_options(port, "clean-start-client"))
                .await
                .expect("first connect");
        assert!(matches!(el_a.poll().await.expect("connected"), Event::Connected));
        client_a
            .subscribe(common::sub_qos1("test/clean-start"))
            .await
            .expect("subscribe");
        tokio::time::sleep(Duration::from_millis(150)).await; // wait for SUBACK
        client_a.disconnect().await.expect("disconnect");
        // Poll to flush the DISCONNECT and receive Disconnected event.
        let _ = tokio::time::timeout(Duration::from_secs(1), el_a.poll()).await;
    }

    // Phase 2: publish while client A is offline — broker queues the message for the session.
    {
        let (client_pub, mut el_pub) =
            connect(common::connect_options(port, "clean-start-pub"))
                .await
                .expect("publisher connect");
        assert!(matches!(el_pub.poll().await.expect("connected"), Event::Connected));
        client_pub
            .publish(common::msg("test/clean-start", b"queued", Qos::AtLeastOnce))
            .await
            .expect("publish");
        // Wait for PUBACK (proves the broker accepted and queued the message).
        let pub_event = el_pub.poll().await.expect("puback");
        assert!(
            matches!(pub_event, Event::PublishAcknowledged(_, _)),
            "expected puback, got {pub_event:?}"
        );
    }

    // Phase 3: reconnect with clean_start=true — session wiped, no queued messages delivered.
    {
        let (_client_a, mut el_a) =
            connect(common::connect_options(port, "clean-start-client"))
                .await
                .expect("second connect");
        assert!(matches!(el_a.poll().await.expect("connected"), Event::Connected));

        // Any queued message would arrive almost immediately; 300ms is sufficient.
        let result =
            tokio::time::timeout(Duration::from_millis(300), el_a.poll()).await;
        assert!(
            result.is_err(),
            "no queued message should arrive after clean start, but got: {result:?}"
        );
    }
}

#[tokio::test]
async fn session_resumption() {
    let (_container, port) = common::anonymous_broker().await;

    // Phase 1: establish persistent session and subscribe at QoS 1.
    {
        let (client_a, mut el_a) =
            connect(common::persistent_connect_options(port, "resume-client"))
                .await
                .expect("first connect");
        assert!(matches!(el_a.poll().await.expect("connected"), Event::Connected));
        client_a
            .subscribe(common::sub_qos1("test/resume"))
            .await
            .expect("subscribe");
        tokio::time::sleep(Duration::from_millis(150)).await;
        client_a.disconnect().await.expect("disconnect");
        let _ = tokio::time::timeout(Duration::from_secs(1), el_a.poll()).await;
    }

    // Phase 2: publish while client A is offline.
    {
        let (client_pub, mut el_pub) =
            connect(common::connect_options(port, "resume-pub"))
                .await
                .expect("publisher connect");
        assert!(matches!(el_pub.poll().await.expect("connected"), Event::Connected));
        client_pub
            .publish(common::msg("test/resume", b"queued-for-resume", Qos::AtLeastOnce))
            .await
            .expect("publish");
        let pub_event = el_pub.poll().await.expect("puback");
        assert!(
            matches!(pub_event, Event::PublishAcknowledged(_, _)),
            "expected puback, got {pub_event:?}"
        );
    }

    // Phase 3: reconnect with clean_start=false — receives the queued message.
    {
        let (_client_a, mut el_a) =
            connect(common::resume_connect_options(port, "resume-client"))
                .await
                .expect("resume connect");
        assert!(matches!(el_a.poll().await.expect("connected"), Event::Connected));

        let event = tokio::time::timeout(Duration::from_secs(3), el_a.poll())
            .await
            .expect("queued message arrives within 3s")
            .expect("no event loop error");
        // Queued QoS 1 message arrives as MessageWithRequiredAcknowledgement.
        assert!(
            matches!(
                event,
                Event::MessageWithRequiredAcknowledgement(_, _)
            ),
            "expected queued message, got {event:?}"
        );
    }
}

#[tokio::test]
async fn will_message_delivered() {
    let (_container, port) = common::anonymous_broker().await;

    // Subscriber: subscribe to the will topic and wait for the will in background.
    let (client_sub, mut el_sub) =
        connect(common::connect_options(port, "will-subscriber"))
            .await
            .expect("connect subscriber");
    assert!(matches!(el_sub.poll().await.expect("connected"), Event::Connected));
    client_sub
        .subscribe(common::sub("will/gone"))
        .await
        .expect("subscribe to will topic");
    let sub_task = tokio::spawn(async move { el_sub.poll().await });
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Will sender: connect with a will, receive Connected, then drop without DISCONNECT.
    // Dropping the EventLoop closes the TCP socket; Mosquitto detects the unclean close
    // and publishes the will immediately (will_delay_interval defaults to 0).
    {
        let will = Will {
            topic: Topic::try_new("will/gone").expect("valid will topic"),
            payload: Payload::from(&b"sender-gone"[..]),
            qos: Qos::AtMostOnce,
            retain: false,
            will_delay_interval: None, // publish immediately on disconnect
            ..Will::default()
        };
        let opts = ConnectOptions {
            addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
            connection: ConnectionOptions {
                clean_start: true,
                client_identifier: Utf8String::try_from("will-sender").expect("valid"),
                will: Some(will),
                session_expiry_interval: Some(0), // expire session on disconnect
                ..ConnectionOptions::default()
            },
            ..ConnectOptions::default()
        };
        let (_client, mut el_will) = connect(opts).await.expect("connect will sender");
        assert!(matches!(el_will.poll().await.expect("connected"), Event::Connected));
        // Drop EventLoop — abrupt TCP close, no DISCONNECT sent.
    }

    // The will should arrive at the subscriber shortly after the connection drops.
    let event = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await
        .expect("will arrives within 3s")
        .expect("sub task joins")
        .expect("subscriber event");
    assert!(
        matches!(event, Event::Message(_)),
        "expected will message, got {event:?}"
    );
}
```

- [ ] **Step 2: Run the session tests**

```bash
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration session
```

Expected: all three session tests pass. `clean_start_clears_session` uses a
300ms timeout (fast). `session_resumption` and `will_message_delivered` complete
within 3s.

- [ ] **Step 3: Commit**

```bash
git add crates/sansio-mqtt-v5-tokio/tests/integration/session.rs
git commit -m "test(tokio/integration): add session integration tests (clean start, resumption, will)"
```

---

## Task 6: auth.rs — Valid Credentials, Invalid Credentials, Anonymous Rejected

**Files:**

- Modify: `crates/sansio-mqtt-v5-tokio/tests/integration/auth.rs`

- [ ] **Step 1: Write the three auth tests**

Replace `crates/sansio-mqtt-v5-tokio/tests/integration/auth.rs` with:

```rust
use sansio_mqtt_v5_tokio::{connect, Event};

use crate::common;

#[tokio::test]
async fn valid_credentials_accepted() {
    let (_container, port) = common::authenticated_broker().await;

    let (_client, mut event_loop) =
        connect(common::authenticated_connect_options(port, "testuser", "testpassword"))
            .await
            .expect("tcp connect");

    let event = event_loop.poll().await.expect("poll");
    assert!(
        matches!(event, Event::Connected),
        "expected Connected with valid credentials, got {event:?}"
    );
}

#[tokio::test]
async fn invalid_credentials_rejected() {
    let (_container, port) = common::authenticated_broker().await;

    let (_client, mut event_loop) =
        connect(common::authenticated_connect_options(port, "testuser", "wrongpassword"))
            .await
            .expect("tcp connect succeeds");

    // Broker sends CONNACK with a failure reason code; the protocol layer returns an error.
    let result = event_loop.poll().await;
    assert!(
        result.is_err(),
        "expected error for wrong password, got Ok({:?})",
        result.ok()
    );
}

#[tokio::test]
async fn anonymous_rejected() {
    let (_container, port) = common::authenticated_broker().await;

    // No credentials provided — broker rejects the connection.
    let (_client, mut event_loop) =
        connect(common::connect_options(port, "anon-client"))
            .await
            .expect("tcp connect succeeds");

    let result = event_loop.poll().await;
    assert!(
        result.is_err(),
        "expected error for anonymous connect to authenticated broker, got Ok({:?})",
        result.ok()
    );
}
```

- [ ] **Step 2: Run the auth tests**

```bash
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration auth
```

Expected: all three auth tests pass. The two rejection tests should complete
quickly (broker sends CONNACK immediately).

- [ ] **Step 3: Run the full integration suite**

```bash
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration
```

Expected: 11 tests pass. Only `keep_alive_maintains_connection` takes ~7
seconds; all others complete in under 3 seconds.

- [ ] **Step 4: Verify baseline tests still pass**

```bash
cargo test -p sansio-mqtt-v5-tokio
```

Expected: existing tests pass, no `mqtt_integration` binary compiled.

- [ ] **Step 5: Commit**

```bash
git add crates/sansio-mqtt-v5-tokio/tests/integration/auth.rs
git commit -m "test(tokio/integration): add auth integration tests (valid, invalid credentials, anonymous rejected)"
```

---

## Running the Full Suite

```sh
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration
```

To run a specific test:

```sh
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration session::session_resumption
```
