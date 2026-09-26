# Phase 3 — Acceptance Report (draft, pending owner verification)

|              |                                                                                                                                                                                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 3 — Provider Runtime Adapters: Codex and Claude Code                                                                                                                                                                                                    |
| **Branch**   | `claude/phase-3-runtime-adapters` ([PR #3](https://github.com/Seckcey/plenipo/pull/3); replaces #2)                                                                                                                                                     |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, full `pnpm e2e`. GitHub CI **green** on `9fdd498`: Rust, Frontend, E2E (Linux), Windows (tests, installer, launch smoke) — [run](https://github.com/Seckcey/plenipo/actions/runs/36219759501). |
| **Date**     | 2026-09-26                                                                                                                                                                                                                                              |
| **Result**   | **All acceptance criteria pass end to end against fake CLIs.** The same criteria with the **real** Claude Code and Codex sign-ins are owner item **O2**.                                                                                                |

Screenshots: [agent runtimes](evidence/phase-3/agent-runtimes.png) ·
[live turn](evidence/phase-3/worker-live.png) · [result](evidence/phase-3/worker-result.png) ·
[ledger trail](evidence/phase-3/worker-ledger-trail.png).

Test totals: **225 Rust** (Linux; Windows adds the npm-shim test) · **46 frontend** · **20
end-to-end** against the real release binary (6 Phase 1 + 6 Phase 2 + 8 Phase 3).

CI has no provider accounts, so every automated test drives `plenipo-fake-agent`, a test double
installed as `claude` / `codex` that speaks each CLI's documented stream format (`stream-json`,
`exec --json`), its version and sign-in status commands, session storage, and resume.

## 1. Acceptance criteria → evidence

| #   | Criterion (from the Plenipo UI)        | Result (fake CLIs) | Evidence                                                                                                                                                                                                                                                                                                                                          |
| --- | -------------------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1  | Launch one Codex task                  | **Pass**           | E2E `A1+A3+A4: launches a Codex task…` (Workers → Codex → Start task). Integration `new_session_streams_activity_and_returns_a_normalized_result` (codex).                                                                                                                                                                                        |
| A2  | Launch one Claude Code task            | **Pass**           | E2E `A2+A3+A4: launches a Claude Code task…`. Integration test (claude-code). Plenipo chooses the provider session ID (`--session-id`).                                                                                                                                                                                                           |
| A3  | See live activity                      | **Pass**           | E2E: streamed text ticks visible while the turn is still running ([screenshot](evidence/phase-3/worker-live.png)); Codex command and message in the turn's activity. Integration: live `TextDelta`s arrive before completion.                                                                                                                     |
| A4  | Receive a normalized completion result | **Pass**           | Both runtimes produce the same `TurnResult` (outcome, text, provider session, model, token usage, duration). Parser unit tests for success, error results, usage limits, sign-in errors, crashes, malformed and unknown output.                                                                                                                   |
| A5  | Resume both sessions                   | **Pass**           | E2E `A5: resumes both sessions…`: turn 2 answers with the turn-1 objective ("Previous: …"). Integration `resume_continues_the_same_provider_session` checks `--resume <id>` / `resume <thread>` with the confirmed provider ID.                                                                                                                   |
| A6  | Cancel an active task                  | **Pass**           | E2E `A3+A6`: Cancel turn → **Cancelled**, then the session is resumed. Integration `cancellation_stops_the_turn_and_the_session_stays_resumable` (both runtimes); the execution is `cancelled`.                                                                                                                                                   |
| A7  | Preserve the executions in the Ledger  | **Pass**           | E2E `A7`: the task's trail shows creation, provider session, execution running/succeeded, agent message, and the result ([screenshot](evidence/phase-3/worker-ledger-trail.png)); raw output listed in Runtimes. E2E kill -9 mid-turn → relaunch: the turn is **Interrupted**, all earlier sessions and results intact. Migration v1 → v2 tested. |
| —   | Desktop apps not required to be open   | **Pass**           | Only the CLIs run, as supervised child processes; no UI automation exists.                                                                                                                                                                                                                                                                        |

## 2. Required Phase 3 tests → evidence (each for both runtimes)

| Test (ROLLOUT_PLAN.md)   | Result   | Evidence (`crates/runtime/tests/agents.rs` unless noted)                                                                                                                     |
| ------------------------ | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Installation detection   | **Pass** | `installation_detection` (version, canonical executable), `missing_runtimes_are_reported_not_installed`, `windows_npm_shims_are_not_run`; Codex npm launcher → native binary |
| Unauthenticated behavior | **Pass** | `unauthenticated_runtimes_refuse_work_with_login_guidance` (refused, login command shown, nothing recorded); API-key/cloud refused; E2E signed-out + direct IPC call refused |
| Authenticated smoke task | **Pass** | Fake CLIs (above). **Real CLIs: O2**                                                                                                                                         |
| New session              | **Pass** | `new_session_streams_activity_and_returns_a_normalized_result`                                                                                                               |
| Resume same session      | **Pass** | `resume_continues_the_same_provider_session`, `usage_limited_session_can_be_resumed_later`                                                                                   |
| Streamed output          | **Pass** | Live activity assertions; E2E live ticks                                                                                                                                     |
| Cancellation             | **Pass** | `cancellation_stops_the_turn_and_the_session_stays_resumable`, `shutdown_stops_running_turns_and_records_them`                                                               |
| Process crash            | **Pass** | `failures_are_normalized` `[crash]` → `crashed` with the CLI's stderr                                                                                                        |
| Rate/usage-limit failure | **Pass** | `[usage-limit]` → `usageLimited`; session stays open, no provider switch; resumable later                                                                                    |
| Malformed output         | **Pass** | `[malformed]` → `malformedOutput`; unknown event types counted, not fatal; 1 MiB lines parse                                                                                 |
| Provider unavailable     | **Pass** | CLI removed after detection → refused before running; `[offline]` → `providerUnavailable`                                                                                    |

Also: Claude Code reporting API billing mid-stream is stopped within seconds
(`claude_code_turn_is_stopped_when_it_reports_api_billing`); restart recovery; one turn per
session; input validation; IPC boundary tests for all 7 new commands (ungranted windows and
remote origins denied, smuggled `executable`/`args` fields rejected).

## 3. Defects found and fixed during Phase 3

| Found by           | Problem                                                                                                                                                                                       | Fix                                                                                                                                              |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| E2E (intermittent) | For a fast turn, the `start` command's response (a snapshot taken as the turn began) arrived after the live "finished" update and marked the session **Running** again, disabling follow-ups. | The UI merges snapshots without moving facts backwards (finished turns, confirmed provider session, turn count, closed state); regression tests. |
| Query plan review  | The per-session turn count scanned the task index instead of seeking it (TEXT affinity on `s.id` blocked the expression index).                                                               | Compare with `+s.id`; verified with `EXPLAIN QUERY PLAN` (SEARCH).                                                                               |
| Ledger tests       | Migration tests used a synthetic "version 2", which collides with the real migration 0002.                                                                                                    | Synthetic migration is now one past the newest real one; a real v1 → v2 upgrade test was added.                                                  |
| E2E                | A test read the previously selected session before the new one was shown.                                                                                                                     | Wait for the new session's title before reading its turns.                                                                                       |
| Code review        | Selecting a session could show only the turns that live updates had delivered (e.g. after a webview reload), never its full history.                                                          | The view fetches a session's history whenever it is selected; the store tracks which sessions are fully loaded. Tests added.                     |
| Code review        | If Claude Code omitted its credential source while the sign-in check was inconclusive, the per-turn billing check passed silently.                                                            | Without a confirmed subscription, the stream must report a subscription credential before anything else, or the turn is stopped. Tests added.    |
| Code review        | A slow Ledger writer could hold up reading a turn's output past the 2 s drain window, cutting off the final result.                                                                           | The output observer is unbounded, so reading never waits on it. Regression test with a deliberately slow observer.                               |
| Code review        | Close and a follow-up submitted at the same moment could start a turn on a just-closed session.                                                                                               | Both claim the session exclusively before acting; a follow-up re-reads the session after claiming it. Race test (10 iterations).                 |
| Parallel tests     | Copying the fake CLI while other tests forked could fail with `Text file busy` (Linux).                                                                                                       | Copy once per test process, wait until it runs, hard-link per test.                                                                              |

## 4. Deliverables

| Deliverable                             | Location                                                                                    |
| --------------------------------------- | ------------------------------------------------------------------------------------------- |
| Runtime adapter interface               | `crates/runtime/src/agent/adapter.rs` (`RuntimeAdapter`, `TurnParser`, normalization rules) |
| Codex adapter                           | `crates/runtime/src/agent/codex.rs`                                                         |
| Claude Code adapter                     | `crates/runtime/src/agent/claude_code.rs`                                                   |
| Provider detection, authenticated state | `agent/discovery.rs` + each adapter's `parse_auth`; preflight before every turn             |
| Session creation / resume               | `agent/service.rs` (`AgentRuntime`), Ledger `runtime_sessions` (`migrations/0002_*`)        |
| Streaming output, cancellation          | Supervisor `LaunchSpec` (stdin, line observer), `AgentUpdate` on `plenipo://agents`         |
| Structured results                      | `TurnResult`, `agent.result` event written with the task's final state (`complete_task`)    |
| Provider diagnostics UI                 | Runtimes → Agent runtimes (`components/AgentRuntimeCards.tsx`)                              |
| Workers UI                              | `views/WorkersView.tsx`, `agents/*`                                                         |
| Test double                             | `crates/runtime/src/bin/plenipo-fake-agent.rs`                                              |
| Decision record                         | [ADR-007](../adr/ADR-007-runtime-adapters.md)                                               |

## 5. Security notes

- The UI supplies only a runtime ID, an objective (sent on **stdin**), an optional model name
  matching a strict pattern, and session IDs. Every argument is built by Core; executables come
  only from detection and pass the allowlist at detection and again at spawn.
- No credentials are collected or read. API-key and cloud-provider variables are never passed
  to the CLIs (tested); account identifiers from sign-in checks are never stored or shown.
- API billing is refused up front (API-key / third-party-cloud sign-ins) and, for Claude Code,
  again during every turn from the credential source the CLI reports.
- Phase 3 workers get no capabilities: Claude Code has no tools or MCP servers; Codex runs in
  its read-only sandbox. Each session has its own empty workspace.
- Agent text in activity events is capped but not redacted; redaction is Phase 7.

## 6. Deviations from the plan

| Deviation                                                                           | Why                                                                                            | Recorded         |
| ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | ---------------- |
| Codex via `codex exec --json`, not app-server                                       | Same surface the official Codex SDK uses; app-server's mid-turn approvals are Phase 7 material | ADR-007 §1, alt. |
| Least-privilege posture (no tools / read-only sandbox) instead of provider defaults | Capabilities are Phase 7; Phase 3 must not grant any                                           | ADR-007 §5       |
| Sessions in a new `runtime_sessions` table, not `agent_instances`                   | Agent instances are organizational workers (Phase 5); sessions are provider mechanics          | ADR-007 §6, alt. |
| Windows runs only native `.exe` CLIs                                                | npm `.cmd` shims pass input through `cmd.exe`                                                  | ADR-007 §3       |

## 7. Owner items

| ID  | Item                                                                                                                                                                                             | Recommendation                                           |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------- |
| O1  | ADR-007 is **Proposed**.                                                                                                                                                                         | Accept or amend.                                         |
| O2  | Windows check with the **real** CLIs and your subscription sign-ins (~10 min): the steps in [phase-3-checklist.md](phase-3-checklist.md#owner-check-on-windows-10-minutes). Report anything odd. | Required for acceptance (criteria A1–A7 with real CLIs). |
| O3  | Version stays **0.3.1** until O2 passes; then **0.4.0** per the phase convention.                                                                                                                | Bump after O2.                                           |
| O4  | A repository rule rejected pushes to the branch mid-session (work moved to PR #3).                                                                                                               | **Resolved** — owner updated the rule; CI ran green.     |
| O5  | Phase 4 (Liaison message bus, cross-provider handoffs) lets agents hand work to each other — a material expansion (plan §8.12).                                                                  | Say "start Phase 4" after O1–O2.                         |

Things only the real CLIs can confirm (O2): the exact `claude auth status --json` fields,
Claude Code's `apiKeySource` for a subscription sign-in (`none` expected — anything else stops
the turn as "API billing"), Codex's `login status` wording, the npm-installed Codex native binary
path, and whether Claude Code on Windows needs `CLAUDE_CODE_GIT_BASH_PATH` set.

## 8. Phase boundary

Phase 3 is implemented and verified against fake CLIs. It is **not accepted** until O1–O2.
