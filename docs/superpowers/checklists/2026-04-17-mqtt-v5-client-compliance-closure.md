# MQTT v5 Client Compliance Closure Checklist

Purpose: track remaining work required before claiming `sansio-mqtt-v5-protocol`
is spec-compliant as an MQTT v5 client.

Scope: `crates/sansio-mqtt-v5-protocol`

Status key:

- `DONE` implemented and tested
- `PARTIAL` implemented but not full spec behavior
- `MISSING` not implemented

## 1) Connection and Session Lifecycle

- [x] `DONE` CONNECT/CONNACK happy path
  - References: `crates/sansio-mqtt-v5-protocol/src/proto.rs`,
    `crates/sansio-mqtt-v5-protocol/tests/client_protocol.rs`
- [x] `DONE` Clean Start local reset behavior
  - Spec anchor: `[MQTT-3.1.2-4]`
- [ ] `PARTIAL` Session expiry full semantics
  - Gap: effective session lifetime and disconnect-property override handling
    are simplified.
- [x] `DONE` session-expiry-aware persistence on error/timeout close paths
  - Behavior: protocol-error/keepalive close paths now apply session persistence
    policy (preserve state when expiry > 0).
- [x] `DONE` session-expiry=0 enforcement on transport-close paths
  - Behavior: socket close/error now discard local session state when session
    expiry is zero/absent.
- [x] `DONE` CONNACK Session Present mismatch enforcement
  - Behavior: Session Present=1 with `clean_start=true` is treated as protocol
    error and connection close.
- [x] `DONE` Will QoS/Retain semantic support in CONNECT
  - Behavior: `Will` now carries `qos` and `retain`, mapped to CONNECT Will
    payload.

## 2) Keep Alive

- [x] `DONE` PINGREQ/PINGRESP basic flow
- [x] `DONE` close on keepalive timeout
  - Spec anchors in code: `[MQTT-3.1.2-24]`, `[MQTT-4.13.1-1]`
- [x] `DONE` CONNACK Server Keep Alive zero-value handling
  - Behavior: value `0` no longer panics and disables keepalive timers.
- [ ] `PARTIAL` time-accurate scheduling guarantees
  - Gap: strict interval/deadline accounting and scheduler contract should be
    formalized and proven by tests.

## 3) Enhanced Authentication (AUTH)

- [x] `DONE` AUTH packets tolerated during Connecting
- [ ] `MISSING` full enhanced-auth state machine
  - Gap: ContinueAuthentication/ReAuthenticate round-trips, auth-data
    propagation, and reason-code driven transitions.

## 4) QoS Flows

- [x] `DONE` Outbound QoS1 (`PUBLISH` -> `PUBACK`)
  - Spec family: section `4.3.2`
- [x] `DONE` Outbound QoS2 (`PUBLISH` -> `PUBREC` -> `PUBREL` -> `PUBCOMP`)
  - Spec family: section `4.3.3`
- [x] `DONE` Inbound QoS1 receiver path with `PUBACK`
- [x] `DONE` Inbound QoS2 receiver path including duplicate `PUBLISH` handling
      and `PUBREL`/`PUBCOMP`
- [x] `DONE` strict ACK guardrails for unexpected packet/state mismatches

## 5) Flow Control and Limits

- [x] `DONE` Receive Maximum immediate-error enforcement
  - Spec family: section `4.9`
- [x] `DONE` Outbound Maximum Packet Size enforcement (from CONNACK)
- [x] `DONE` CONNECT-side Maximum Packet Size advertisement support
- [x] `DONE` CONNECT-side Receive Maximum advertisement support
- [x] `DONE` Outbound Topic Alias Maximum enforcement
- [x] `DONE` Inbound Topic Alias protocol semantics
  - Behavior: alias registration/lookup implemented; alias-only unknown alias
    and over-limit alias are protocol errors.
- [x] `DONE` full server capability enforcement
  - Behavior: outbound publish enforces CONNACK `Maximum QoS` and
    `Retain Available`.
- [x] `DONE` explicit subscribe capability enforcement
  - Behavior: enforced `Wildcard Subscription Available`,
    `Shared Subscription Available`, and `Subscription Identifier Available`.

## 6) Subscribe/Unsubscribe Lifecycle

- [x] `DONE` packet-id tracking for SUBSCRIBE/SUBACK and UNSUBSCRIBE/UNSUBACK
  - Spec anchors in code: `[MQTT-3.8.4-1]`, `[MQTT-3.10.4-1]`
- [ ] `MISSING` pending SUBSCRIBE/UNSUBSCRIBE lifecycle cleanup across reconnect
      boundaries
  - Gap: stale pending subscribe/unsubscribe packet-id reservations can survive
    resumed session paths and block future valid packet-id allocation.
- [x] `DONE` shared-subscription + No Local protocol validation
  - Spec anchor: `[MQTT-3.8.3-4]`
- [ ] `MISSING` richer reason-code outcome handling
  - Gap: more explicit user-facing handling/reporting for sub/unsub ACK reason
    codes.

## 7) Error Handling and Closure Semantics

- [x] `DONE` malformed packet strict close path
  - Spec anchor in code: `[MQTT-4.13.1-1]`
- [x] `DONE` protocol-error strict close path
- [x] `DONE` best-effort DISCONNECT emission before close where applicable

## 8) Reconnect and Inflight Replay

- [ ] `PARTIAL` session-resumed inflight replay with DUP semantics
  - Spec family: section `4.4`
- [x] `DONE` resend of unacknowledged PUBREL on session-resume reconnect
  - Spec anchor: `[MQTT-4.4.0-1]`
- [x] `DONE` non-resumed session inflight drop events
- [x] `DONE` full local session-state discard on Session Present=0
  - Spec anchor: `[MQTT-3.2.2-5]`
- [ ] `PARTIAL` broadened replay validation matrix
  - Gap: add explicit edge-case matrix for all inflight states across reconnect
    boundaries.

## 9) Conformance Evidence (Release Gate)

- [x] `DONE` requirement traceability matrix (mandatory client requirements ->
      tests)
  - File:
    `docs/superpowers/checklists/2026-04-17-mqtt-v5-client-requirement-traceability.md`
- [x] `DONE` explicit pass/fail conformance report artifact for CI/release
  - File:
    `docs/superpowers/checklists/2026-04-18-mqtt-v5-client-conformance-report.md`

## Closure Plan (recommended order)

1. Complete enhanced AUTH state machine.
2. Implement inbound Topic Alias semantics and CONNECT-side packet-size
   advertisement behavior.
3. Complete capability enforcement (`Maximum QoS`, `Retain Available`, subscribe
   capability flags, etc.).
4. Tighten keepalive timing model and Server Keep Alive edge cases (including
   `0`).
5. Expand reconnect/inflight edge-case matrix.
6. Produce requirement traceability matrix and final conformance report.

## Notes

- This checklist is intentionally conservative: only mark `DONE` when behavior
  is implemented and covered by targeted tests.
- Keep spec markers in code using `[MQTT-x.x.x-y]` where IDs are defined and
  precise section references where IDs are not explicit.
