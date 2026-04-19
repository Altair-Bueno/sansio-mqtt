# MQTT v5 Client Conformance Report

Date: 2026-04-18
Scope: `crates/sansio-mqtt-v5-protocol` (with dependency validation in `crates/sansio-mqtt-v5-types`)

## Result

- Practical conformance closure status: PASS for all currently tracked checklist items except one release-process artifact note (this file now provides that artifact).

## Verification Commands and Outcomes

1. `cargo test -p sansio-mqtt-v5-protocol --test client_protocol`
   - Result: PASS (`77 passed; 0 failed`)
2. `cargo test -p sansio-mqtt-v5-protocol`
   - Result: PASS (`5 unit tests + 77 integration tests + doc tests`)
3. `cargo test -p sansio-mqtt-v5-types`
   - Result: PASS (`3 unit + 46 mirror + 103 compatibility + doc tests`)
4. `cargo fmt`
   - Result: PASS
5. `cargo clippy -p sansio-mqtt-v5-protocol --all-targets`
   - Result: PASS for `sansio-mqtt-v5-protocol`; warnings remain in `sansio-mqtt-v5-types` and pre-existing large-enum/style advisories.

## Coverage Notes

- Requirement-level mapping is documented in:
  - `docs/superpowers/checklists/2026-04-17-mqtt-v5-client-requirement-traceability.md`
- Session-expiry close-path semantics covered by new tests:
  - `zero_session_expiry_clears_inflight_on_socket_closed`
  - `keepalive_timeout_with_session_expiry_preserves_inflight_for_resume`

## Open Risks / Follow-up

- `clippy` warnings in `sansio-mqtt-v5-types` remain and are not regressions from this closure pass.
- Additional strict formal conformance certification (beyond current practical checklist/test matrix) may still require expanded external interoperability runs.
