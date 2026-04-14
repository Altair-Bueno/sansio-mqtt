# AGENTS.md Index and Spec Gate Design

Date: 2026-04-14
Topic: AGENTS.md updates (index, spec gate, default execution mode)

## Goal
Add a short index and key paths list to AGENTS.md, enforce a mandatory spec check with the MQTT v5.0 URL, and declare subagent-driven execution as the default mode for work in this repo.

## Scope
- Update AGENTS.md in the worktree with an Index section (TOC + Key Paths).
- Require agents to read the MQTT v5.0 spec before implementation and include the URL.
- Mirror the spec gate in the mandatory checklist.
- Add default execution mode guidance (subagent-driven).

Out of scope: code changes, tests, or build configuration updates.

## Proposed AGENTS.md Changes

### 1. Index (new)
- Add a short Index section near the top.
- Include a Table of Contents with links to each section.
- Add a Key Paths list: `Cargo.toml`, `crates/*`, `rust-toolchain.toml`, `AGENTS.md`.

### 2. Spec Gate (new MUST)
- In the checklist: require agents to read the MQTT v5.0 spec before any implementation.
- In Protocol and Spec Compliance: repeat the requirement, include the exact URL:
  `https://docs.oasis-open.org/mqtt/mqtt/v5.0/mqtt-v5.0.html`.
- Emphasize conformance citations use `[MQTT-x.x.x-y]`.

### 3. Default Execution Mode
- Add a line under Contribution Workflow: “Default execution mode: subagent-driven.”

## Trade-offs
- Slightly longer AGENTS.md but easier navigation and stronger compliance enforcement.

## Success Criteria
- Agents can quickly navigate to key sections and files.
- Spec URL is visible and the “must read before implementation” rule is unambiguous.
- Default execution mode is clearly stated.

## Open Questions
None.
