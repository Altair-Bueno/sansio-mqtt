# MQTT v5 Client Requirement Traceability

Scope: `crates/sansio-mqtt-v5-protocol`

| Requirement | Spec Ref | Code Path | Test Name | Status |
|-------------|----------|-----------|-----------|--------|
| Session Present=1 invalid with Clean Start=1 must close | MQTT-3.2.2-2 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `connack_resume_with_clean_start_is_protocol_error` | PASS |
| Session Present=0 non-resume discards local session state | MQTT-3.2.2-5 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `non_resumed_connack_discards_all_local_session_state` | PASS |
| Resume retransmits unacknowledged QoS publishes with DUP | MQTT-4.4.0-1, MQTT-4.4.0-2 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `resumed_session_replays_outbound_qos_publish_with_dup_set` | PASS |
| Resume retransmits unacknowledged PUBREL | MQTT-4.4.0-1 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `resumed_session_replays_unacknowledged_pubrel` | PASS |
| Shared subscription cannot set No Local=1 | MQTT-3.8.3-4 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `subscribe_shared_with_no_local_is_rejected` | PASS |
| Outbound QoS must not exceed server Maximum QoS | MQTT-3.2.2-9 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `publish_qos_above_server_maximum_qos_is_rejected` | PASS |
| Retained publish disallowed when Retain Available=0 | MQTT-3.2.2-14 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `publish_retain_when_server_retain_not_available_is_rejected` | PASS |
| Wildcard filters disallowed when server forbids wildcard subs | MQTT-3.2.2-15 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `subscribe_wildcard_when_server_disallows_is_rejected` | PASS |
| Shared filters disallowed when server forbids shared subs | MQTT-3.2.2-16 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `subscribe_shared_when_server_disallows_is_rejected` | PASS |
| Subscription Identifier disallowed when server forbids it | MQTT-3.2.2-17 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `subscribe_identifier_when_server_disallows_is_rejected` | PASS |
| Topic Alias inbound unknown alias is protocol error | MQTT-3.3.2-7 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `inbound_publish_alias_only_unknown_alias_is_protocol_error` | PASS |
| Topic Alias inbound alias max must be enforced | MQTT-3.2.2-17 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `inbound_publish_alias_exceeds_client_alias_max_is_protocol_error` | PASS |
| CONNECT encodes client Receive Maximum when configured | MQTT-3.1.2-11 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `connect_encodes_receive_maximum_when_configured` | PASS |
| CONNECT encodes client Maximum Packet Size when configured | MQTT-3.1.2-24 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `connect_encodes_maximum_packet_size_when_configured` | PASS |
| CONNECT Will maps QoS/Retain from options | MQTT-3.1.2-14 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `build_connect_packet_maps_will_qos_and_retain_from_options` | PASS |
| Server Keep Alive=0 must not panic and disable keepalive | MQTT-3.2.2-21 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `connack_server_keep_alive_zero_disables_keepalive_without_panic` | PASS |
| AUTH in connecting requires configured authentication and valid continuation reason | MQTT-4.12.0-1 | `crates/sansio-mqtt-v5-protocol/src/proto.rs` | `connecting_auth_without_configured_authentication_is_protocol_error`, `connecting_auth_with_reason_other_than_continue_is_protocol_error`, `connecting_auth_continue_then_connack_success_connects` | PASS |

Notes:
- This matrix tracks practical conformance items that are explicitly covered by tests in this repository.
- Remaining checklist gaps (session-expiry close-path semantics and final conformance report artifact) stay open until implemented.
