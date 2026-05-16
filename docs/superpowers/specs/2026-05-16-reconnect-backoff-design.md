# Reconnect & Backoff Design — sansio-mqtt-v5-tokio

**Date:** 2026-05-16
**Crate:** `sansio-mqtt-v5-tokio`

## 1. Motivation

The tokio driver has no reconnection logic. After `Event::Disconnected` is emitted the
caller must tear down `Client` + `EventLoop`, rebuild `ConnectOptions`, and call the
`connect()` free function again — a significant burden for resilient applications. This
design adds optional automatic reconnection with a configurable backoff algorithm.

## 2. Architecture

### 2.1 Remove channels — single `Connection` struct

`Client` (command sender) and `EventLoop` (event poller) are replaced by a single
`Connection` struct. The `mpsc` channel between them is removed. Commands feed directly
into the sansio protocol state machine via synchronous `&mut self` methods; async I/O is
driven exclusively by `poll()`.

```rust
pub struct Connection { /* owns TcpStream + ProtocolClient + reconnect state */ }

impl Connection {
    pub async fn connect(options: ConnectOptions) -> Result<Self, ConnectError>;
    pub async fn poll(&mut self) -> Result<Event, ConnectionError>;

    pub fn publish(&mut self, msg: ClientMessage) -> Result<(), ConnectionError>;
    pub fn subscribe(&mut self, opts: SubscribeOptions) -> Result<(), ConnectionError>;
    pub fn unsubscribe(&mut self, opts: UnsubscribeOptions) -> Result<(), ConnectionError>;
    pub fn disconnect(&mut self) -> Result<(), ConnectionError>;
}
```

Typical usage:

```rust
let mut conn = Connection::connect(opts).await?;
loop {
    tokio::select! {
        event = conn.poll() => { handle(event?)? }
        line  = stdin.next_line() => { conn.publish(make_msg(line))? }
    }
}
```

The `&mut self` borrow on both branches enforces single-owner access — no `Arc<Mutex<_>>`,
no channel capacity to tune.

### 2.2 `ConnectOptions` changes

```rust
pub struct ConnectOptions {
    pub addr: SocketAddr,
    pub connection: ConnectionOptions,
    pub protocol_config: ClientSettings,
    pub max_in_queued_messages: usize,   // cap on inbound in-flight BTree (QoS 1/2)
    pub max_out_queued_messages: usize,  // cap on outbound in-flight BTree (QoS 1/2)
    pub backoff: Option<Backoff>,        // None = no reconnect (current behaviour)
    // command_channel_capacity removed
}
```

`max_in_queued_messages` and `max_out_queued_messages` are hard limits on the internal
BTrees the sansio protocol state machine uses to track in-flight messages in each
direction. When the outbound BTree is full, sync command methods return
`ConnectionError::QueueFull` immediately. When the inbound BTree is full, the sansio
protocol enforces the MQTT Receive Maximum [MQTT-3.3.4-7] — the broker is not permitted
to send further QoS 1/2 PUBLISH packets until slots are freed by acknowledgement; this
is handled entirely within the protocol state machine and is not surfaced as a driver
error. These are the sole backpressure mechanisms.

### 2.3 Backoff types

```rust
pub struct Backoff {
    pub algorithm: BackoffAlgorithm,
    pub range: RangeInclusive<Duration>,  // min..=max delay between attempts
    pub seed: u64,                        // RNG seed; used by Jitter variants only
}

pub enum BackoffAlgorithm {
    /// delay = range.start + slope * attempt, clamped to range.end
    Linear { slope: Duration },
    /// delay = range.start * factor^attempt, clamped to range.end
    Exponential { factor: f64 },
    /// uniform random in range.start..=range.end (attempt-independent)
    Jitter,
    /// exponential result + uniform random in 0..=result, then clamped to range.end
    JitteredExponential { factor: f64 },
}
```

All arithmetic uses `Duration::saturating_add` / `saturating_mul` to prevent overflow.

## 3. Connection State Machine

`Connection` holds an internal socket state alongside the protocol state machine. The
protocol state machine is **never reset** between reconnects when `Clean Start = 0` —
it preserves session state for QoS continuity. When `Clean Start = 1` (set in
`ConnectionOptions`) the protocol intentionally discards session state on each reconnect;
this is already encoded in the sansio layer and requires no special handling in the driver.

```rust
enum SocketState {
    Active(TcpStream),
    Offline { attempt: u32, wake_at: Instant },
}
```

### `poll()` behaviour

**When `Active`:**

`tokio::select!` over socket reads, keep-alive timeouts, and pending protocol writes.
On `DriverEventOut::CloseSocket` or an I/O error:
1. Flush any remaining pending writes.
2. Return `Event::Disconnected(reason)`.
3. If `backoff` is configured → transition to `Offline { attempt: 0, wake_at: now + backoff(0) }`.
4. If `backoff` is `None` → transition to terminal; subsequent `poll()` returns `ConnectionError::Disconnected`.

**When `Offline` (backoff configured):**

`poll()` runs an internal retry loop:
1. Sleep until `wake_at`.
2. Attempt `TcpStream::connect(addr)`.
3. Replay the MQTT `CONNECT` / `CONNACK` handshake via the existing sansio protocol.
4. On success → transition to `Active`, return `Event::Connected`.
5. On failure → `attempt += 1`, `wake_at = Instant::now() + backoff(attempt)`, continue loop.

The caller sees nothing between `Event::Disconnected` and the eventual `Event::Connected`
(or a fatal `ConnectionError`).

`poll()` is cancellation-safe: the backoff sleep uses `tokio::time::sleep` (cancel-safe).
If the future is dropped mid-reconnect-handshake, the in-progress `TcpStream` is dropped
and the next `poll()` call restarts from the sleep phase of the current attempt (i.e.
`wake_at` is not re-computed; the attempt counter is not incremented).

**When `Offline` (no backoff) — terminal:**

`poll()` returns `ConnectionError::Disconnected` immediately.

### Sync command methods

`publish()`, `subscribe()`, `unsubscribe()`, and `disconnect()` feed `UserWriteIn`
directly into the sansio protocol state machine. They operate in both `Active` and
`Offline` states. If the outbound BTree has reached `max_out_queued_messages`, they
return `ConnectionError::QueueFull`. The caller may retry after `poll()` has made
progress and drained some in-flight messages.

## 4. Backoff Algorithm

`backoff(attempt: u32) -> Duration` implementation per variant:

| Variant | Formula |
|---|---|
| `Linear { slope }` | `range.start + slope * attempt`, clamped to `range.end` |
| `Exponential { factor }` | `range.start * factor^attempt`, clamped to `range.end` |
| `Jitter` | xorshift64(seed) → uniform in `range.start..=range.end` |
| `JitteredExponential { factor }` | exponential result + xorshift64 in `0..=result`, clamped to `range.end` |

### RNG

**xorshift64** is used for all jitter variants: three XOR-shift operations, no external
dependencies, deterministic given `seed`. The RNG state is stored inside `Connection`
and advanced on each jitter call. `seed` is taken from `Backoff::seed`.

```rust
fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
```

`Linear` and `Exponential` variants ignore `seed` and hold no RNG state.

## 5. Error Handling

`ClientError` and `EventLoopError` are removed. Two error types remain:

**`ConnectError`** — returned by `Connection::connect()` only:
- `Io` — TCP connection failed
- `Protocol` — protocol violation during initial handshake
- `UnexpectedDriverAction` — sansio emitted an unrecognised action

**`ConnectionError`** — returned by `poll()` and all command methods:
- `Io` — socket I/O failure (fatal, no backoff)
- `Protocol` — protocol state machine violation (fatal)
- `UnexpectedDriverAction` — sansio emitted an unrecognised action (fatal)
- `ProtocolRequestedQuit` — broker sent a fatal DISCONNECT (fatal)
- `QueueFull` — `max_out_queued_messages` reached; recoverable, returned by command methods
- `Disconnected` — `poll()` called on a terminal connection with no backoff configured

Fatal errors leave `Connection` in an unusable state. `QueueFull` is recoverable.

## 6. Testing

| Test | Location | What it verifies |
|---|---|---|
| Backoff algorithm unit tests | `sansio-mqtt-v5-tokio` | Each variant produces correct delay sequence; jitter stays within `range`; fixed seed is deterministic; all variants saturate at `range.end` |
| Reconnect integration test | `test-sansio-mqtt-v5-tokio-mosquitto` | Connect → broker forcibly closes → `Event::Disconnected` emitted → `Event::Connected` emitted → queued publishes delivered |
| Queue-full test | `sansio-mqtt-v5-tokio` | Filling `max_out_queued_messages` returns `ConnectionError::QueueFull` on the next command |
| No-backoff regression | `sansio-mqtt-v5-tokio` | `backoff: None` → `ConnectionError::Disconnected` after disconnect |

## 7. Out of Scope

- TLS / WebSocket transport support
- Infinite retry without a cap (range.end enforces a maximum delay, not a maximum attempt count)
- Subscription re-registration after reconnect (responsibility of the caller, signalled by `Event::Connected`)
