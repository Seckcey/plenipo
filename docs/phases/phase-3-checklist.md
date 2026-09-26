# Phase 3 — Implementation Checklist

**Status:** complete — accepted by the owner (v0.4.0). See
[phase-3-acceptance-report.md](phase-3-acceptance-report.md).

Source: `ROLLOUT_PLAN.md`, Phase 3 — Provider Runtime Adapters: Codex and Claude Code.
Phase 2 accepted (v0.3.0, owner sign-off 2026-09-26). Owner approved starting Phase 3.

**Goal:** run real OpenAI Codex and Claude Code workers under Plenipo without relying on their
desktop GUIs.

## Design decisions (details in ADR-007)

- **Official non-interactive CLIs, one supervised process per turn.** Claude Code:
  `claude -p --output-format stream-json`. Codex: `codex exec --json` (the same invocation the
  official Codex SDK uses). No UI automation; the desktop apps need not be open.
- **Provider-neutral `RuntimeAdapter` contract** in `crates/runtime`; provider names appear only
  inside the two adapters. Output is normalized into common `AgentEvent`s and a `TurnResult`.
- **The UI never supplies a command, path, or flag.** It names a runtime, an objective (sent on
  **stdin**, never argv), an optional model name (strictly validated), and a session ID. Core
  builds the launch. Executables come only from detection rules and pass the allowlist at
  detection and again at spawn (extends ADR-005).
- **No credentials collected; no silent API billing.** Children get the OS baseline plus a short
  per-adapter pass-through list (config location, proxy/CA). API-key variables are never passed.
  A sign-in check runs before every turn: not signed in → refused with the official login command;
  API-key or third-party-cloud sign-in → refused (API fallback is disabled). Claude Code's
  reported credential source is checked again at the start of each turn's stream.
- **Least privilege until Guard (Phase 7).** Claude Code runs with no built-in tools and no MCP
  servers; Codex runs in its read-only sandbox. Each session works in its own empty workspace
  under Plenipo's app data, not in a project.
- **Sessions are durable.** A new `runtime_sessions` ledger table (migration 0002) keeps
  Plenipo's session ↔ provider session ID. Each turn is a ledger **task** with an **execution**
  (runtime, provider, model, session, usage), normalized activity events, and a final
  `agent.result` event. Turns left running by a previous session become `interrupted`.
- **Explicit failure outcomes**: not signed in, billing not allowed, usage limit, provider
  unavailable, malformed output, crash, cancelled, timed out. A usage limit never triggers a
  switch to another provider.

## Deliverables

- [x] Runtime adapter interface (`RuntimeAdapter`: detect installation, detect authentication,
      capabilities, start/resume session + submit task, stream events, cancel, close, normalize)
- [x] Codex adapter
- [x] Claude Code adapter
- [x] Provider detection (PATH + known install locations; Windows `.exe` only)
- [x] Authenticated-state detection (subscription vs API key vs signed out)
- [x] Session creation and session resume (provider session IDs preserved in the Ledger)
- [x] Streaming output (normalized live activity; raw provider output kept for diagnostics)
- [x] Cancellation (process-tree kill; session remains resumable)
- [x] Structured, normalized results
- [x] Provider diagnostics UI (installation, version, sign-in, capabilities, re-check)
- [x] Workers UI: start a task on a runtime, live activity, result, follow-up (resume), cancel,
      close session
- [x] Supervisor extensions: adapter-built launch specs, stdin, per-execution line observer,
      runtime-extendable allowlist, agent attribution on execution records
- [x] Ledger migration 0002 (`runtime_sessions`), repository, events, up/down tested
- [x] Fake provider CLIs for tests (`plenipo-fake-agent`) — no network or real account in CI
- [x] ADR-007; architecture, configuration, README updated

## Phase 3 tests (from plan, for each runtime)

Every test runs for **both** runtimes in `crates/runtime/tests/agents.rs` (the real session
service, supervisor, and adapters driving `plenipo-fake-agent` installed as `claude` / `codex`),
plus parser unit tests in `agent/claude_code.rs` and `agent/codex.rs`.

- [x] Installation detection — `installation_detection`, `missing_runtimes_are_reported_not_installed`,
      `windows_npm_shims_are_not_run` (Windows)
- [x] Unauthenticated behavior — `unauthenticated_runtimes_refuse_work_with_login_guidance`,
      `api_key_and_cloud_sign_ins_are_refused`, `unverifiable_sign_in_is_allowed_only_with_a_per_turn_billing_check`,
      `claude_code_turn_is_stopped_when_it_reports_api_billing`,
      `unconfirmed_sign_in_without_a_reported_credential_source_is_stopped`
- [x] Authenticated smoke task — fake CLIs in CI; **real CLIs: owner check below**
- [x] New session — `new_session_streams_activity_and_returns_a_normalized_result`
- [x] Resume same session — `resume_continues_the_same_provider_session`
- [x] Streamed output — live activity assertions in the new-session and cancel tests
- [x] Cancellation — `cancellation_stops_the_turn_and_the_session_stays_resumable`
- [x] Process crash — `failures_are_normalized` (`[crash]`)
- [x] Rate/usage-limit failure — `failures_are_normalized` (`[usage-limit]`),
      `usage_limited_session_can_be_resumed_later`
- [x] Malformed output — `failures_are_normalized` (`[malformed]`), `large_and_unknown_output_is_handled`
- [x] Provider unavailable — `provider_unavailable_after_detection_is_refused`, `failures_are_normalized` (`[offline]`)

Also: restart recovery (`restart_marks_unfinished_turns_interrupted`, E2E kill -9 mid-turn),
shutdown (`shutdown_stops_running_turns_and_records_them`), one turn per session and closing
(`close_and_follow_up_never_interleave`),
input validation, Ledger migration 0002 up/down and v1 → v2 upgrade, IPC boundary tests for
every new command, and E2E through the real app (`tests/e2e/specs/agents.e2e.mjs`).

## Acceptance criteria (from plan) — from the Plenipo UI

- [x] 1. Launch one Codex task
- [x] 2. Launch one Claude Code task
- [x] 3. See live activity
- [x] 4. Receive a normalized completion result
- [x] 5. Resume both sessions
- [x] 6. Cancel an active task
- [x] 7. Preserve the executions in the Ledger

The Codex and Claude desktop applications are not required to be open.

CI proves 1–7 end to end against fake CLIs that speak each provider's documented stream format
(`tests/e2e/specs/agents.e2e.mjs`). Criteria 1–7 with the **real** CLIs and the owner's own
subscription sign-ins are verified by the owner on Windows (as in Phase 2's O2), because CI has
no provider accounts.

### Owner check on Windows (~10 minutes)

1. Install and sign in to both CLIs (setup guide §3), then open Plenipo → **Runtimes** →
   **Re-check**. Both cards show **Ready**, a version, and "Signed in (subscription)".
2. **Workers** → Codex → objective "Say hello and tell me what folder you are in" → **Start
   task**. Live activity appears; the turn ends **Completed** with an answer. (Criteria 1, 3, 4.)
3. Same with Claude Code; text streams in while it answers. (Criteria 2, 3, 4.)
4. In each session, **Continue this session**: "What did I ask you before?" — the answer must
   refer to the first objective. (Criterion 5.)
5. Start a Claude Code task "Count slowly from 1 to 200, one number per line" and choose
   **Cancel turn** while it streams → **Cancelled**; then continue the session. (Criterion 6.)
6. **Activity** → open one of the tasks: its trail shows the turn, the provider session, the
   execution, and the result. Quit and relaunch Plenipo: Workers and Activity still show
   everything. (Criterion 7.) Neither desktop app needs to be open.

Things only a real CLI can confirm (report anything odd): the exact `claude auth status`
output shape, Claude Code's `apiKeySource` value for a subscription sign-in (`none` is
expected), Codex's `login status` wording, and that the npm-installed Codex's native binary is
found (`vendor/<target>/codex/codex.exe`).

## Out of scope

Automatic model selection, cross-agent delegation, Gemini/Grok/local models, API billing
fallback, production capabilities (file/shell/Git/network grants — Phase 7), projects as
working directories (Phase 7/8), secret redaction (Phase 7).
