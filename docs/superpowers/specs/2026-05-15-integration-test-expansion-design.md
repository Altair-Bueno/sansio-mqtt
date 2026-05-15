# Integration Test Expansion Design

**Date:** 2026-05-15  
**Crate:** `test-sansio-mqtt-v5-tokio-mosquitto`  
**Goal:** Expand the integration test suite from 11 to ~61 tests, covering all
gaps identified in `docs/review-tests.md`.

---

## Context

The existing integration test crate spins up a real Mosquitto 2 broker via
testcontainers and exercises the full `sansio-mqtt-v5-tokio` async client. It
currently has 11 tests across three files:

- `core_flows.rs` — connect/disconnect, QoS 0/1/2 basic, keep-alive
- `auth.rs` — valid credentials, invalid credentials, anonymous rejected
- `session.rs` — clean start, session resumption, will on abrupt disconnect

`docs/review-tests.md` identifies ~52 specific gaps. This design addresses all
of them by adding nine new test files and minor helpers to `lib.rs`.

---

## Approach: One File per MQTT Feature Area

Each new file maps directly to a section of the MQTT v5.0 spec, making coverage
gaps visible by filename. No existing files are modified.

---

## New Helpers in `lib.rs`

Three helpers added alongside the existing ones:

```rust
/// ConnectOptions pre-loaded with a Will message.
pub fn will_connect_options(port: u16, client_id: &str, will: Will) -> ConnectOptions

/// SubscribeOptions with full control over no_local, retain_as_published,
/// retain_handling, and an optional subscription_identifier.
pub fn sub_with_options(topic: &str, qos: Qos, no_local: bool,
    retain_as_published: bool, retain_handling: RetainHandling,
    subscription_identifier: Option<NonZero<u64>>) -> SubscribeOptions

/// ClientMessage with retain=true (for testing retained message behaviour).
pub fn msg_retain(topic: &str, payload: &str, qos: Qos) -> ClientMessage
```

---

## New Test Files

### `will_messages.rs` — §3.1.2.5-9, §4.3 (8 tests)

| Test                                   | Behaviour verified                                                             |
| -------------------------------------- | ------------------------------------------------------------------------------ |
| `will_not_sent_on_graceful_disconnect` | `disconnect()` suppresses will [MQTT-3.1.2-10]                                 |
| `will_sent_on_abrupt_disconnect_qos0`  | Drop event loop → QoS 0 will delivered                                         |
| `will_sent_on_abrupt_disconnect_qos1`  | Drop event loop → QoS 1 will arrives as `MessageWithRequiredAcknowledgement`   |
| `will_with_retain_flag`                | Retained will received by subscriber connecting after disconnect               |
| `will_with_delay_interval`             | `will_delay_interval=2s` → will arrives after 2 s                              |
| `will_with_expiry_interval`            | `expiry_interval=1s` + `will_delay_interval=3s` → will expires before delivery |
| `will_with_empty_payload`              | Zero-length will payload accepted and delivered                                |
| `will_with_user_properties`            | User properties on will preserved in delivered message                         |

**Setup note:** Tests that use `will_delay_interval` require real wall-clock
sleep; keep delays short (≤3 s) to avoid slow CI.

---

### `retain.rs` — §3.3.1.3, §4.1 (7 tests)

| Test                                           | Behaviour verified                                                              |
| ---------------------------------------------- | ------------------------------------------------------------------------------- |
| `retained_message_delivered_to_new_subscriber` | Retained publish → late subscriber receives on subscribe                        |
| `clear_retained_message`                       | Empty-payload retain publish clears retained message                            |
| `retained_message_is_latest_value`             | Two retained publishes → subscriber sees only the second                        |
| `retain_handling_send_on_subscribe`            | `RetainHandling::SendOnSubscribe` (default) sends retained every subscribe      |
| `retain_handling_send_only_on_new_subscribe`   | `RetainHandling::SendOnNewSubscribe` sends retained only for new subscriptions  |
| `retain_handling_do_not_send`                  | `RetainHandling::DoNotSend` never sends retained on subscribe                   |
| `retain_as_published_preserves_retain_flag`    | Subscription with `retain_as_published=true` → received message has retain=true |

---

### `topic_filters.rs` — §4.7 (7 tests)

| Test                                                | Behaviour verified                                                                                            |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `single_level_wildcard_matches_one_segment`         | `sensors/+/temp` matches `sensors/room1/temp`, not `sensors/a/b/temp`                                         |
| `multi_level_wildcard_matches_all_below`            | `sensors/#` receives messages at any depth                                                                    |
| `hash_wildcard_does_not_match_dollar_prefix`        | `#` subscription does not deliver `$SYS/...` topics                                                           |
| `multiple_topics_in_one_subscribe_packet`           | `extra_subscriptions` field → all 3 topic types received                                                      |
| `overlapping_subscriptions_deliver_once_per_filter` | Subscribe to `a/#` and `a/b`; publish to `a/b`; Mosquitto 2 delivers **2 messages** (one per matching filter) |
| `resubscribe_upgrades_qos`                          | Subscribe QoS 0 → resubscribe QoS 1 → message arrives as `MessageWithRequiredAcknowledgement`                 |
| `unsubscribe_followed_by_resubscribe`               | Unsub then resub → messages flow again                                                                        |

---

### `session_advanced.rs` — §3.1.2.4, §4.2.1.1 (5 tests)

| Test                                                   | Behaviour verified                                                                                                 |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| `multiple_inflight_qos1_all_delivered_after_reconnect` | 3 QoS 1 publishes queued while offline → all 3 delivered on session resume                                         |
| `multiple_inflight_qos2_all_completed_after_reconnect` | 2 QoS 2 publishes mid-flight → both `PublishCompleted` after resume                                                |
| `queued_inbound_messages_arrive_after_connack`         | Subscriber offline; 3 messages queued; all arrive after `Connected` on resume                                      |
| `session_takeover_disconnects_old_connection`          | Same `client_id` from second connection → first client gets `Disconnected(Some(SessionTakenOver))`                 |
| `session_expiry_drops_subscriptions`                   | `session_expiry=1s`, wait 2 s, reconnect `clean_start=false` → CONNACK `session_present=false`, no queued messages |

---

### `subscriptions.rs` — §3.8-3.11 (8 tests)

| Test                                               | Behaviour verified                                                                          |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `no_local_prevents_receiving_own_publishes`        | `no_local=true` → own publish not echoed back                                               |
| `subscription_identifier_delivered_with_message`   | Sub with `subscription_identifier=42` → `BrokerMessage.subscription_identifiers=[42]`       |
| `subscription_identifier_on_wildcard_subscription` | Wildcard sub with identifier → all matching messages carry it                               |
| `unsubscribe_stops_delivery`                       | Subscribe, receive, unsubscribe, publish → second message not received                      |
| `unsubscribe_from_nonexistent_topic_succeeds`      | Unsubscribe from never-subscribed topic → no error                                          |
| `resubscribe_downgrades_qos`                       | Subscribe QoS 1 → resubscribe QoS 0 → messages arrive at QoS 0                              |
| `multiple_subscriptions_one_call`                  | `extra_subscriptions` with 3 topics → all receive successfully                              |
| `shared_subscription_load_balancing`               | Two clients on `$share/group/topic` → each published message goes to exactly one subscriber |

---

### `message_properties.rs` — §3.3.2 (6 tests)

| Test                                             | Behaviour verified                                                                |
| ------------------------------------------------ | --------------------------------------------------------------------------------- |
| `user_properties_preserved_end_to_end`           | 3 user properties published → all 3 received unchanged                            |
| `message_expiry_interval_drops_stale_message`    | `expiry_interval=1s`, subscriber offline 2 s → no message on reconnect            |
| `response_topic_and_correlation_data_round_trip` | Requester sets `response_topic` + `correlation_data`; responder reads and replies |
| `content_type_preserved`                         | `content_type="application/json"` present in received `BrokerMessage`             |
| `payload_format_indicator_preserved`             | `format_indicator=Utf8` present in received `BrokerMessage`                       |
| `multiple_user_properties_duplicate_keys`        | Duplicate user property keys allowed and all pairs preserved in order             |

---

### `qos2_advanced.rs` — §4.3.3 (4 tests)

| Test                                              | Behaviour verified                                                                                                         |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `concurrent_qos2_publishes_all_complete`          | 5 concurrent QoS 2 publishes → all 5 `PublishCompleted` events received                                                    |
| `qos2_session_resume_completes_inflight_outbound` | Disconnect after PUBREC, resume → PUBREL/PUBCOMP completes; subscriber receives exactly once                               |
| `qos2_inbound_across_reconnect`                   | Subscriber receives QoS 2, acknowledges, disconnects before PUBCOMP → re-delivery on reconnect is handled cleanly          |
| `qos2_receive_maximum_respected`                  | Publish more QoS 2 messages than `receive_maximum` allows; verify no `ReceiveMaximumExceeded` error and backpressure works |

---

### `server_disconnect.rs` — §4.13.1 (3 tests)

| Test                                      | Behaviour verified                                                                                   |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `graceful_disconnect_reason_code_is_none` | Own `disconnect()` → `Disconnected(None)` (verifies reason code plumbing end-to-end)                 |
| `disconnect_with_keep_alive_too_large`    | Connect with `keep_alive=u16::MAX`; verify Mosquitto accepts or rejects with appropriate reason code |

---

### `keep_alive_advanced.rs` — §3.1.2.10 (2 tests)

| Test                                     | Behaviour verified                                                                      |
| ---------------------------------------- | --------------------------------------------------------------------------------------- |
| `zero_keep_alive_connection_stays_alive` | `keep_alive=0` (disabled), idle 3 s → connection still alive, no PINGREQ sent           |
| `traffic_resets_keep_alive_deadline`     | `keep_alive=2s`, publish at t=1.5 s → connection survives past t=2 s without disconnect |

---

## Summary

| File                     | Tests   | MQTT spec section  |
| ------------------------ | ------- | ------------------ |
| `will_messages.rs`       | 8       | §3.1.2.5-9, §4.3   |
| `retain.rs`              | 7       | §3.3.1.3, §4.1     |
| `topic_filters.rs`       | 7       | §4.7               |
| `session_advanced.rs`    | 5       | §3.1.2.4, §4.2.1.1 |
| `subscriptions.rs`       | 8       | §3.8-3.11          |
| `message_properties.rs`  | 6       | §3.3.2             |
| `qos2_advanced.rs`       | 4       | §4.3.3             |
| `server_disconnect.rs`   | 2       | §4.13.1            |
| `keep_alive_advanced.rs` | 2       | §3.1.2.10          |
| **New total**            | **49**  |                    |
| Existing tests           | 11      |                    |
| **Grand total**          | **~60** |                    |

---

## Constraints and Notes

- **No Docker locally:** Tests cannot be run locally during implementation.
  Implementation proceeds by code review and CI. The user will provide test
  results after branch push.
- **AUTH stretch goal:** Enhanced authentication (AUTH packet loop,
  server-initiated re-auth) is out of scope unless Mosquitto plugin setup proves
  straightforward.
- **Timing-sensitive tests:** Tests using `will_delay_interval`,
  `expiry_interval`, or `session_expiry` require real `tokio::time::sleep`. Keep
  all delays ≤3 s to avoid flaky CI. Use `#[tokio::test(start_paused = false)]`
  explicitly where wall-clock time matters.
- **Mosquitto semantics:** Overlapping subscription delivery count, shared
  subscription format (`$share/group/topic`), and `$SYS` topic filtering are
  Mosquitto-specific; tests document the expected Mosquitto 2 behavior.
- **Branch isolation:** All implementation work happens on a separate git branch
  and worktree from `master`.
- **Execution model:** Implementation uses subagent-driven development (parallel
  subagents per file group).
