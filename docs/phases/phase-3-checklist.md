# Phase 3 — Implementation Checklist

**Status:** in progress.

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

- [ ] Runtime adapter interface (`RuntimeAdapter`: detect installation, detect authentication,
      capabilities, start/resume session + submit task, stream events, cancel, close, normalize)
- [ ] Codex adapter
- [ ] Claude Code adapter
- [ ] Provider detection (PATH + known install locations; Windows `.exe` only)
- [ ] Authenticated-state detection (subscription vs API key vs signed out)
- [ ] Session creation and session resume (provider session IDs preserved in the Ledger)
- [ ] Streaming output (normalized live activity; raw provider output kept for diagnostics)
- [ ] Cancellation (process-tree kill; session remains resumable)
- [ ] Structured, normalized results
- [ ] Provider diagnostics UI (installation, version, sign-in, capabilities, re-check)
- [ ] Workers UI: start a task on a runtime, live activity, result, follow-up (resume), cancel,
      close session
- [ ] Supervisor extensions: adapter-built launch specs, stdin, per-execution line observer,
      runtime-extendable allowlist, agent attribution on execution records
- [ ] Ledger migration 0002 (`runtime_sessions`), repository, events, up/down tested
- [ ] Fake provider CLIs for tests (`plenipo-fake-agent`) — no network or real account in CI
- [ ] ADR-007; architecture, configuration, README updated

## Phase 3 tests (from plan, for each runtime)

- [ ] Installation detection
- [ ] Unauthenticated behavior
- [ ] Authenticated smoke task (fake CLI in CI; real CLI on the owner's machine)
- [ ] New session
- [ ] Resume same session
- [ ] Streamed output
- [ ] Cancellation
- [ ] Process crash
- [ ] Rate/usage-limit failure
- [ ] Malformed output
- [ ] Provider unavailable

## Acceptance criteria (from plan) — from the Plenipo UI

- [ ] 1. Launch one Codex task
- [ ] 2. Launch one Claude Code task
- [ ] 3. See live activity
- [ ] 4. Receive a normalized completion result
- [ ] 5. Resume both sessions
- [ ] 6. Cancel an active task
- [ ] 7. Preserve the executions in the Ledger

The Codex and Claude desktop applications are not required to be open.

CI proves 1–7 end to end against fake CLIs that speak each provider's documented stream format.
Criteria 1–7 with the **real** CLIs and the owner's own subscription sign-ins are verified by
the owner on Windows (as in Phase 2's O2), because CI has no provider accounts.

## Out of scope

Automatic model selection, cross-agent delegation, Gemini/Grok/local models, API billing
fallback, production capabilities (file/shell/Git/network grants — Phase 7), projects as
working directories (Phase 7/8), secret redaction (Phase 7).
