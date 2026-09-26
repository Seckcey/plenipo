# Architecture Overview

This document is the architectural contract for Plenipo. It describes what exists today
(through Phase 3) and the boundaries later phases must respect. Decisions behind it are in
[`docs/adr`](../adr/README.md); the delivery sequence is in [`ROLLOUT_PLAN.md`](../../ROLLOUT_PLAN.md).

## 1. Shape of the system

```
┌──────────────────────────── Plenipo Desktop (one process) ────────────────────────────┐
│                                                                                       │
│  WebView (untrusted UI)                    Rust (privileged)                          │
│  ┌──────────────────────┐   typed IPC      ┌──────────────────────────────────────┐   │
│  │ React + TypeScript   │ ───────────────▶ │ Tauri command layer (src-tauri)      │   │
│  │ src/api/commands.ts  │ ◀─────────────── │  - validates every input             │   │
│  │ (only IPC caller)    │   DTOs (JSON)    │  - gated by capabilities/*.json      │   │
│  └──────────────────────┘                  │           │                          │   │
│                                            │           ▼                          │   │
│                                            │ Plenipo Core (crates/core)           │   │
│                                            │  - provider-neutral domain + DTOs    │   │
│                                            │ Plenipo Ledger (crates/ledger)       │   │
│   ▲ events: plenipo://ledger               │  - SQLite system of record (WAL)     │   │
│                                            │  - append-only ordered event trail   │   │
│                                            │ Plenipo Runtime (crates/runtime)     │   │
│   ▲ events: plenipo://runtime              │  - Supervisor, profiles, policy      │   │
│   ▲ events: plenipo://agents               │  - agent runtimes: adapters (Claude  │   │
│   └────────────────────────────────────────│    Code, Codex), sessions, turns     │   │
│                                            │  - persists via Ledger               │   │
│                                            └──────────────┬───────────────────────┘   │
└───────────────────────────────────────────────────────────┼───────────────────────────┘
                                                            │ spawns approved profiles and
                                                            │ adapter-built turns only
                                          ┌─────────────────▼─────────────────┐
                                          │ child process tree                │
                                          │ (Job Object / process group,      │
                                          │  cleared env, stdin = objective,  │
                                          │  stdout/stderr piped)             │
                                          └───────────────────────────────────┘
```

## 2. Trust boundary

The **WebView is untrusted**. It renders UI and asks Core to do things. It never executes OS
operations itself. This is enforced by:

1. **Explicit command manifest.** `src-tauri/build.rs` declares every app command. A command
   not listed there cannot be invoked.
2. **Capability grants.** `src-tauri/capabilities/default.json` grants the `main` window
   `core:default` plus each app command by name. There are no filesystem, shell, HTTP, or
   process plugins installed.
3. **Single IPC client.** `apps/desktop/src/api/commands.ts` is the only module allowed to
   call `invoke` (ESLint `no-restricted-imports`).
4. **Content Security Policy.** `tauri.conf.json` restricts scripts to `'self'` and network
   connections to Tauri IPC.
5. **Input validation in Rust.** Commands validate their arguments and return a typed
   `CommandError`; they never panic on bad input.

`apps/desktop/src-tauri/src/lib.rs` (`ipc_boundary_tests`) verifies this against the real
capability configuration: granted commands succeed from `main`; unknown commands, OS plugin
commands, remote origins, and windows without a grant are all rejected.

Later phases add privileged operations (process supervision, filesystem, Git, …) **only** as
validated Core operations behind this boundary, and later still, behind Plenipo Guard.

## 3. Shared DTOs

DTOs are defined once in Rust (`crates/core/src/dto.rs`) with `serde` (camelCase on the wire)
and `ts-rs`. `pnpm bindings` regenerates `packages/types/src/generated/*.ts`; CI fails if the
committed bindings differ from what the Rust code produces. The frontend imports types only
from `@plenipo/types`.

Current commands:

| Command                  | Input                              | Returns              | Purpose                                                    |
| ------------------------ | ---------------------------------- | -------------------- | ---------------------------------------------------------- |
| `get_app_info`           | —                                  | `AppInfo`            | Name, version, build profile, OS, arch                     |
| `frontend_ready`         | —                                  | `()`                 | UI signals successful render; only acts in smoke-test mode |
| `get_runtime_overview`   | —                                  | `RuntimeOverview`    | Profiles, executions (newest first), active count, notices |
| `start_execution`        | `profileId`                        | `ExecutionRecord`    | Launch an **approved profile**; no command or path input   |
| `cancel_execution`       | `executionId`                      | `ExecutionRecord`    | Terminate the process tree; returns the final record       |
| `get_execution_output`   | `executionId`                      | `ExecutionOutput`    | Buffered output, used to rebuild the view after a reload   |
| `get_ledger_status`      | —                                  | `LedgerStatus`       | Location, schema, counts, notices, last check/backup       |
| `list_tasks`             | —                                  | `Task[]`             | Tasks, newest first                                        |
| `get_task_timeline`      | `taskId`                           | `TaskTimeline`       | A task's complete ordered trail and direct children        |
| `list_recent_events`     | —                                  | `LedgerEvent[]`      | Most recent ledger events, newest first                    |
| `create_synthetic_task`  | —                                  | `Task`               | Diagnostics: create a synthetic task                       |
| `advance_synthetic_task` | `taskId`, `action`                 | `Task`               | Diagnostics: act on a **synthetic** task only              |
| `run_integrity_check`    | —                                  | `IntegrityReport`    | Full SQLite integrity + foreign-key check                  |
| `create_ledger_backup`   | —                                  | `BackupInfo`         | Verified backup; **Core chooses the path**                 |
| `export_ledger`          | —                                  | `ExportInfo`         | JSON export; **Core chooses the path**                     |
| `get_agent_overview`     | —                                  | `AgentOverview`      | Agent runtimes (install, sign-in, capabilities), sessions  |
| `refresh_agent_runtimes` | —                                  | `AgentRuntimeInfo[]` | Re-detect installation and sign-in                         |
| `get_agent_session`      | `sessionId`                        | `AgentSessionDetail` | A session's turns and recent live activity                 |
| `start_agent_session`    | `runtimeId`, `objective`, `model?` | `AgentSessionDetail` | New session + first turn; objective goes to **stdin**      |
| `resume_agent_session`   | `sessionId`, `objective`           | `AgentSessionDetail` | Next turn in the same provider session                     |
| `cancel_agent_turn`      | `sessionId`                        | `AgentSessionDetail` | Kill the running turn's tree; resolves once recorded       |
| `close_agent_session`    | `sessionId`                        | `AgentSession`       | No further turns                                           |

Events (Rust → UI): `plenipo://runtime` carries `RuntimeEvent`
(`{ kind: "output", executionId, lines[] }` batched and `seq`-ordered, or
`{ kind: "lifecycle", record }`); `plenipo://ledger` carries each committed `LedgerEvent`;
`plenipo://agents` carries `AgentUpdate` (`activity`, `turn`, `session`, or `runtimes`).
The frontend subscribes only through `src/api/events.ts`.

All commands return `Result<T, CommandError>`; the TS client converts rejections into
`PlenipoCommandError { kind, message }`.

## 4. Runtime supervisor (Phase 1)

Decision record: [ADR-005](../adr/ADR-005-runtime-supervisor.md).

- **Profiles, not commands.** The UI names a profile ID; the profile (Rust only) fixes the
  executable, args, working directory, declared env vars, and max runtime.
- **Allowlist** of canonical executable paths, checked at registration and at spawn.
- **Cleared environment**: OS baseline + declared variables only.
- **Process-tree ownership**: Windows Job Object (kill-on-close), Unix process group. Cancel,
  timeout, and shutdown kill the whole tree.
- **Lifecycle**: UUID per launch; `starting → running → succeeded | failed | cancelled |
timedOut`; `interrupted` for runs left active by a previous session.
- **Output**: 8 KiB line cap, 1,000-line ring buffer, batched events (50 ms / 200 lines).
- **Shutdown**: tray Quit, last-window close, SIGTERM/SIGINT → graceful shutdown. Closing the
  window with work running hides to the tray.
- **UI state** lives above the views, so navigation never loses it; after a webview reload it
  is rebuilt from `get_runtime_overview` + `get_execution_output` and deduplicated by `seq`.

Phase 1 ships only diagnostic profiles that run Plenipo itself with
`--plenipo-diagnostic=<scenario>` (handled in `main()` before Tauri starts).

## 5. Ledger (Phase 2)

Decision record: [ADR-006](../adr/ADR-006-ledger.md).

- SQLite at `<local app data>/ledger/plenipo.db`: WAL, `synchronous=FULL`, foreign keys.
- Entities: roles, departments, projects, agent instances, tasks (parent/child), events,
  executions (runtime/provider/model/session/usage), approvals, artifacts.
- Every mutation writes its event in the same transaction; `events` is append-only (triggers)
  and globally ordered. Rejected task transitions are recorded.
- Task states: `queued → running → blocked | awaitingApproval → running → succeeded | failed |
cancelled` (terminal states are final).
- Forward-only migrations with checksums and a verified pre-migration backup.
- Corruption: quick check on open → quarantine + fresh ledger + prominent notice.
- Backups (`VACUUM INTO`, verified, keep 10) and JSON export.

## 6. Agent runtimes (Phase 3)

Decision record: [ADR-007](../adr/ADR-007-runtime-adapters.md).

- **Contract.** `RuntimeAdapter` (`crates/runtime/src/agent/adapter.rs`) is provider-neutral:
  detection, sign-in check, capabilities, turn arguments (new or resumed provider session),
  a stream parser producing normalized `AgentEvent`s, and a normalized `TurnResult`. Vendor
  names appear only in `agent/claude_code.rs` and `agent/codex.rs`.
- **Surface.** The official non-interactive CLIs: `claude -p --output-format stream-json` and
  `codex exec --json`. One turn = one supervised execution; the objective is written to stdin.
- **Boundary.** The UI names a runtime ID, an objective, an optional (validated) model, and a
  session ID. Executables come only from detection (PATH + known install locations; Windows
  `.exe` only), are allowlisted by Core, and re-checked at spawn.
- **Billing.** Sign-in is checked before every turn with the CLI's own status command; signed
  out, API-key, and third-party-cloud sign-ins are refused. Claude Code's reported credential
  source is checked again in each stream. API-key variables are never passed to children.
- **Least privilege.** Claude Code: no tools, no MCP servers. Codex: read-only sandbox. Each
  session has its own empty workspace. Capabilities arrive with Guard (Phase 7).
- **Sessions.** `runtime_sessions` (migration 0002) maps Plenipo's session to the provider
  session ID. Each turn is a task (`metadata.sessionId`) with an execution, `agent.*` activity
  events, and an `agent.result` event written together with the task's final state.
- **Outcomes.** `completed`, `failed`, `cancelled`, `timedOut`, `usageLimited`,
  `authRequired`, `billingNotAllowed`, `providerUnavailable`, `malformedOutput`, `crashed`,
  `interrupted`. A usage limit never switches provider. Turns running when Plenipo stopped are
  recorded as interrupted on the next start.
- **Tests** run against `plenipo-fake-agent`, a test double that speaks both stream formats.

## 7. Launch smoke test

With `PLENIPO_SMOKE_TEST=1`, the app launches normally, the UI calls `frontend_ready` once it
has rendered **and** successfully called Core, and the process exits 0. If that does not
happen within `PLENIPO_SMOKE_TIMEOUT_SECS` (default 60) a watchdog exits 1. The outcome is
tracked in shared state rather than trusting the runtime's exit-code propagation, which is not
reliable on every platform. CI runs this against the release build on Windows.

## 8. Target component map

From the rollout plan. **Desktop**, **Core**, **Runtime** (supervisor and agent runtime
adapters), and **Ledger** exist today.

| Component    | Responsibility                                 | Introduced |
| ------------ | ---------------------------------------------- | ---------- |
| Desktop      | UI                                             | Phase 0    |
| Core         | Orchestration and domain logic, shared DTOs    | Phase 0    |
| Runtime      | Supervisor ✅, Codex / Claude Code adapters ✅ | Phase 1, 3 |
| Ledger       | SQLite system of record ✅                     | Phase 2    |
| Liaison      | Task/message/event bus                         | Phase 4    |
| Workforce    | Departments, roles, coordinators, workers      | Phase 5    |
| Router       | Role → provider/model selection                | Phase 6    |
| Capabilities | Filesystem, shell, Git, SSH, browser, MCP      | Phase 7    |
| Guard        | Permissions, approvals, policy enforcement     | Phase 7    |
| Vault        | Credential references (OS-protected storage)   | Phase 7    |
| Integrations | Paperclip, GitHub, CrewOS                      | Phase 8+   |

## 9. Invariants every phase must keep

- **Local-first** ([ADR-002](../adr/ADR-002-local-first-architecture.md)): the desktop app owns
  execution; remote surfaces never become the privileged runtime.
- **Provider-independent roles** ([ADR-003](../adr/ADR-003-provider-independent-roles.md)): no
  vendor names in Core types; provider specifics live in adapters.
- **No credentials to launch.** Launching Plenipo must never require provider keys.
- **No secrets in source control, logs, or prompts.**
- **Capabilities enforced in code**, not described in prompts.
- **Every long-running operation has an ID and a cancel path** (from Phase 1 onward).
