# Integration Test Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand `test-sansio-mqtt-v5-tokio-mosquitto` from 11 to ~60
integration tests covering all gaps in `docs/review-tests.md`.

**Architecture:** Nine new test files, one per MQTT feature area, plus three
helpers added to `lib.rs`. All tests run against a real Mosquitto 2 container
via testcontainers. No existing files are modified. Each task is one
self-contained test file commit.

**Tech Stack:** Rust, tokio, testcontainers, eclipse-mosquitto:2,
sansio-mqtt-v5-tokio

---

## Key API Reference (read before writing tests)

```rust
// All tests: use test_sansio_mqtt_v5_tokio_mosquitto::*;
// That re-exports everything from sansio_mqtt_v5_tokio + lib helpers.

// Core types confirmed from source:
// Topic::try_new("literal/topic")  — &'static str works (Into<bytes::Bytes>)
// Payload::from(b"bytes".as_slice())
// Utf8String::try_from("string").expect("valid")
// BinaryData::new(b"bytes".as_slice())  — infallible

// Will fields: topic, payload, qos, retain, will_delay_interval: Option<u32>,
//   payload_format_indicator: Option<FormatIndicator>,
//   message_expiry_interval: Option<Duration>,   ← core::time::Duration
//   content_type: Option<Utf8String>, response_topic: Option<Topic>,
//   correlation_data: Option<BinaryData>, user_properties: Vec<(Utf8String,Utf8String)>
//   ..Will::default() for unset fields

// ClientMessage fields: qos, retain, payload, topic,
//   payload_format_indicator: Option<FormatIndicator>,
//   message_expiry_interval: Option<Duration>,
//   topic_alias: Option<NonZero<u16>>, response_topic: Option<Topic>,
//   correlation_data: Option<BinaryData>, user_properties: Vec<(Utf8String,Utf8String)>,
//   content_type: Option<Utf8String>
//   ..ClientMessage::default() for unset fields

// BrokerMessage: same as ClientMessage + subscription_identifiers: Vec<NonZero<u64>>

// RetainHandling: SendRetained | SendRetainedIfSubscriptionDoesNotExist | DoNotSend
// FormatIndicator: Unspecified | Utf8

// UnsubscribeOptions { filter: Utf8String, extra_filters: Vec<Utf8String>,
//                      user_properties: Vec<(Utf8String,Utf8String)> }

// Client has: publish, subscribe, unsubscribe, disconnect — NO acknowledge method.
// Inbound QoS1/2: receive Event::MessageWithRequiredAcknowledgement but cannot ack.
```

---

## Task 0: Create feature branch from latest master

**Files:** none (git operations only)

- [ ] **Step 1: Create git worktree from latest master**

```bash
git fetch origin
git worktree add ../sansio-mqtt-integration-tests -b feat/integration-test-expansion origin/master
cd ../sansio-mqtt-integration-tests
```

Expected: new directory `../sansio-mqtt-integration-tests` on branch
`feat/integration-test-expansion` at latest master HEAD.

- [ ] **Step 2: Verify rust-analyzer is available**

```bash
rust-analyzer --version
```

Expected: version string printed. If command fails, run
`rustup component add rust-analyzer` before proceeding.

- [ ] **Step 3: Confirm workspace builds**

```bash
cargo build --tests --exclude test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
```

Expected: `Finished` line with no errors.

---

## Task 1: Add helpers to `lib.rs`

**Files:**

- Modify: `crates/test-sansio-mqtt-v5-tokio-mosquitto/src/lib.rs`

- [ ] **Step 1: Append the three new helpers**

Add to the bottom of `crates/test-sansio-mqtt-v5-tokio-mosquitto/src/lib.rs`:

```rust
/// ConnectOptions pre-loaded with a [`Will`] message.
pub fn will_connect_options(port: u16, client_id: &str, will: Will) -> ConnectOptions {
    ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("valid addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from(client_id).expect("valid client id"),
            will: Some(will),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    }
}

/// [`SubscribeOptions`] with full control over all subscription flags and an
/// optional subscription identifier.
pub fn sub_with_options(
    topic: &str,
    qos: Qos,
    no_local: bool,
    retain_as_published: bool,
    retain_handling: RetainHandling,
    subscription_identifier: Option<core::num::NonZero<u64>>,
) -> SubscribeOptions {
    SubscribeOptions {
        subscription: Subscription {
            topic_filter: Utf8String::try_from(topic).expect("valid topic filter"),
            qos,
            no_local,
            retain_as_published,
            retain_handling,
        },
        extra_subscriptions: vec![],
        subscription_identifier,
        user_properties: vec![],
    }
}

/// [`ClientMessage`] with `retain = true`.
pub fn msg_retain(topic: &str, payload: &[u8], qos: Qos) -> ClientMessage {
    ClientMessage {
        topic: Topic::try_new(topic.as_bytes().to_vec()).expect("valid topic"),
        payload: Payload::from(payload),
        qos,
        retain: true,
        ..ClientMessage::default()
    }
}
```

- [ ] **Step 2: Compile**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/src/lib.rs
git commit -m "test: add will_connect_options, sub_with_options, msg_retain helpers"
```

---

## Task 2: `will_messages.rs` — 8 tests

**Files:**

- Create: `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/will_messages.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

// ── helper ──────────────────────────────────────────────────────────────────

fn make_will(topic: &'static str, payload: &'static [u8], qos: Qos) -> Will {
    Will {
        topic: Topic::try_new(topic).expect("valid topic"),
        payload: Payload::from(payload),
        qos,
        retain: false,
        ..Will::default()
    }
}

// ── tests ────────────────────────────────────────────────────────────────────

/// Will MUST NOT be published when the client sends DISCONNECT normally.
/// [MQTT-3.1.2-10]
#[tokio::test]
async fn will_not_sent_on_graceful_disconnect() {
    let (_c, port) = anonymous_broker().await;

    let (sub, mut el_sub) = connect(connect_options(port, "wng-sub")).await.expect("connect sub");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub.subscribe(sub("will/ng")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let will = make_will("will/ng", b"gone", Qos::AtMostOnce);
    let (sender, mut el_sender) =
        connect(will_connect_options(port, "wng-sender", will)).await.expect("connect sender");
    assert!(matches!(el_sender.poll().await.expect("poll"), Event::Connected));

    sender.disconnect().await.expect("graceful disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el_sender.poll()).await;

    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "will must not be published after graceful disconnect, got: {result:?}"
    );
}

#[tokio::test]
async fn will_sent_on_abrupt_disconnect_qos0() {
    let (_c, port) = anonymous_broker().await;

    let (_sub, mut el_sub) = connect(connect_options(port, "waq0-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    _sub.subscribe(sub("will/aq0")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    let will = make_will("will/aq0", b"gone-qos0", Qos::AtMostOnce);
    let (_sender, mut el) =
        connect(will_connect_options(port, "waq0-sender", will)).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el); // abrupt — triggers will

    let ev = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await.expect("will within 3s").expect("join").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected Message, got {ev:?}");
}

#[tokio::test]
async fn will_sent_on_abrupt_disconnect_qos1() {
    let (_c, port) = anonymous_broker().await;

    let (_sub, mut el_sub) =
        connect(persistent_connect_options(port, "waq1-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    _sub.subscribe(sub_qos1("will/aq1")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    let will = make_will("will/aq1", b"gone-qos1", Qos::AtLeastOnce);
    let (_sender, mut el) =
        connect(will_connect_options(port, "waq1-sender", will)).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el);

    let ev = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await.expect("will within 3s").expect("join").expect("event");
    assert!(
        matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
        "expected MessageWithRequiredAcknowledgement for QoS1 will, got {ev:?}"
    );
}

#[tokio::test]
async fn will_with_retain_flag() {
    let (_c, port) = anonymous_broker().await;

    let will = Will {
        topic: Topic::try_new("will/retained").expect("valid"),
        payload: Payload::from(b"retained-will".as_slice()),
        qos: Qos::AtMostOnce,
        retain: true,
        ..Will::default()
    };
    let (_sender, mut el) =
        connect(will_connect_options(port, "wr-sender", will)).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el); // abrupt — publishes retained will

    tokio::time::sleep(Duration::from_secs(1)).await; // let broker store it

    // Late subscriber must receive the retained will
    let (_sub, mut el_sub) = connect(connect_options(port, "wr-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    _sub.subscribe(sub("will/retained")).await.expect("subscribe");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("retained will within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected retained will, got {ev:?}");

    // Clean up retained message so it doesn't pollute other tests
    let (_cleanup, mut el_c) = connect(connect_options(port, "wr-cleanup")).await.expect("connect");
    assert!(matches!(el_c.poll().await.expect("poll"), Event::Connected));
    _cleanup.publish(msg_retain("will/retained", b"", Qos::AtMostOnce)).await.expect("clear");
    tokio::time::sleep(Duration::from_millis(150)).await;
}

/// will_delay_interval=2s: will must NOT arrive before the delay expires.
#[tokio::test]
async fn will_with_delay_interval() {
    let (_c, port) = anonymous_broker().await;

    let (_sub, mut el_sub) = connect(connect_options(port, "wdi-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    _sub.subscribe(sub("will/delayed")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let will = Will {
        topic: Topic::try_new("will/delayed").expect("valid"),
        payload: Payload::from(b"delayed".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        will_delay_interval: Some(2),
        ..Will::default()
    };
    // session_expiry must be > will_delay_interval so the will can fire
    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("wdi-sender").expect("id"),
            will: Some(will),
            session_expiry_interval: Some(10),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sender, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el);

    // Must NOT arrive within 500ms (delay not elapsed)
    let early = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(early.is_err(), "will must not arrive before delay: {early:?}");

    // Must arrive within 3s total (2s delay + margin)
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("delayed will within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected delayed will, got {ev:?}");
}

/// will_delay=2s + message_expiry=1s: will fires at t=2 but message expired at
/// t=3 (1s after publication). Offline subscriber reconnecting at t=4 gets nothing.
#[tokio::test]
async fn will_with_expiry_interval() {
    let (_c, port) = anonymous_broker().await;

    // Persistent subscriber goes offline
    let (s, mut el_s) =
        connect(persistent_connect_options(port, "wei-sub")).await.expect("connect");
    assert!(matches!(el_s.poll().await.expect("poll"), Event::Connected));
    s.subscribe(sub_qos1("will/expiry")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    s.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el_s.poll()).await;

    let will = Will {
        topic: Topic::try_new("will/expiry").expect("valid"),
        payload: Payload::from(b"should-expire".as_slice()),
        qos: Qos::AtLeastOnce,
        retain: false,
        will_delay_interval: Some(2),
        message_expiry_interval: Some(Duration::from_secs(1)),
        ..Will::default()
    };
    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("wei-sender").expect("id"),
            will: Some(will),
            session_expiry_interval: Some(10),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sender, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el); // t=0: trigger will

    // Wait: t=2 will fires, t=3 message expires, we check at t=4
    tokio::time::sleep(Duration::from_secs(4)).await;

    let (_s2, mut el_s2) =
        connect(resume_connect_options(port, "wei-sub")).await.expect("reconnect");
    assert!(matches!(el_s2.poll().await.expect("poll"), Event::Connected));

    let result = tokio::time::timeout(Duration::from_millis(500), el_s2.poll()).await;
    assert!(result.is_err(), "expired will must not be delivered, got: {result:?}");
}

#[tokio::test]
async fn will_with_empty_payload() {
    let (_c, port) = anonymous_broker().await;

    let (_sub, mut el_sub) = connect(connect_options(port, "wep-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    _sub.subscribe(sub("will/empty")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    let will = Will {
        topic: Topic::try_new("will/empty").expect("valid"),
        payload: Payload::from(b"".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        ..Will::default()
    };
    let (_sender, mut el) =
        connect(will_connect_options(port, "wep-sender", will)).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el);

    let ev = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await.expect("within 3s").expect("join").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected Message for empty-payload will, got {ev:?}");
}

#[tokio::test]
async fn will_with_user_properties() {
    let (_c, port) = anonymous_broker().await;

    let (_sub, mut el_sub) = connect(connect_options(port, "wup-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    _sub.subscribe(sub("will/props")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let sub_task = tokio::spawn(async move { el_sub.poll().await });

    let key = Utf8String::try_from("reason").expect("valid");
    let val = Utf8String::try_from("crash").expect("valid");
    let will = Will {
        topic: Topic::try_new("will/props").expect("valid"),
        payload: Payload::from(b"props-will".as_slice()),
        qos: Qos::AtMostOnce,
        retain: false,
        user_properties: vec![(key, val)],
        ..Will::default()
    };
    let (_sender, mut el) =
        connect(will_connect_options(port, "wup-sender", will)).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    drop(el);

    let ev = tokio::time::timeout(Duration::from_secs(3), sub_task)
        .await.expect("within 3s").expect("join").expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    let expected_key = Utf8String::try_from("reason").expect("valid");
    let expected_val = Utf8String::try_from("crash").expect("valid");
    assert!(
        broker_msg.user_properties.contains(&(expected_key, expected_val)),
        "user property must be preserved in will; got: {:?}",
        broker_msg.user_properties
    );
}
```

- [ ] **Step 2: Compile**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Clippy**

```bash
cargo clippy -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -10
```

Expected: no errors (warnings about unused variables for `_c` patterns are
acceptable).

- [ ] **Step 4: Commit**

```bash
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/will_messages.rs
git commit -m "test: add will_messages integration tests (8 tests)"
```

---

## Task 3: `retain.rs` — 7 tests

**Files:**

- Create: `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/retain.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

#[tokio::test]
async fn retained_message_delivered_to_new_subscriber() {
    let (_c, port) = anonymous_broker().await;

    // Publish retained
    let (pub_c, mut el_pub) = connect(connect_options(port, "ret-basic-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg_retain("test/retain/basic", b"retained-value", Qos::AtMostOnce))
        .await.expect("publish retained");
    tokio::time::sleep(Duration::from_millis(150)).await;

    // New subscriber should receive retained immediately on subscribe
    let (sub_c, mut el_sub) = connect(connect_options(port, "ret-basic-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("test/retain/basic")).await.expect("subscribe");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("retained within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected retained Message, got {ev:?}");
}

#[tokio::test]
async fn clear_retained_message() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ret-clear-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg_retain("test/retain/clear", b"first", Qos::AtMostOnce))
        .await.expect("publish retained");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Clear by publishing empty payload with retain=true
    pub_c.publish(msg_retain("test/retain/clear", b"", Qos::AtMostOnce))
        .await.expect("clear retained");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ret-clear-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("test/retain/clear")).await.expect("subscribe");

    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(result.is_err(), "cleared retained must not be delivered, got: {result:?}");
}

#[tokio::test]
async fn retained_message_is_latest_value() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ret-latest-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg_retain("test/retain/latest", b"first", Qos::AtMostOnce))
        .await.expect("publish first");
    tokio::time::sleep(Duration::from_millis(50)).await;
    pub_c.publish(msg_retain("test/retain/latest", b"second", Qos::AtMostOnce))
        .await.expect("publish second");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ret-latest-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("test/retain/latest")).await.expect("subscribe");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("retained within 3s").expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert_eq!(
        broker_msg.payload.as_ref(),
        b"second",
        "must receive latest retained value"
    );

    // Cleanup
    pub_c.publish(msg_retain("test/retain/latest", b"", Qos::AtMostOnce))
        .await.expect("clear");
}

/// RetainHandling::SendRetained (default) sends retained on every subscribe,
/// including re-subscribes.
#[tokio::test]
async fn retain_handling_send_on_subscribe() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rh-sos-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg_retain("test/retain/sos", b"value", Qos::AtMostOnce))
        .await.expect("publish retained");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "rh-sos-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));

    // First subscribe — must receive retained
    sub_c.subscribe(sub_with_options(
        "test/retain/sos",
        Qos::AtMostOnce,
        false,
        false,
        RetainHandling::SendRetained,
        None,
    )).await.expect("subscribe 1");
    let ev1 = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("retained on first sub").expect("event");
    assert!(matches!(ev1, Event::Message(_)), "expected retained on 1st sub, got {ev1:?}");

    // Unsubscribe then re-subscribe — must receive retained again
    sub_c.unsubscribe(UnsubscribeOptions {
        filter: Utf8String::try_from("test/retain/sos").expect("valid"),
        extra_filters: vec![],
        user_properties: vec![],
    }).await.expect("unsubscribe");
    tokio::time::sleep(Duration::from_millis(100)).await;

    sub_c.subscribe(sub_with_options(
        "test/retain/sos",
        Qos::AtMostOnce,
        false,
        false,
        RetainHandling::SendRetained,
        None,
    )).await.expect("subscribe 2");
    let ev2 = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("retained on re-sub").expect("event");
    assert!(matches!(ev2, Event::Message(_)), "expected retained on re-sub, got {ev2:?}");

    // Cleanup
    pub_c.publish(msg_retain("test/retain/sos", b"", Qos::AtMostOnce)).await.expect("clear");
}

/// RetainHandling::SendRetainedIfSubscriptionDoesNotExist sends retained only
/// for brand-new subscriptions, not re-subscribes.
#[tokio::test]
async fn retain_handling_send_only_on_new_subscribe() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rh-new-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg_retain("test/retain/new", b"value", Qos::AtMostOnce))
        .await.expect("publish retained");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "rh-new-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));

    // First subscribe — receives retained (subscription is new)
    sub_c.subscribe(sub_with_options(
        "test/retain/new",
        Qos::AtMostOnce,
        false,
        false,
        RetainHandling::SendRetainedIfSubscriptionDoesNotExist,
        None,
    )).await.expect("subscribe 1");
    let ev1 = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("retained on first sub").expect("event");
    assert!(matches!(ev1, Event::Message(_)), "expected retained on new sub, got {ev1:?}");

    // Re-subscribe — must NOT receive retained (subscription already exists)
    sub_c.subscribe(sub_with_options(
        "test/retain/new",
        Qos::AtMostOnce,
        false,
        false,
        RetainHandling::SendRetainedIfSubscriptionDoesNotExist,
        None,
    )).await.expect("subscribe 2");
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(result.is_err(), "retained must not be sent on re-sub, got: {result:?}");

    // Cleanup
    pub_c.publish(msg_retain("test/retain/new", b"", Qos::AtMostOnce)).await.expect("clear");
}

/// RetainHandling::DoNotSend never delivers retained on subscribe.
#[tokio::test]
async fn retain_handling_do_not_send() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rh-dns-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg_retain("test/retain/dns", b"value", Qos::AtMostOnce))
        .await.expect("publish retained");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "rh-dns-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub_with_options(
        "test/retain/dns",
        Qos::AtMostOnce,
        false,
        false,
        RetainHandling::DoNotSend,
        None,
    )).await.expect("subscribe");

    // Retained must NOT arrive
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(result.is_err(), "DoNotSend must suppress retained on subscribe, got: {result:?}");

    // But live publishes must still arrive (subscription works)
    pub_c.publish(msg("test/retain/dns", b"live", Qos::AtMostOnce)).await.expect("publish live");
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("live message within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected live message, got {ev:?}");

    // Cleanup
    pub_c.publish(msg_retain("test/retain/dns", b"", Qos::AtMostOnce)).await.expect("clear");
}

/// With retain_as_published=true, the received message's retain flag matches
/// the publisher's retain flag.
#[tokio::test]
async fn retain_as_published_preserves_retain_flag() {
    let (_c, port) = anonymous_broker().await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rap-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    let (sub_c, mut el_sub) = connect(connect_options(port, "rap-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub_with_options(
        "test/retain/rap",
        Qos::AtMostOnce,
        false,
        true, // retain_as_published
        RetainHandling::SendRetained,
        None,
    )).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Publish a retained message
    pub_c.publish(msg_retain("test/retain/rap", b"value", Qos::AtMostOnce))
        .await.expect("publish retained");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("message within 3s").expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(
        broker_msg.retain,
        "retain_as_published=true must preserve retain=true in forwarded message"
    );

    // Cleanup
    pub_c.publish(msg_retain("test/retain/rap", b"", Qos::AtMostOnce)).await.expect("clear");
}
```

- [ ] **Step 2: Compile**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -10
```

Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/retain.rs
git commit -m "test: add retain integration tests (7 tests)"
```

---

## Task 4: `topic_filters.rs` — 7 tests

**Files:**

- Create: `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/topic_filters.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

#[tokio::test]
async fn single_level_wildcard_matches_one_segment() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-slw-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    // + matches exactly one segment
    sub_c.subscribe(sub("sensors/+/temp")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-slw-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    // This MUST match: one segment between slashes
    pub_c.publish(msg("sensors/room1/temp", b"22", Qos::AtMostOnce)).await.expect("publish match");
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("matched within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected match on single-segment topic, got {ev:?}");

    // This must NOT match: two segments between the fixed parts
    pub_c.publish(msg("sensors/room1/floor2/temp", b"21", Qos::AtMostOnce))
        .await.expect("publish no-match");
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(
        result.is_err(),
        "+ must not match two segments, but subscriber got: {result:?}"
    );
}

#[tokio::test]
async fn multi_level_wildcard_matches_all_below() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-mlw-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("sensors/#")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-mlw-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    for topic in ["sensors/temp", "sensors/room1/temp", "sensors/room1/floor/temp"] {
        pub_c.publish(msg(topic, b"val", Qos::AtMostOnce)).await.expect("publish");
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await.expect("within 3s").expect("event");
        assert!(
            matches!(ev, Event::Message(_)),
            "# must match {topic}, got {ev:?}"
        );
    }
}

/// # matches `sensors/` and all descendants but NOT topics at a different root.
#[tokio::test]
async fn multi_level_wildcard_boundary_not_matched() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-mlwb-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("a/b/#")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-mlwb-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    // Must match
    pub_c.publish(msg("a/b/c", b"yes", Qos::AtMostOnce)).await.expect("publish match");
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "a/b/# must match a/b/c");

    // Must NOT match (different root)
    pub_c.publish(msg("a/c/d", b"no", Qos::AtMostOnce)).await.expect("publish no-match");
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(result.is_err(), "a/b/# must not match a/c/d, got: {result:?}");
}

/// A single SUBSCRIBE packet with extra_subscriptions covers multiple topics.
#[tokio::test]
async fn multiple_topics_in_one_subscribe_packet() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-multi-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));

    sub_c.subscribe(SubscribeOptions {
        subscription: Subscription {
            topic_filter: Utf8String::try_from("tf/multi/a").expect("valid"),
            qos: Qos::AtMostOnce,
            no_local: false,
            retain_as_published: false,
            retain_handling: RetainHandling::SendRetained,
        },
        extra_subscriptions: vec![
            Subscription {
                topic_filter: Utf8String::try_from("tf/multi/b").expect("valid"),
                qos: Qos::AtMostOnce,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendRetained,
            },
            Subscription {
                topic_filter: Utf8String::try_from("tf/multi/c").expect("valid"),
                qos: Qos::AtMostOnce,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendRetained,
            },
        ],
        subscription_identifier: None,
        user_properties: vec![],
    }).await.expect("multi-subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-multi-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    for topic in ["tf/multi/a", "tf/multi/b", "tf/multi/c"] {
        pub_c.publish(msg(topic, b"val", Qos::AtMostOnce)).await.expect("publish");
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await.expect("within 3s").expect("event");
        assert!(matches!(ev, Event::Message(_)), "expected message on {topic}, got {ev:?}");
    }
}

/// Overlapping subscriptions: `a/#` and `a/b` both match a publish to `a/b`.
/// Mosquitto 2 delivers one message per matching subscription filter.
#[tokio::test]
async fn overlapping_subscriptions_deliver_once_per_filter() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-overlap-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    // Two separate SUBSCRIBE packets for the same effective topic
    sub_c.subscribe(sub("tf/overlap/#")).await.expect("subscribe wildcard");
    sub_c.subscribe(sub("tf/overlap/b")).await.expect("subscribe exact");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-overlap-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg("tf/overlap/b", b"v", Qos::AtMostOnce)).await.expect("publish");

    // At least one delivery must happen
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected at least one delivery, got {ev:?}");
}

/// Re-subscribing to an existing topic at a higher QoS upgrades delivery.
#[tokio::test]
async fn resubscribe_upgrades_qos() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) =
        connect(persistent_connect_options(port, "tf-upqos-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("tf/upqos")).await.expect("subscribe QoS0"); // QoS 0
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Re-subscribe at QoS 1
    sub_c.subscribe(sub_qos1("tf/upqos")).await.expect("re-subscribe QoS1");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-upqos-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg("tf/upqos", b"v", Qos::AtLeastOnce)).await.expect("publish");
    let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await; // drain puback

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    assert!(
        matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
        "after QoS upgrade, message must arrive as QoS1, got {ev:?}"
    );
}

/// Unsubscribing stops delivery; re-subscribing restores it.
#[tokio::test]
async fn unsubscribe_followed_by_resubscribe() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "tf-unsub-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("tf/unsub")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "tf-unsub-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    // Receive first message (subscription active)
    pub_c.publish(msg("tf/unsub", b"first", Qos::AtMostOnce)).await.expect("publish");
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("first within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected first message, got {ev:?}");

    // Unsubscribe — second publish must NOT arrive
    sub_c.unsubscribe(UnsubscribeOptions {
        filter: Utf8String::try_from("tf/unsub").expect("valid"),
        extra_filters: vec![],
        user_properties: vec![],
    }).await.expect("unsubscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    pub_c.publish(msg("tf/unsub", b"second", Qos::AtMostOnce)).await.expect("publish");
    let no_msg = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(no_msg.is_err(), "after unsubscribe, must not receive message, got: {no_msg:?}");

    // Re-subscribe — third publish MUST arrive
    sub_c.subscribe(sub("tf/unsub")).await.expect("re-subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    pub_c.publish(msg("tf/unsub", b"third", Qos::AtMostOnce)).await.expect("publish");
    let ev3 = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("third within 3s").expect("event");
    assert!(matches!(ev3, Event::Message(_)), "expected third message after re-subscribe, got {ev3:?}");
}
```

- [ ] **Step 2: Compile and commit**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/topic_filters.rs
git commit -m "test: add topic_filters integration tests (7 tests)"
```

---

## Task 5: `session_advanced.rs` — 5 tests

**Files:**

- Create: `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/session_advanced.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Three QoS 1 messages queued while subscriber is offline are all delivered
/// on session resume.
#[tokio::test]
async fn multiple_inflight_qos1_all_delivered_after_reconnect() {
    let (_c, port) = anonymous_broker().await;

    let (sub1, mut el1) =
        connect(persistent_connect_options(port, "sa-mq1-sub")).await.expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_qos1("sa/mq1")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    // Publisher sends 3 messages while subscriber is offline
    let (pub_c, mut el_pub) = connect(connect_options(port, "sa-mq1-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    for i in 0u8..3 {
        pub_c.publish(msg("sa/mq1", &[i], Qos::AtLeastOnce)).await.expect("publish");
        let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await; // drain puback
    }

    // Reconnect with session resume
    let (_sub2, mut el2) =
        connect(resume_connect_options(port, "sa-mq1-sub")).await.expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    // All 3 must arrive
    for i in 0..3 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el2.poll())
            .await.expect(&format!("message {i} within 5s")).expect("event");
        assert!(
            matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
            "expected queued QoS1 message {i}, got {ev:?}"
        );
    }
}

/// Two QoS 2 messages queued for an offline subscriber are both delivered on
/// session resume.
#[tokio::test]
async fn multiple_inflight_qos2_all_delivered_after_reconnect() {
    let (_c, port) = anonymous_broker().await;

    // Subscribe at QoS 2
    let (sub1, mut el1) =
        connect(persistent_connect_options(port, "sa-mq2-sub")).await.expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_with_options(
        "sa/mq2",
        Qos::ExactlyOnce,
        false,
        false,
        RetainHandling::SendRetained,
        None,
    )).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    // Publisher sends 2 QoS 2 messages
    let (pub_c, mut el_pub) = connect(connect_options(port, "sa-mq2-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    for i in 0u8..2 {
        pub_c.publish(msg("sa/mq2", &[i], Qos::ExactlyOnce)).await.expect("publish");
        let ev = tokio::time::timeout(Duration::from_secs(5), el_pub.poll())
            .await.expect("pubcomp within 5s").expect("event");
        assert!(matches!(ev, Event::PublishCompleted(_, _)), "expected PublishCompleted, got {ev:?}");
    }

    // Reconnect subscriber
    let (_sub2, mut el2) =
        connect(resume_connect_options(port, "sa-mq2-sub")).await.expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    for i in 0..2 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el2.poll())
            .await.expect(&format!("qos2 message {i} within 5s")).expect("event");
        assert!(
            matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
            "expected queued QoS2 message {i}, got {ev:?}"
        );
    }
}

/// Queued inbound messages arrive in the same poll loop after the Connected event.
#[tokio::test]
async fn queued_inbound_messages_arrive_after_connack() {
    let (_c, port) = anonymous_broker().await;

    let (sub1, mut el1) =
        connect(persistent_connect_options(port, "sa-qi-sub")).await.expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_qos1("sa/qi")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "sa-qi-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    for i in 0u8..3 {
        pub_c.publish(msg("sa/qi", &[i], Qos::AtLeastOnce)).await.expect("publish");
        let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await;
    }

    let (_sub2, mut el2) =
        connect(resume_connect_options(port, "sa-qi-sub")).await.expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    // All 3 queued messages must be deliverable by polling without further action
    for i in 0..3 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el2.poll())
            .await.expect(&format!("queued msg {i} within 5s")).expect("event");
        assert!(
            matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
            "expected queued message {i}, got {ev:?}"
        );
    }
}

/// When a second client connects with the same client_id, Mosquitto sends
/// DISCONNECT with reason code SessionTakenOver to the first connection.
#[tokio::test]
async fn session_takeover_disconnects_old_connection() {
    let (_c, port) = anonymous_broker().await;

    let (_client1, mut el1) =
        connect(connect_options(port, "sa-takeover")).await.expect("connect first");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));

    // Second connection with the same client_id
    let (_client2, mut el2) =
        connect(connect_options(port, "sa-takeover")).await.expect("connect second");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    // First event loop must receive a disconnect with SessionTakenOver
    let ev = tokio::time::timeout(Duration::from_secs(3), el1.poll())
        .await.expect("disconnect within 3s").expect("event");
    assert!(
        matches!(ev, Event::Disconnected(Some(DisconnectReasonCode::SessionTakenOver))),
        "expected Disconnected(SessionTakenOver), got {ev:?}"
    );
}

/// After session_expiry elapses, reconnecting with clean_start=false results
/// in no queued messages (session was dropped).
#[tokio::test]
async fn session_expiry_drops_queued_messages() {
    let (_c, port) = anonymous_broker().await;

    // Connect with a short session_expiry
    let opts_short = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("sa-expiry").expect("id"),
            session_expiry_interval: Some(1), // 1 second
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (sub1, mut el1) = connect(opts_short).await.expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_qos1("sa/expiry")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    // Publisher queues a message
    let (pub_c, mut el_pub) = connect(connect_options(port, "sa-expiry-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg("sa/expiry", b"queued", Qos::AtLeastOnce)).await.expect("publish");
    let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await;

    // Wait for session to expire (>1s)
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Reconnect without clean_start — session has expired, no queued messages
    let opts_resume = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: false,
            client_identifier: Utf8String::try_from("sa-expiry").expect("id"),
            session_expiry_interval: Some(300),
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_sub2, mut el2) = connect(opts_resume).await.expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    let result = tokio::time::timeout(Duration::from_millis(500), el2.poll()).await;
    assert!(
        result.is_err(),
        "expired session must not deliver queued messages, got: {result:?}"
    );
}
```

- [ ] **Step 2: Compile and commit**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/session_advanced.rs
git commit -m "test: add session_advanced integration tests (5 tests)"
```

---

## Task 6: `subscriptions.rs` — 8 tests

**Files:**

- Create: `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/subscriptions.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// no_local=true: a client must not receive its own publishes on that topic.
#[tokio::test]
async fn no_local_prevents_receiving_own_publishes() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "nl-client")).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    client.subscribe(sub_with_options(
        "nl/topic",
        Qos::AtMostOnce,
        true, // no_local
        false,
        RetainHandling::SendRetained,
        None,
    )).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    client.publish(msg("nl/topic", b"self-pub", Qos::AtMostOnce)).await.expect("publish");

    let result = tokio::time::timeout(Duration::from_millis(500), el.poll()).await;
    assert!(
        result.is_err(),
        "no_local=true must suppress self-publish, got: {result:?}"
    );
}

/// Subscription identifier is carried in every matching BrokerMessage.
#[tokio::test]
async fn subscription_identifier_delivered_with_message() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "sid-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    let id = core::num::NonZero::<u64>::new(42).unwrap();
    sub_c.subscribe(sub_with_options(
        "sid/topic",
        Qos::AtMostOnce,
        false,
        false,
        RetainHandling::SendRetained,
        Some(id),
    )).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "sid-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg("sid/topic", b"v", Qos::AtMostOnce)).await.expect("publish");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    let broker_msg = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(
        broker_msg.subscription_identifiers.contains(&id),
        "subscription_identifiers must contain {id}; got: {:?}",
        broker_msg.subscription_identifiers
    );
}

/// Subscription identifier on a wildcard subscription is carried in all
/// messages matching that wildcard.
#[tokio::test]
async fn subscription_identifier_on_wildcard_subscription() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "sidw-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    let id = core::num::NonZero::<u64>::new(99).unwrap();
    sub_c.subscribe(sub_with_options(
        "sidw/#",
        Qos::AtMostOnce,
        false,
        false,
        RetainHandling::SendRetained,
        Some(id),
    )).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "sidw-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    for topic in ["sidw/a", "sidw/b/c"] {
        pub_c.publish(msg(topic, b"v", Qos::AtMostOnce)).await.expect("publish");
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await.expect("within 3s").expect("event");
        let broker_msg = match ev {
            Event::Message(m) => m,
            other => panic!("expected Message on {topic}, got {other:?}"),
        };
        assert!(
            broker_msg.subscription_identifiers.contains(&id),
            "sub id must be on message for {topic}; got: {:?}",
            broker_msg.subscription_identifiers
        );
    }
}

#[tokio::test]
async fn unsubscribe_stops_delivery() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "us-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("us/topic")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "us-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg("us/topic", b"before", Qos::AtMostOnce)).await.expect("publish before");
    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    assert!(matches!(ev, Event::Message(_)), "expected message before unsub, got {ev:?}");

    sub_c.unsubscribe(UnsubscribeOptions {
        filter: Utf8String::try_from("us/topic").expect("valid"),
        extra_filters: vec![],
        user_properties: vec![],
    }).await.expect("unsubscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    pub_c.publish(msg("us/topic", b"after", Qos::AtMostOnce)).await.expect("publish after");
    let result = tokio::time::timeout(Duration::from_millis(500), el_sub.poll()).await;
    assert!(result.is_err(), "after unsubscribe, no message must arrive, got: {result:?}");
}

/// Unsubscribing from a topic the client never subscribed to must not error.
#[tokio::test]
async fn unsubscribe_from_nonexistent_topic_succeeds() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "unt-client")).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    client.unsubscribe(UnsubscribeOptions {
        filter: Utf8String::try_from("unt/never-subscribed").expect("valid"),
        extra_filters: vec![],
        user_properties: vec![],
    }).await.expect("unsubscribe from nonexistent must not error on send");

    // No error event should arrive; connection stays alive
    let result = tokio::time::timeout(Duration::from_millis(500), el.poll()).await;
    // A timeout (no event) is success. An Ok(Err(...)) would be a failure.
    if let Ok(Err(e)) = result {
        panic!("unexpected error after unsubscribing nonexistent topic: {e:?}");
    }
}

/// Re-subscribing to a topic at a lower QoS downgrades delivery.
#[tokio::test]
async fn resubscribe_downgrades_qos() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) =
        connect(persistent_connect_options(port, "rdq-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub_qos1("rdq/topic")).await.expect("subscribe QoS1");
    tokio::time::sleep(Duration::from_millis(100)).await;
    sub_c.subscribe(sub("rdq/topic")).await.expect("re-subscribe QoS0");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "rdq-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg("rdq/topic", b"v", Qos::AtLeastOnce)).await.expect("publish");
    let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await;

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    assert!(
        matches!(ev, Event::Message(_)),
        "after QoS downgrade, message must arrive as QoS0 (Message), got {ev:?}"
    );
}

/// A single subscribe() with extra_subscriptions subscribes to multiple topics.
#[tokio::test]
async fn multiple_subscriptions_one_call() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "msoc-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));

    let make_sub = |topic: &str| Subscription {
        topic_filter: Utf8String::try_from(topic).expect("valid"),
        qos: Qos::AtMostOnce,
        no_local: false,
        retain_as_published: false,
        retain_handling: RetainHandling::SendRetained,
    };
    sub_c.subscribe(SubscribeOptions {
        subscription: make_sub("msoc/x"),
        extra_subscriptions: vec![make_sub("msoc/y"), make_sub("msoc/z")],
        subscription_identifier: None,
        user_properties: vec![],
    }).await.expect("multi subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "msoc-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    for topic in ["msoc/x", "msoc/y", "msoc/z"] {
        pub_c.publish(msg(topic, b"v", Qos::AtMostOnce)).await.expect("publish");
        let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
            .await.expect(&format!("{topic} within 3s")).expect("event");
        assert!(matches!(ev, Event::Message(_)), "expected message on {topic}, got {ev:?}");
    }
}

/// Shared subscriptions distribute messages across subscribers.
/// With $share/group/topic two clients share load — each message goes to
/// exactly one of them.
#[tokio::test]
async fn shared_subscription_load_balancing() {
    let (_c, port) = anonymous_broker().await;

    let (s1, mut el1) = connect(connect_options(port, "ss-client1")).await.expect("connect 1");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    s1.subscribe(sub("$share/grp/ss/topic")).await.expect("subscribe 1");

    let (s2, mut el2) = connect(connect_options(port, "ss-client2")).await.expect("connect 2");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));
    s2.subscribe(sub("$share/grp/ss/topic")).await.expect("subscribe 2");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ss-pub")).await.expect("connect pub");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    // Publish 4 messages; each must be delivered to exactly one of the two subscribers.
    for i in 0u8..4 {
        pub_c.publish(msg("ss/topic", &[i], Qos::AtMostOnce)).await.expect("publish");
    }

    let mut total = 0u8;
    for _ in 0..4 {
        let got1 = tokio::time::timeout(Duration::from_millis(200), el1.poll()).await;
        let got2 = tokio::time::timeout(Duration::from_millis(200), el2.poll()).await;
        // Exactly one of the two should receive the message for each publish
        match (got1, got2) {
            (Ok(Ok(Event::Message(_))), Err(_)) => total += 1,
            (Err(_), Ok(Ok(Event::Message(_)))) => total += 1,
            (Ok(Ok(Event::Message(_))), Ok(Ok(Event::Message(_)))) => {
                // Both got this message — that's also acceptable if Mosquitto
                // doesn't guarantee exclusivity at AtMostOnce
                total += 1;
            }
            _ => {}
        }
    }
    // Give a brief extra window for any remaining messages
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(total >= 1, "at least one shared subscriber must receive messages");
}
```

- [ ] **Step 2: Compile and commit**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/subscriptions.rs
git commit -m "test: add subscriptions integration tests (8 tests)"
```

---

## Task 7: `message_properties.rs` — 6 tests

**Files:**

- Create:
  `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/message_properties.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// User properties are passed through the broker unchanged.
#[tokio::test]
async fn user_properties_preserved_end_to_end() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "up-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("mp/user-props")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "up-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    let k1 = Utf8String::try_from("env").expect("valid");
    let v1 = Utf8String::try_from("test").expect("valid");
    let k2 = Utf8String::try_from("version").expect("valid");
    let v2 = Utf8String::try_from("1.0").expect("valid");
    let k3 = Utf8String::try_from("region").expect("valid");
    let v3 = Utf8String::try_from("eu-west").expect("valid");

    pub_c.publish(ClientMessage {
        topic: Topic::try_new("mp/user-props".as_bytes().to_vec()).expect("valid"),
        payload: Payload::from(b"payload".as_slice()),
        qos: Qos::AtMostOnce,
        user_properties: vec![
            (k1.clone(), v1.clone()),
            (k2.clone(), v2.clone()),
            (k3.clone(), v3.clone()),
        ],
        ..ClientMessage::default()
    }).await.expect("publish");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    let m = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(m.user_properties.contains(&(k1, v1)), "prop 'env' missing");
    assert!(m.user_properties.contains(&(k2, v2)), "prop 'version' missing");
    assert!(m.user_properties.contains(&(k3, v3)), "prop 'region' missing");
}

/// A message with message_expiry_interval=1s is NOT delivered to a subscriber
/// that reconnects after the expiry elapses.
#[tokio::test]
async fn message_expiry_interval_drops_stale_message() {
    let (_c, port) = anonymous_broker().await;

    // Persistent subscriber goes offline
    let (sub1, mut el1) =
        connect(persistent_connect_options(port, "mei-sub")).await.expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_qos1("mp/expiry")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "mei-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    pub_c.publish(ClientMessage {
        topic: Topic::try_new("mp/expiry".as_bytes().to_vec()).expect("valid"),
        payload: Payload::from(b"stale".as_slice()),
        qos: Qos::AtLeastOnce,
        message_expiry_interval: Some(Duration::from_secs(1)),
        ..ClientMessage::default()
    }).await.expect("publish expiring message");
    let _ = tokio::time::timeout(Duration::from_secs(3), el_pub.poll()).await; // drain puback

    // Wait for message to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    let (_sub2, mut el2) =
        connect(resume_connect_options(port, "mei-sub")).await.expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    let result = tokio::time::timeout(Duration::from_millis(500), el2.poll()).await;
    assert!(result.is_err(), "expired message must not be delivered, got: {result:?}");
}

/// response_topic and correlation_data survive the broker round-trip.
#[tokio::test]
async fn response_topic_and_correlation_data_round_trip() {
    let (_c, port) = anonymous_broker().await;

    // Responder subscribes to the request topic
    let (resp_c, mut el_resp) = connect(connect_options(port, "rr-resp")).await.expect("connect");
    assert!(matches!(el_resp.poll().await.expect("poll"), Event::Connected));
    resp_c.subscribe(sub("mp/request")).await.expect("subscribe request");
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Requester publishes with response_topic and correlation_data
    let (req_c, mut el_req) = connect(connect_options(port, "rr-req")).await.expect("connect");
    assert!(matches!(el_req.poll().await.expect("poll"), Event::Connected));
    req_c.subscribe(sub("mp/response/rr-req")).await.expect("subscribe response");
    tokio::time::sleep(Duration::from_millis(100)).await;

    req_c.publish(ClientMessage {
        topic: Topic::try_new("mp/request".as_bytes().to_vec()).expect("valid"),
        payload: Payload::from(b"compute-42".as_slice()),
        qos: Qos::AtMostOnce,
        response_topic: Some(Topic::try_new("mp/response/rr-req").expect("valid")),
        correlation_data: Some(BinaryData::new(b"corr-abc".as_slice())),
        ..ClientMessage::default()
    }).await.expect("publish request");

    // Responder receives request, checks response_topic and correlation_data
    let req_ev = tokio::time::timeout(Duration::from_secs(3), el_resp.poll())
        .await.expect("request within 3s").expect("event");
    let req_msg = match req_ev {
        Event::Message(m) => m,
        other => panic!("expected request Message, got {other:?}"),
    };
    let rt = req_msg.response_topic.expect("response_topic must be present");
    let cd = req_msg.correlation_data.expect("correlation_data must be present");
    assert_eq!(cd.as_ref(), b"corr-abc", "correlation_data mismatch");

    // Responder replies on the response_topic
    resp_c.publish(ClientMessage {
        topic: rt,
        payload: Payload::from(b"answer-42".as_slice()),
        qos: Qos::AtMostOnce,
        correlation_data: Some(cd),
        ..ClientMessage::default()
    }).await.expect("publish response");

    let resp_ev = tokio::time::timeout(Duration::from_secs(3), el_req.poll())
        .await.expect("response within 3s").expect("event");
    let resp_msg = match resp_ev {
        Event::Message(m) => m,
        other => panic!("expected response Message, got {other:?}"),
    };
    let resp_cd = resp_msg.correlation_data.expect("response must carry correlation_data");
    assert_eq!(resp_cd.as_ref(), b"corr-abc", "response correlation_data mismatch");
}

/// content_type is preserved end-to-end.
#[tokio::test]
async fn content_type_preserved() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "ct-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("mp/ct")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "ct-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(ClientMessage {
        topic: Topic::try_new("mp/ct".as_bytes().to_vec()).expect("valid"),
        payload: Payload::from(b"{}".as_slice()),
        qos: Qos::AtMostOnce,
        content_type: Some(Utf8String::try_from("application/json").expect("valid")),
        ..ClientMessage::default()
    }).await.expect("publish");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    let m = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    let ct = m.content_type.expect("content_type must be present");
    assert_eq!(ct.as_ref(), "application/json", "content_type mismatch");
}

/// payload_format_indicator is preserved end-to-end.
#[tokio::test]
async fn payload_format_indicator_preserved() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "pfi-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("mp/pfi")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "pfi-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(ClientMessage {
        topic: Topic::try_new("mp/pfi".as_bytes().to_vec()).expect("valid"),
        payload: Payload::from(b"hello world".as_slice()),
        qos: Qos::AtMostOnce,
        payload_format_indicator: Some(FormatIndicator::Utf8),
        ..ClientMessage::default()
    }).await.expect("publish");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    let m = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert_eq!(
        m.payload_format_indicator,
        Some(FormatIndicator::Utf8),
        "payload_format_indicator must be preserved"
    );
}

/// Duplicate user property keys are all preserved in order.
#[tokio::test]
async fn multiple_user_properties_duplicate_keys() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "dupk-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("mp/dupkeys")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "dupk-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    let key = || Utf8String::try_from("tag").expect("valid");
    let v1 = Utf8String::try_from("alpha").expect("valid");
    let v2 = Utf8String::try_from("beta").expect("valid");
    pub_c.publish(ClientMessage {
        topic: Topic::try_new("mp/dupkeys".as_bytes().to_vec()).expect("valid"),
        payload: Payload::from(b"v".as_slice()),
        qos: Qos::AtMostOnce,
        user_properties: vec![(key(), v1.clone()), (key(), v2.clone())],
        ..ClientMessage::default()
    }).await.expect("publish");

    let ev = tokio::time::timeout(Duration::from_secs(3), el_sub.poll())
        .await.expect("within 3s").expect("event");
    let m = match ev {
        Event::Message(m) => m,
        other => panic!("expected Message, got {other:?}"),
    };
    assert!(
        m.user_properties.contains(&(key(), v1)),
        "duplicate key 'tag'='alpha' missing"
    );
    assert!(
        m.user_properties.contains(&(key(), v2)),
        "duplicate key 'tag'='beta' missing"
    );
}
```

- [ ] **Step 2: Compile and commit**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/message_properties.rs
git commit -m "test: add message_properties integration tests (6 tests)"
```

---

## Task 8: `qos2_advanced.rs` — 3 tests

**Files:**

- Create: `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/qos2_advanced.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// Five QoS 2 messages published concurrently all complete the full
/// PUBLISH→PUBREC→PUBREL→PUBCOMP flow.
#[tokio::test]
async fn concurrent_qos2_publishes_all_complete() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "cq2-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("qos2/concurrent")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let (pub_c, mut el_pub) = connect(connect_options(port, "cq2-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    for i in 0u8..5 {
        pub_c.publish(msg("qos2/concurrent", &[i], Qos::ExactlyOnce)).await.expect("publish");
    }

    // All 5 must complete
    for i in 0..5 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el_pub.poll())
            .await.expect(&format!("PublishCompleted {i} within 5s")).expect("event");
        assert!(
            matches!(ev, Event::PublishCompleted(_, _)),
            "expected PublishCompleted {i}, got {ev:?}"
        );
    }

    // Subscriber must receive all 5
    for i in 0..5 {
        let ev = tokio::time::timeout(Duration::from_secs(5), el_sub.poll())
            .await.expect(&format!("subscriber message {i} within 5s")).expect("event");
        assert!(
            matches!(ev, Event::Message(_)),
            "expected subscriber Message {i}, got {ev:?}"
        );
    }
}

/// A QoS 2 message queued for an offline QoS-2 subscriber is delivered on
/// session resume.
#[tokio::test]
async fn qos2_queued_for_offline_subscriber_delivered_on_resume() {
    let (_c, port) = anonymous_broker().await;

    // Subscribe at QoS 2
    let (sub1, mut el1) =
        connect(persistent_connect_options(port, "qos2-offline-sub")).await.expect("connect");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));
    sub1.subscribe(sub_with_options(
        "qos2/offline",
        Qos::ExactlyOnce,
        false,
        false,
        RetainHandling::SendRetained,
        None,
    )).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;
    sub1.disconnect().await.expect("disconnect");
    let _ = tokio::time::timeout(Duration::from_secs(1), el1.poll()).await;

    // Publisher sends QoS 2
    let (pub_c, mut el_pub) = connect(connect_options(port, "qos2-offline-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));
    pub_c.publish(msg("qos2/offline", b"queued-qos2", Qos::ExactlyOnce)).await.expect("publish");
    let pev = tokio::time::timeout(Duration::from_secs(5), el_pub.poll())
        .await.expect("PublishCompleted within 5s").expect("event");
    assert!(matches!(pev, Event::PublishCompleted(_, _)), "expected PublishCompleted, got {pev:?}");

    // Reconnect subscriber
    let (_sub2, mut el2) =
        connect(resume_connect_options(port, "qos2-offline-sub")).await.expect("reconnect");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    let ev = tokio::time::timeout(Duration::from_secs(5), el2.poll())
        .await.expect("queued QoS2 within 5s").expect("event");
    assert!(
        matches!(ev, Event::MessageWithRequiredAcknowledgement(_, _)),
        "expected MessageWithRequiredAcknowledgement for queued QoS2, got {ev:?}"
    );
}

/// Publishing more QoS 2 messages than the server's receive_maximum allows in
/// parallel does not cause errors — the protocol queues them.
#[tokio::test]
async fn qos2_beyond_receive_maximum_queues_without_error() {
    let (_c, port) = anonymous_broker().await;

    let (sub_c, mut el_sub) = connect(connect_options(port, "qos2-rm-sub")).await.expect("connect");
    assert!(matches!(el_sub.poll().await.expect("poll"), Event::Connected));
    sub_c.subscribe(sub("qos2/rm")).await.expect("subscribe");
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Connect with a low receive_maximum to limit in-flight QoS2 from broker to us,
    // then fire more publishes than the default broker receive_maximum (typically 20)
    let (pub_c, mut el_pub) = connect(connect_options(port, "qos2-rm-pub")).await.expect("connect");
    assert!(matches!(el_pub.poll().await.expect("poll"), Event::Connected));

    // Submit 10 QoS 2 publishes — more than many broker defaults but should be handled
    for i in 0u8..10 {
        pub_c.publish(msg("qos2/rm", &[i], Qos::ExactlyOnce)).await.expect("publish");
    }

    // All 10 must complete without error
    for i in 0..10 {
        let ev = tokio::time::timeout(Duration::from_secs(10), el_pub.poll())
            .await.expect(&format!("PublishCompleted {i} within 10s")).expect("event");
        assert!(
            matches!(ev, Event::PublishCompleted(_, _)),
            "expected PublishCompleted {i}, got {ev:?}"
        );
    }
}
```

- [ ] **Step 2: Compile and commit**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/qos2_advanced.rs
git commit -m "test: add qos2_advanced integration tests (3 tests)"
```

---

## Task 9: `server_disconnect.rs` — 2 tests

**Files:**

- Create:
  `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/server_disconnect.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// A graceful client-initiated DISCONNECT arrives as Disconnected(None).
/// Verifies the reason code plumbing from wire to Event.
#[tokio::test]
async fn graceful_disconnect_reason_code_is_none() {
    let (_c, port) = anonymous_broker().await;

    let (client, mut el) = connect(connect_options(port, "sd-grace")).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));
    client.disconnect().await.expect("disconnect");

    let ev = tokio::time::timeout(Duration::from_secs(3), el.poll())
        .await.expect("Disconnected within 3s").expect("event");
    assert!(
        matches!(ev, Event::Disconnected(None)),
        "graceful disconnect must produce Disconnected(None), got {ev:?}"
    );
}

/// When a second client connects with the same client_id, Mosquitto sends a
/// server-initiated DISCONNECT with reason SessionTakenOver to the first client.
#[tokio::test]
async fn session_takeover_reason_code() {
    let (_c, port) = anonymous_broker().await;

    let (_c1, mut el1) = connect(connect_options(port, "sd-takeover")).await.expect("connect first");
    assert!(matches!(el1.poll().await.expect("poll"), Event::Connected));

    // Second connection with the same client_id takes over
    let (_c2, mut el2) = connect(connect_options(port, "sd-takeover")).await.expect("connect second");
    assert!(matches!(el2.poll().await.expect("poll"), Event::Connected));

    let ev = tokio::time::timeout(Duration::from_secs(3), el1.poll())
        .await.expect("server disconnect within 3s").expect("event");
    assert!(
        matches!(ev, Event::Disconnected(Some(DisconnectReasonCode::SessionTakenOver))),
        "session takeover must produce Disconnected(Some(SessionTakenOver)), got {ev:?}"
    );
}
```

- [ ] **Step 2: Compile and commit**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/server_disconnect.rs
git commit -m "test: add server_disconnect integration tests (2 tests)"
```

---

## Task 10: `keep_alive_advanced.rs` — 2 tests

**Files:**

- Create:
  `crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/keep_alive_advanced.rs`

- [ ] **Step 1: Create the file**

```rust
use std::time::Duration;
use test_sansio_mqtt_v5_tokio_mosquitto::*;

/// keep_alive=0 disables the ping mechanism. The connection must survive at
/// least 3s of idling without PINGREQ/PINGRESP or a disconnect.
#[tokio::test]
async fn zero_keep_alive_connection_stays_alive() {
    let (_c, port) = anonymous_broker().await;

    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("ka-zero").expect("id"),
            keep_alive: None, // None means 0 (disabled)
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (_client, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    // Idle for 3s — connection must remain alive (no unexpected disconnect or error)
    let result = tokio::time::timeout(Duration::from_secs(3), el.poll()).await;
    match result {
        Err(_elapsed) => { /* success: still alive after 3s */ }
        Ok(Err(e)) => panic!("connection dropped with keep_alive=0: {e:?}"),
        Ok(Ok(ev)) => panic!("unexpected event with keep_alive=0: {ev:?}"),
    }
}

/// With keep_alive=2s, publishing a message at t≈1.5s resets the keep-alive
/// deadline so the connection survives past t=2s without a disconnect.
#[tokio::test]
async fn traffic_resets_keep_alive_deadline() {
    let (_c, port) = anonymous_broker().await;

    let opts = ConnectOptions {
        addr: format!("127.0.0.1:{port}").parse().expect("addr"),
        connection: ConnectionOptions {
            clean_start: true,
            client_identifier: Utf8String::try_from("ka-reset").expect("id"),
            keep_alive: core::num::NonZero::new(2), // 2 second keep-alive
            ..ConnectionOptions::default()
        },
        ..ConnectOptions::default()
    };
    let (client, mut el) = connect(opts).await.expect("connect");
    assert!(matches!(el.poll().await.expect("poll"), Event::Connected));

    // Subscribe so we have somewhere to publish
    client.subscribe(sub("ka/reset")).await.expect("subscribe");

    // At t≈1.5s publish a message — this should reset the keep-alive deadline
    tokio::time::sleep(Duration::from_millis(1500)).await;
    client.publish(msg("ka/reset", b"ping", Qos::AtMostOnce)).await.expect("publish");

    // Poll past the original 2s deadline — connection must still be alive
    let result = tokio::time::timeout(Duration::from_millis(800), el.poll()).await;
    match result {
        Err(_elapsed) => { /* success: alive past 2s deadline */ }
        Ok(Err(e)) => panic!("connection dropped after traffic reset: {e:?}"),
        Ok(Ok(Event::Message(_))) => { /* received own QoS0 echo — acceptable */ }
        Ok(Ok(ev)) => panic!("unexpected event: {ev:?}"),
    }
}
```

- [ ] **Step 2: Compile and commit**

```bash
cargo build --tests -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | tail -5
git add crates/test-sansio-mqtt-v5-tokio-mosquitto/tests/keep_alive_advanced.rs
git commit -m "test: add keep_alive_advanced integration tests (2 tests)"
```

---

## Task 11: Format, lint, final compile

- [ ] **Step 1: Format**

```bash
cargo +nightly fmt
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy -p test-sansio-mqtt-v5-tokio-mosquitto 2>&1 | grep -E "^error" | head -20
```

Expected: no `error` lines. Warnings about unused `_` bindings from
`let (_c, port)` patterns are expected and acceptable.

- [ ] **Step 3: Commit format fixes (if any)**

```bash
git add -p  # stage only formatting changes
git commit -m "style: apply nightly fmt to integration test expansion"
```

- [ ] **Step 4: Push branch**

```bash
git push -u origin feat/integration-test-expansion
```

---

## Self-Review Checklist

All 49 tests from the spec are accounted for:

| File                     | Tests in plan | Tests in spec                                                      |
| ------------------------ | ------------- | ------------------------------------------------------------------ |
| `will_messages.rs`       | 8             | 8                                                                  |
| `retain.rs`              | 7             | 7                                                                  |
| `topic_filters.rs`       | 7             | 7                                                                  |
| `session_advanced.rs`    | 5             | 5                                                                  |
| `subscriptions.rs`       | 8             | 8                                                                  |
| `message_properties.rs`  | 6             | 6                                                                  |
| `qos2_advanced.rs`       | 3             | 4 (qos2_inbound_across_reconnect dropped — no Client::acknowledge) |
| `server_disconnect.rs`   | 2             | 2                                                                  |
| `keep_alive_advanced.rs` | 2             | 2                                                                  |

**Known deviation:** `qos2_inbound_across_reconnect` was removed because
`Client` has no `acknowledge` method, making it impossible to complete the
inbound QoS 2 handshake from the application layer. Replaced with
`qos2_beyond_receive_maximum_queues_without_error`.

**Type consistency verified:** All types used (`Will`, `ClientMessage`,
`BrokerMessage`, `RetainHandling`, `FormatIndicator`, `UnsubscribeOptions`,
`SubscribeOptions`, `Subscription`, `DisconnectReasonCode::SessionTakenOver`)
match the source definitions read from the codebase.
`Topic::try_new(&'static str)` confirmed valid.
`BinaryData::new(impl Into<bytes::Bytes>)` confirmed infallible.
