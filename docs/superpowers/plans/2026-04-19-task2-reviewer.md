# Task 2 Bake-off Reviewer (Implementation Plan)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Produce a concise written report comparing branches `task2a-proto-statig` and `task2b-proto-plain`, then recommend a winner. Report only — no code changes, no diffs.

**Architecture:** Read both worktrees (or both branches from the same clone), collect metrics, render a structured comparison, and end with a recommendation + reasoning.

---

## Context

- Worktrees (created by orchestrator):
  - `../sansio-mqtt-task2a-statig` — branch `task2a-proto-statig`
  - `../sansio-mqtt-task2b-plain` — branch `task2b-proto-plain`
- Spec: `docs/superpowers/specs/2026-04-19-parallel-docs-and-refactor-design.md`.
- Task 2A plan: `docs/superpowers/plans/2026-04-19-task2a-proto-refactor-statig.md`
- Task 2B plan: `docs/superpowers/plans/2026-04-19-task2b-proto-refactor-plain.md`

## Constraints

- **No code or test changes on either branch.** Read-only.
- **No diffs in the output.** The orchestrator wants a written report only.
- Write the final report to: `docs/superpowers/plans/2026-04-19-task2-reviewer-report.md` (on a new branch `task2-reviewer-report` branched from `master`, committed, ready for the orchestrator to read).
- Report length target: 400-900 words total.

---

## Task 1: Gather metrics for both branches

For EACH branch, run from inside the matching worktree and record:

- [ ] `cargo fmt --check` — pass/fail
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — pass/fail, warning count
- [ ] `cargo test --workspace` — pass/fail, test count
- [ ] `cargo build -p sansio-mqtt-v5-tokio` — pass/fail (contract: downstream compiles)
- [ ] `wc -l crates/sansio-mqtt-v5-protocol/src/**/*.rs` — file LOC table
- [ ] Total LOC in `crates/sansio-mqtt-v5-protocol/src/`
- [ ] Largest file (path, LOC)
- [ ] Largest function (heuristic: `cargo clippy -p sansio-mqtt-v5-protocol -- -W clippy::too_many_lines 2>&1 | grep too_many`)
- [ ] Cognitive complexity warnings (`cargo clippy -p sansio-mqtt-v5-protocol -- -W clippy::cognitive_complexity 2>&1 | grep cognitive`)
- [ ] Max nesting depth (sample via `grep -c "^                " src/**/*.rs` — not perfect but directional)
- [ ] Number of source files
- [ ] Dependency delta vs. `master` (diff of `Cargo.toml` and `Cargo.lock` — for 2B this should be empty; for 2A this should show `statig`)

## Task 2: Qualitative read

For each branch, read the top-level module layout and the `sansio::Protocol` impl. Record:

- [ ] **Clarity:** can you tell which state a given packet handler runs in without reading other files? Rate 1-5.
- [ ] **State machine legibility:** can you enumerate every allowed transition from one file? Rate 1-5.
- [ ] **Boilerplate overhead:** how much ceremony does the chosen approach impose (macro DSL surface for `statig`, trait impls, etc.)?
- [ ] **Friction for future edits** (adding a new packet type, new state, new transition): describe in 2 sentences.

Keep each rating justified by a one-sentence reference to a specific file or function.

## Task 3: Public API check

For each branch, verify:

- [ ] `git diff master -- 'crates/sansio-mqtt-v5-protocol/src/lib.rs' 'crates/sansio-mqtt-v5-protocol/src/types.rs'` produces no semantic changes to public re-exports or trait signatures.
- [ ] `git diff master -- 'crates/sansio-mqtt-v5-tokio/**/*.rs'` is empty.

If either branch fails this, flag as **DISQUALIFYING**.

## Task 4: Write the report

Target path (on a fresh branch `task2-reviewer-report` from `master`):
`docs/superpowers/plans/2026-04-19-task2-reviewer-report.md`

Structure:

```markdown
# Task 2 Bake-off Review

**Date:** 2026-04-19
**Reviewer:** Claude subagent
**Branches compared:**
- `task2a-proto-statig` (commit `<short-sha>`)
- `task2b-proto-plain`  (commit `<short-sha>`)

## 1. Gate checks

| Check                         | 2A       | 2B       |
|-------------------------------|----------|----------|
| `cargo fmt --check`           | pass/fail | pass/fail |
| `cargo clippy -D warnings`    | pass/fail | pass/fail |
| `cargo test --workspace`      | N passed / M total | … |
| `cargo build -p …-tokio`      | pass/fail | pass/fail |
| Public API unchanged          | yes/no   | yes/no   |

If either branch fails any gate, stop and flag disqualification.

## 2. Metrics

| Metric                                | 2A       | 2B       |
|---------------------------------------|----------|----------|
| Total LOC in `src/`                   | …        | …        |
| Number of source files                | …        | …        |
| Largest file (path, LOC)              | …        | …        |
| Largest function (path:line, LOC)     | …        | …        |
| `clippy::too_many_lines` warnings     | …        | …        |
| `clippy::cognitive_complexity` warnings| …       | …        |
| New workspace deps                    | statig   | none     |

## 3. Qualitative assessment

### Clarity
…

### State-machine legibility
…

### Boilerplate overhead
…

### Friction for future edits
…

## 4. Recommendation

**Winner: 2A | 2B**

Reasoning (3-5 sentences): …

### Risks to carry forward
- …

### What the winner should NOT do in Part 2/3
- …
```

## Task 5: Commit and report

- [ ] **Step 1: Commit the report**

```
git checkout -b task2-reviewer-report master
git add docs/superpowers/plans/2026-04-19-task2-reviewer-report.md
git commit -m "docs(reviewer): task2 bake-off report"
```

- [ ] **Step 2: Tell the orchestrator**

Return the final recommendation (2A or 2B) plus the committed report path to the orchestrator.

---

## Self-review checklist

- [ ] Both branches actually compared (not just one).
- [ ] Gate checks run on both; any failure flagged.
- [ ] All metric rows filled in.
- [ ] Recommendation is unambiguous.
- [ ] No code or test changes made on 2A or 2B branches.
- [ ] Report is under ~900 words.
