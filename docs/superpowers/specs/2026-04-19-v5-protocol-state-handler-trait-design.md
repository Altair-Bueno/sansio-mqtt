# v5-Protocol `StateHandler` Trait Refactor

Date: 2026-04-19 Scope: `crates/sansio-mqtt-v5-protocol/` — internal
reorganization of the client state machine

## Goal

Break the 1815-line `proto.rs` into small, testable modules by modelling the
MQTT client as an FSM with a `StateHandler` trait. Reduce cyclomatic complexity
of the nested `handle_read_control_packet` / `handle_write` matches, make
transitions explicit in type signatures, and keep the public surface
backward-compatible only for the names `Client<Time>` and the `sansio::Protocol`
impl. All other public names may change.

## Non-goals

- No change to MQTT v5 wire behavior.
- No new dependency.
- No change to persistence boundary (still three components: settings,
  persistent session, transient scratchpad).
- No new feature flags.

## Architecture summary

- `Client<Time>` holds four fields: immutable settings, persistent session
  (formerly `ClientState`, renamed to `ClientSession`), transient scratchpad,
  and a new FSM field `state: ClientState`.
- `ClientState` is an enum whose variants wrap one struct per lifecycle state. A
  `Transitioning` variant serves as the zero-size `Default` sentinel so
  `core::mem::take` on `self.state` is free.
- A `StateHandler<Time>` trait defines one method per state-dependent entry
  point of `sansio::Protocol`. Each method consumes the state by value and
  returns `(ClientState, Result<(), Error>)`.
- `Client` dispatches through a single `#[inline(always)]` helper that pulls the
  state out with `mem::take`, runs the handler, and writes the next state back.
- State-specific logic lives in one file per state under `src/state/`.
- Cross-state helpers (packet encoding, queue mutation, limits, session ops)
  live in dedicated sibling modules and are free functions taking
  `&ClientSettings` / `&mut ClientSession` / `&mut ClientScratchpad<Time>`
  explicitly.

## Decisions

### 1) Top-level types and renames

`sansio::Protocol` impl and `Client<Time>` keep their exported names. Every
other public name may change.

- **Rename** persistent `ClientState` → `ClientSession`. Reflects "survives
  reconnect/power loss". Frees the name `ClientState` for the FSM.
- **Keep** `ClientScratchpad<Time>` and `ClientSettings`.
- **New** `ClientState` enum (the FSM):

  ```rust
  pub(crate) enum ClientState {
      Transitioning,                  // mem::take sentinel
      Start(Start),
      Disconnected(Disconnected),
      Connecting(Connecting),
      Connected(Connected),
  }

  impl Default for ClientState {
      fn default() -> Self { ClientState::Transitioning }
  }
  ```

- **`Client<Time>`** becomes:

  ```rust
  pub struct Client<Time> {
      settings:   ClientSettings,
      session:    ClientSession,
      scratchpad: ClientScratchpad<Time>,
      state:      ClientState,
  }
  ```

  Constructors rename to match the `session` naming:
  - `Client::with_settings(settings)` — unchanged public name
  - `Client::with_settings_and_state(settings, state)` → **rename to**
    `Client::with_settings_and_session(settings, session)`

  Both initialize `state` to `ClientState::Start(Start)`.

### 2) Per-state structs

Purely state-local data lives on state structs. Session-lifetime data stays on
`ClientScratchpad`.

```rust
pub(crate) struct Start;
pub(crate) struct Disconnected;
pub(crate) struct Connecting { pub pending_connect_options: ConnectionOptions }
pub(crate) struct Connected;
```

Consequences:

- `ConnectingPhase` and `connecting_phase` are **deleted**. They were write-only
  dead state (`AwaitConnAck`/`AuthInProgress` never branched behavior).
- `pending_connect_options` moves onto `Connecting`. The `Start → Connecting`
  transition now happens on `UserWriteIn::Connect`, not on `SocketConnected`. No
  external observer can see this because the lifecycle state is not exported and
  no packets flow until the socket opens.
- `negotiated_*` (9 fields), `keep_alive_*` (3 fields),
  `session_should_persist`, `read_buffer`, `read_queue`, `write_queue`,
  `action_queue`, `next_timeout` remain on `ClientScratchpad`. Session-lifetime,
  multi-state-reachable.

### 3) `StateHandler` trait

```rust
pub(crate) trait StateHandler<Time>: Sized {
    fn handle_control_packet(
        self,
        settings:   &ClientSettings,
        session:    &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        packet:     ControlPacket,
    ) -> (ClientState, Result<(), Error>);

    fn handle_write(
        self,
        settings:   &ClientSettings,
        session:    &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        msg:        UserWriteIn,
    ) -> (ClientState, Result<(), Error>);

    fn handle_event(
        self,
        settings:   &ClientSettings,
        session:    &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        evt:        DriverEventIn,
    ) -> (ClientState, Result<(), Error>);

    fn handle_timeout(
        self,
        settings:   &ClientSettings,
        session:    &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
        now:        Time,
    ) -> (ClientState, Result<(), Error>);

    fn close(
        self,
        settings:   &ClientSettings,
        session:    &mut ClientSession,
        scratchpad: &mut ClientScratchpad<Time>,
    ) -> (ClientState, Result<(), Error>);
}
```

- `settings` is `&`, never `&mut` (settings are immutable post-construction).
- All state-mutating side effects go through `&mut ClientSession` and
  `&mut ClientScratchpad<Time>`.
- `ClientState` itself `impl StateHandler<Time>`, matching on the variant and
  delegating to the inner struct. `Transitioning` panics with
  `unreachable!("FSM observed mid-transition")` — it is only observable if a
  handler forgot to assign the next state.
- `handle_read` stays on `Client<Time>` because byte-level parsing is not
  state-dependent; it only dispatches to `handle_control_packet` after parsing.

### 4) Dispatch

```rust
impl<Time: Copy + Ord + 'static> Client<Time> {
    #[inline(always)]
    fn dispatch<F>(&mut self, f: F) -> Result<(), Error>
    where
        F: FnOnce(
            ClientState,
            &ClientSettings,
            &mut ClientSession,
            &mut ClientScratchpad<Time>,
        ) -> (ClientState, Result<(), Error>),
    {
        let state = core::mem::take(&mut self.state);
        let (next, result) = f(state, &self.settings, &mut self.session, &mut self.scratchpad);
        self.state = next;
        result
    }
}
```

Each state-dependent `sansio::Protocol` method is a one-liner:

```rust
fn handle_write(&mut self, msg: UserWriteIn) -> Result<(), Self::Error> {
    self.dispatch(|s, set, ses, sp| s.handle_write(set, ses, sp, msg))
}
```

### 5) File layout

```
crates/sansio-mqtt-v5-protocol/src/
├── lib.rs                  # re-exports: Client, ClientSettings, ClientSession, types::*
├── types.rs                # unchanged
├── client.rs               # Client<Time>, Default, new*, dispatch, Protocol impl
├── scratchpad.rs           # ClientScratchpad<Time> + Default + reset helpers
├── session.rs              # ClientSession + Default + clear
├── state/
│   ├── mod.rs              # enum ClientState + StateHandler trait + dispatch impl
│   ├── start.rs            # Start + impl StateHandler
│   ├── disconnected.rs     # Disconnected + impl StateHandler
│   ├── connecting.rs       # Connecting { pending_connect_options } + impl
│   │                       #   (CONNECT packet built inline in SocketConnected handler)
│   └── connected.rs        # Connected + impl (bulk of packet dispatch)
├── limits.rs               # recompute_effective_limits, reset_negotiated_limits,
│                           # validate_outbound_*, apply_inbound_publish_topic_alias,
│                           # ensure_outbound_receive_maximum_capacity,
│                           # min_option_nonzero_u16 / min_option_nonzero_u32
├── queues.rs               # encode_control_packet, enqueue_packet,
│                           # enqueue_(pubrel|puback|pubrec|pubcomp)_or_fail_protocol,
│                           # fail_protocol_and_disconnect
└── session_ops.rs          # reset_inflight_transactions, clear_pending_subscriptions,
                            # replay_outbound_inflight_with_dup,
                            # emit_publish_dropped_for_all_inflight,
                            # next_packet_id, next_outbound_publish_packet_id,
                            # next_packet_id_checked
```

`proto.rs` is deleted at the end. `build_connect_packet` is not extracted — it
is called from exactly one non-test site; it is inlined into
`Connecting::handle_event(SocketConnected)`. Its three redundant in-file unit
tests are deleted; their coverage exists in `client_protocol.rs` integration
tests (`connect_encodes_*`, `connect_defaults_*`, `connect_topic_alias_*`).

Soft size caps: `client.rs` ≤ 250 LOC, `state/*.rs` ≤ 400 LOC (Connected is the
largest), every other module ≤ 300 LOC.

### 6) Transition map

Authoritative summary of what each state handles and where it transitions.
Implementation must match. All `[MQTT-x.x.x-y]` citations currently attached to
the relevant logic must be preserved at their call sites.

#### `Start`

| Input                         | Transition                                       | Notes                                                                                                                                                                                                                                      |
| ----------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `handle_write(Connect(opts))` | → `Connecting { pending_connect_options: opts }` | Recompute effective limits. If `opts.clean_start` then `session.clear()` [MQTT-3.1.2-4]. Set `scratchpad.session_should_persist = opts.session_expiry_interval.unwrap_or(0) > 0`. Push `DriverEventOut::OpenSocket` if not already queued. |
| `handle_write(*)` else        | stay                                             | `Err(InvalidStateTransition)`                                                                                                                                                                                                              |
| `handle_event(*)`             | stay                                             | `Err(InvalidStateTransition)`                                                                                                                                                                                                              |
| `handle_control_packet(*)`    | → `Disconnected`                                 | `fail_protocol_and_disconnect(ProtocolError)` + `Err(ProtocolError)`                                                                                                                                                                       |
| `handle_timeout(_)`           | stay                                             | no-op                                                                                                                                                                                                                                      |
| `close()`                     | → `Disconnected`                                 | reset keepalive/negotiated; `maybe_reset_session_state`; no `UserWriteOut::Disconnected` emit                                                                                                                                              |

#### `Disconnected`

| Input                         | Transition                                       | Notes                                                                |
| ----------------------------- | ------------------------------------------------ | -------------------------------------------------------------------- |
| `handle_write(Connect(opts))` | → `Connecting { pending_connect_options: opts }` | same as from `Start`                                                 |
| `handle_write(*)` else        | stay                                             | `Err(InvalidStateTransition)`                                        |
| `handle_event(*)`             | stay                                             | `Err(InvalidStateTransition)`                                        |
| `handle_control_packet(*)`    | stay                                             | `fail_protocol_and_disconnect(ProtocolError)` + `Err(ProtocolError)` |
| `handle_timeout(_)`           | stay                                             | no-op                                                                |
| `close()`                     | stay                                             | idempotent                                                           |

#### `Connecting { pending_connect_options }`

| Input                                                              | Transition       | Notes                                                                                                                                                                                                                                                                                                                                                              |
| ------------------------------------------------------------------ | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `handle_event(SocketConnected)`                                    | stay             | `reset_negotiated_limits`; inline-build `Connect` from `pending_connect_options` + `settings`; enqueue; reset keepalive counters                                                                                                                                                                                                                                   |
| `handle_event(SocketClosed)`                                       | → `Disconnected` | reset scratchpad, `maybe_reset_session_state`, push `UserWriteOut::Disconnected`                                                                                                                                                                                                                                                                                   |
| `handle_event(SocketError)`                                        | → `Disconnected` | reset scratchpad, `maybe_reset_session_state`, push `CloseSocket`; `Err(ProtocolError)`                                                                                                                                                                                                                                                                            |
| `handle_control_packet(ConnAck(Success \| ResumePreviousSession))` | → `Connected`    | populate `negotiated_*`, `recompute_effective_limits`, set keepalive interval, emit `UserWriteOut::Connected`. For `ResumePreviousSession`: reject if `pending_connect_options.clean_start` [MQTT-3.2.2-2]; else `replay_outbound_inflight_with_dup` [MQTT-4.4.0-1] [MQTT-4.4.0-2]. For `Success`: `emit_publish_dropped_for_all_inflight` + `reset_session_state` |
| `handle_control_packet(ConnAck(failure))`                          | → `Disconnected` | reset negotiated, push `CloseSocket`; `Err(ProtocolError)`                                                                                                                                                                                                                                                                                                         |
| `handle_control_packet(Auth(ContinueAuthentication))`              | stay             | require `pending_connect_options.authentication.is_some()`; else fail-and-disconnect                                                                                                                                                                                                                                                                               |
| `handle_control_packet(*)` else                                    | → `Disconnected` | `fail_protocol_and_disconnect(ProtocolError)` + `Err(ProtocolError)`                                                                                                                                                                                                                                                                                               |
| `handle_write(Disconnect)`                                         | → `Disconnected` | enqueue DISCONNECT best-effort, push `CloseSocket`, emit `Disconnected`                                                                                                                                                                                                                                                                                            |
| `handle_write(*)` else                                             | stay             | `Err(InvalidStateTransition)`                                                                                                                                                                                                                                                                                                                                      |
| `handle_timeout(_)`                                                | stay             | no-op                                                                                                                                                                                                                                                                                                                                                              |
| `close()`                                                          | → `Disconnected` | enqueue DISCONNECT best-effort, push `CloseSocket`, reset scratchpad, emit `Disconnected`, `maybe_reset_session_state`                                                                                                                                                                                                                                             |

#### `Connected`

| Input                                                                                         | Transition               | Notes                                                                                                              |
| --------------------------------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| `handle_control_packet(Publish, PubAck, PubRec, PubRel, PubComp, SubAck, UnsubAck, PingResp)` | stay                     | full QoS handling as today; spec citations preserved                                                               |
| `handle_control_packet(Disconnect)`                                                           | → `Disconnected`         | reset keepalive/negotiated, `maybe_reset_session_state`, push `UserWriteOut::Disconnected` and `CloseSocket`       |
| `handle_control_packet(*)` else                                                               | → `Disconnected`         | `fail_protocol_and_disconnect(ProtocolError)` + `Err(ProtocolError)`                                               |
| `handle_write(Publish, Acknowledge, Reject, Subscribe, Unsubscribe)`                          | stay                     | same logic as today                                                                                                |
| `handle_write(Disconnect)`                                                                    | → `Disconnected`         | enqueue DISCONNECT, push `CloseSocket`, reset, emit `Disconnected`                                                 |
| `handle_write(Connect(_))`                                                                    | stay                     | `Err(InvalidStateTransition)`                                                                                      |
| `handle_event(SocketClosed)`                                                                  | → `Disconnected`         | reset scratchpad, `maybe_reset_session_state`, emit `UserWriteOut::Disconnected`                                   |
| `handle_event(SocketError)`                                                                   | → `Disconnected`         | same + `CloseSocket`; `Err(ProtocolError)`                                                                         |
| `handle_event(SocketConnected)`                                                               | stay                     | `Err(InvalidStateTransition)`                                                                                      |
| `handle_timeout(now)`                                                                         | stay or → `Disconnected` | full keepalive logic [MQTT-3.1.2-22] [MQTT-3.1.2-24] [MQTT-3.12.4-1] [MQTT-4.13.1-1]; PINGREQ send or timeout-fail |
| `close()`                                                                                     | → `Disconnected`         | enqueue DISCONNECT, push `CloseSocket`, reset scratchpad, emit `Disconnected`, `maybe_reset_session_state`         |

### 7) `InvalidStateTransition` return pattern

Handlers stay in the current state and return
`(ClientState::<Variant>(self), Err(InvalidStateTransition))`. The trait's
by-value-self signature makes this explicit: the state struct is passed back
through unchanged.

## Spec compliance

- Every `[MQTT-x.x.x-y]` citation currently attached to logic in `proto.rs` MUST
  be preserved at the new call site. The reorganization is purely structural.
- New state-transition documentation in `state/mod.rs` (on the `ClientState`
  enum and the `StateHandler` trait) MAY reference the relevant sections but is
  not required to duplicate the per-line citations.
- Tests in `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs` and
  `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs` are the compliance
  surface. Their assertions MUST pass unchanged; only the public type and
  constructor names change (see "Testing strategy" below).

## Testing strategy

- Integration tests in `client_protocol.rs` must be updated only to reflect the
  public renames:
  - `sansio_mqtt_v5_protocol::ClientState` →
    `sansio_mqtt_v5_protocol::ClientSession` (currently referenced at lines 110,
    118, 140).
  - `Client::with_settings_and_state` → `Client::with_settings_and_session`
    (currently referenced at lines 111, 116, 138).
  - Test function names containing the word `state` that specifically mean the
    persistent component (e.g. `client_new_with_state_accepts_preloaded_state`)
    SHOULD be renamed to `*_session_*` for clarity but may stay if the rename
    churn is not worth the noise. Behavior assertions are unchanged.
- Integration tests in `crates/sansio-mqtt-v5-tokio/tests/client_event_loop.rs`
  pass unmodified (tokio crate only uses `Client::with_settings`, not the
  renamed constructor).
- The three in-file unit tests in `proto.rs` that directly test
  `build_connect_packet` are deleted (coverage duplicated by integration tests).
- The two other in-file unit tests
  (`socket_connected_error_does_not_poison_state`,
  `pubrel_enqueue_failure_forces_protocol_close`) are moved into the relevant
  state module under `#[cfg(test)]`.
- Per-task: `cargo fmt --check` +
  `cargo clippy --workspace --all-targets -- -D warnings` +
  `cargo test --workspace` green.
- Per-state trait-impl tests are only added for transitions not already covered
  by an integration test.

## Rollout

Single branch, one commit per extracted module while `proto.rs` remains the
dispatcher. A later commit introduces the state/FSM split and rewires `Client`.
Final commit deletes `proto.rs`. Each commit keeps the full test suite green.

Commit order (indicative):

1. Extract `queues.rs` (encoder + enqueue helpers).
2. Extract `limits.rs` (effective-limit recomputation + validators +
   `min_option_nonzero_*`).
3. Extract `session_ops.rs` (packet-id allocator, replay, inflight drop,
   pending-map clears).
4. Extract `scratchpad.rs` (`ClientScratchpad<Time>` + Default +
   scratchpad-local reset helpers).
5. Extract `session.rs` (`ClientSession` renamed from `ClientState`).
6. Introduce `state/` (enum `ClientState` + `StateHandler` trait + four state
   structs + dispatch).
7. Delete `proto.rs`.

## Verification checklist

- `cargo fmt --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace` green.
- `cargo build -p sansio-mqtt-v5-tokio` still builds with no source changes.
- `#![forbid(unsafe_code)]` in `lib.rs` preserved.
- `no_std` preserved; no `std::` imports outside `cfg(test)`.
- `proto.rs` does not exist.
- `Client<Time>` and `sansio::Protocol for Client<Time>` exported with unchanged
  method signatures.
- Soft size caps met: `client.rs` ≤ 250 LOC, any `state/*.rs` ≤ 400 LOC, every
  other module ≤ 300 LOC.
- All `[MQTT-x.x.x-y]` citations from `proto.rs` appear at the corresponding new
  call sites.
