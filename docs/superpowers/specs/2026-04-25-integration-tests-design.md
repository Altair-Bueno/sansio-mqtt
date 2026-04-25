# Integration Tests Design — sansio-mqtt-v5-tokio

**Date:** 2026-04-25  
**Scope:** `crates/sansio-mqtt-v5-tokio`

---

## Overview

Add integration tests to `sansio-mqtt-v5-tokio` that run the client against a
real Mosquitto MQTT v5.0 broker managed by testcontainers. Tests are
feature-gated to avoid running on every `cargo test` invocation.

---

## Broker

**Eclipse Mosquitto 2** (`eclipse-mosquitto:2` image). Chosen for sub-second
startup, first-class MQTT v5.0 support, and status as the reference broker for
MQTT library testing.

---

## Feature Gate

A new Cargo feature `integration-tests` controls compilation and execution of
the test binary. Without it, `cargo test -p sansio-mqtt-v5-tokio` is unaffected.

```toml
[features]
integration-tests = []

[[test]]
name = "mqtt_integration"
path = "tests/integration/main.rs"
required-features = ["integration-tests"]
```

`testcontainers` is added as a dev-dependency (always compiled as a dev dep, but
only linked into the test binary when the feature is enabled).

---

## Container Runtime

testcontainers-rs communicates with the container daemon via its API socket, not
by shelling out to the `docker` CLI. Two environment variables must be set
before running:

| Variable                       | Purpose                                                    | Typical value (rootless Podman)                |
| ------------------------------ | ---------------------------------------------------------- | ---------------------------------------------- |
| `DOCKER_HOST`                  | Points testcontainers to the correct socket                | `unix:///run/user/$(id -u)/podman/podman.sock` |
| `TESTCONTAINERS_RYUK_DISABLED` | Disables Ryuk cleanup agent (required for rootless Podman) | `true`                                         |

Invocation:

```sh
DOCKER_HOST=unix:///run/user/$(id -u)/podman/podman.sock \
TESTCONTAINERS_RYUK_DISABLED=true \
  cargo test -p sansio-mqtt-v5-tokio --features integration-tests --test mqtt_integration
```

These requirements are documented in `crates/sansio-mqtt-v5-tokio/README.md`.

---

## File Layout

```
crates/sansio-mqtt-v5-tokio/
  Cargo.toml                        # feature + [[test]] entry + testcontainers dev-dep
  tests/
    client_event_loop.rs            # existing tests — untouched
    integration/
      main.rs                       # mod declarations for all submodules
      common.rs                     # broker helpers, ConnectOptions builders
      core_flows.rs                 # connect/disconnect, QoS 0/1/2, keep-alive
      session.rs                    # clean start, session resumption, will messages
      auth.rs                       # valid credentials, invalid credentials, anonymous rejected
      mosquitto/
        anonymous.conf              # allow_anonymous true, listener 1883
        authenticated.conf          # allow_anonymous false, password_file, listener 1883
        passwd                      # htpasswd-format: testuser:testhash
```

---

## Shared Helpers (`common.rs`)

```rust
pub async fn anonymous_broker() -> (ContainerAsync<GenericImage>, u16)
pub async fn authenticated_broker() -> (ContainerAsync<GenericImage>, u16)
pub fn anonymous_connect_options(port: u16) -> ConnectOptions
pub fn authenticated_connect_options(port: u16, user: &str, pass: &str) -> ConnectOptions
```

Each broker helper starts `eclipse-mosquitto:2`, copies the appropriate `.conf`
(and `passwd` for auth) into the container via `with_copy_to`, exposes TCP port
1883, and returns the mapped host port. The caller holds the `ContainerAsync`
handle — when it drops at end of test, the container stops automatically.

Each test function starts its own container to ensure full isolation between
tests.

---

## Test Scenarios

### `core_flows.rs`

| Test                              | What it verifies                                                                    |
| --------------------------------- | ----------------------------------------------------------------------------------- |
| `connect_and_disconnect`          | `Event::Connected` on connect; `Event::Disconnected` on clean disconnect            |
| `publish_qos0`                    | QoS 0 message published by client A arrives at client B as `Event::Message`         |
| `publish_qos1`                    | QoS 1: sender receives `Event::PublishAcknowledged`; receiver gets `Event::Message` |
| `publish_qos2`                    | QoS 2: sender receives `Event::PublishCompleted`; receiver gets `Event::Message`    |
| `keep_alive_maintains_connection` | With 5s keep-alive, connection stays alive after 7s idle (PINGREQ/PONG transparent) |

### `session.rs`

| Test                         | What it verifies                                                                                        |
| ---------------------------- | ------------------------------------------------------------------------------------------------------- |
| `clean_start_clears_session` | Subscribe, disconnect, reconnect with `clean_start=true` — no queued messages                           |
| `session_resumption`         | Subscribe offline with `clean_start=false`, publish while offline, reconnect — queued message delivered |
| `will_message_delivered`     | Connect with will, drop TCP without DISCONNECT — separate subscriber receives the will                  |

### `auth.rs`

| Test                           | What it verifies                                                              |
| ------------------------------ | ----------------------------------------------------------------------------- |
| `valid_credentials_accepted`   | Connect with correct username/password → `Event::Connected`                   |
| `invalid_credentials_rejected` | Connect with wrong password → `ConnectError` with bad-credentials reason code |
| `anonymous_rejected`           | Connect with no credentials to authenticated broker → connection rejected     |

---

## Mosquitto Configuration

**`anonymous.conf`**

```
listener 1883
allow_anonymous true
```

**`authenticated.conf`**

```
listener 1883
allow_anonymous false
password_file /mosquitto/config/passwd
```

**`passwd`** — generated with `mosquitto_passwd` or pre-computed hash:

```
testuser:<bcrypt-hash-of-testpassword>
```

---

## Out of Scope

- TLS/mTLS testing
- WebSocket transport
- CI integration (local-only for now)
- Multiple broker support
