# Architecture Overview

This document is the architectural contract for Plenipo. It describes what exists today
(through Phase 6) and the boundaries later phases must respect. Decisions behind it are in
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
│                                            │ Plenipo Liaison (crates/liaison)     │   │
│                                            │  - handoffs between workers: checks, │   │
│                                            │    child tasks, replies, all in the  │   │
│                                            │    Ledger; reconciles from it        │   │
│                                            │ Plenipo Workforce (crates/workforce) │   │
│                                            │  - organization: positions, teams,   │   │
│                                            │    oversight; places role: handoffs  │   │
│                                            │    through Liaison's directory hook  │   │
│                                            │ Plenipo Router (crates/router)       │   │
│                                            │  - model registry, role policies,    │   │
│                                            │    explained choice of AI tool/model │   │
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

| Command                  | Input                                           | Returns              | Purpose                                                                                         |
| ------------------------ | ----------------------------------------------- | -------------------- | ----------------------------------------------------------------------------------------------- |
| `get_app_info`           | —                                               | `AppInfo`            | Name, version, build profile, OS, arch                                                          |
| `frontend_ready`         | —                                               | `()`                 | UI signals successful render; only acts in smoke-test mode                                      |
| `get_runtime_overview`   | —                                               | `RuntimeOverview`    | Profiles, executions (newest first), active count, notices                                      |
| `start_execution`        | `profileId`                                     | `ExecutionRecord`    | Launch an **approved profile**; no command or path input                                        |
| `cancel_execution`       | `executionId`                                   | `ExecutionRecord`    | Terminate the process tree; returns the final record                                            |
| `get_execution_output`   | `executionId`                                   | `ExecutionOutput`    | Buffered output, used to rebuild the view after a reload                                        |
| `get_ledger_status`      | —                                               | `LedgerStatus`       | Location, schema, counts, notices, last check/backup                                            |
| `list_tasks`             | —                                               | `Task[]`             | Tasks, newest first                                                                             |
| `get_task_timeline`      | `taskId`                                        | `TaskTimeline`       | A task's complete ordered trail and direct children                                             |
| `list_recent_events`     | —                                               | `LedgerEvent[]`      | Most recent ledger events, newest first                                                         |
| `create_synthetic_task`  | —                                               | `Task`               | Diagnostics: create a synthetic task                                                            |
| `advance_synthetic_task` | `taskId`, `action`                              | `Task`               | Diagnostics: act on a **synthetic** task only                                                   |
| `run_integrity_check`    | —                                               | `IntegrityReport`    | Full SQLite integrity + foreign-key check                                                       |
| `create_ledger_backup`   | —                                               | `BackupInfo`         | Verified backup; **Core chooses the path**                                                      |
| `export_ledger`          | —                                               | `ExportInfo`         | JSON export; **Core chooses the path**                                                          |
| `get_agent_overview`     | —                                               | `AgentOverview`      | Agent runtimes (install, sign-in, capabilities), sessions                                       |
| `refresh_agent_runtimes` | —                                               | `AgentRuntimeInfo[]` | Re-detect installation and sign-in                                                              |
| `get_agent_session`      | `sessionId`                                     | `AgentSessionDetail` | A session's turns and recent live activity                                                      |
| `start_agent_session`    | `runtimeId`, `objective`, `model?`, `handoffs?` | `AgentSessionDetail` | New session + first turn; objective goes to **stdin**; `handoffs` lets the worker use Liaison   |
| `resume_agent_session`   | `sessionId`, `objective`                        | `AgentSessionDetail` | Next turn in the same provider session (not for handoff workers)                                |
| `cancel_agent_turn`      | `sessionId`                                     | `AgentSessionDetail` | Kill the running turn's tree, or end a turn waiting for handoff replies; resolves once recorded |
| `close_agent_session`    | `sessionId`                                     | `AgentSession`       | No further turns                                                                                |
| `get_task_handoffs`      | `taskId`                                        | `TaskHandoffs`       | The handoff that created a task and those it made, with replies                                 |
| `get_task_tree`          | `taskId`                                        | `TaskTree`           | The task's whole delegation tree, depth-first from its root                                     |
| `get_liaison_overview`   | —                                               | `LiaisonOverview`    | Protocol, limits, destinations, open handoffs, notices                                          |

Workforce commands (Phase 5). Every change returns the organization as it is afterwards
(`OrgSnapshot`); the Ledger enforces the structure and a refused change rejects with the reason.

| Command                   | Input                            | Returns              | Purpose                                                                                                 |
| ------------------------- | -------------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------- |
| `get_organization`        | —                                | `OrgSnapshot`        | Roles, departments, projects, positions with live status and workers, oversight, stats                  |
| `get_work`                | `positionId?`                    | `WorkView`           | A position's running, waiting, queued, and recent tasks, and its team's unfinished tasks                |
| `rename_organization`     | `name`                           | `OrgSnapshot`        | The organization's display name                                                                         |
| `set_organization_titles` | `titles` (`TitleTheme`)          | `OrgSnapshot`        | What the app calls the ranks (display only; ADR-010)                                                    |
| `create_role`             | `input` (`RoleInput`)            | `OrgSnapshot`        | A custom role (class and staffing)                                                                      |
| `create_department`       | `input` (`DepartmentInput`)      | `OrgSnapshot`        | A department with its head position (and agent, unless left vacant)                                     |
| `update_department`       | `departmentId`, `input`          | `OrgSnapshot`        | Name, description, active                                                                               |
| `remove_department`       | `departmentId`                   | `OrgSnapshot`        | Delete a department without projects; its head position is archived                                     |
| `create_project`          | `input` (`ProjectInput`)         | `OrgSnapshot`        | A project in a department with its coordinator; allowed runtimes, recorded path/profile                 |
| `update_project`          | `projectId`, `input`             | `OrgSnapshot`        | Settings (allowed runtimes are checked against every position under the project)                        |
| `archive_project`         | `projectId`                      | `OrgSnapshot`        | Archive the project and its whole team (refused while any of it has unfinished work)                    |
| `hire_position`           | `input` (`HireInput`)            | `OrgSnapshot`        | A new position under a lead (or the owner); a persistent one gets its agent; no `runtimeId`: automatic  |
| `fill_position`           | `positionId`                     | `OrgSnapshot`        | Hire an agent into a vacant persistent position                                                         |
| `vacate_position`         | `positionId`                     | `OrgSnapshot`        | Retire a persistent position's agent; the position stays                                                |
| `update_position`         | `positionId`, `input`            | `OrgSnapshot`        | Title, runtime (`""`: automatic), model (a new runtime or model hires a new agent for a persistent one) |
| `move_position`           | `positionId`, `reportsTo`        | `OrgSnapshot`        | Change who it reports to (`null`: the owner); a moved coordinator takes its project along               |
| `archive_position`        | `positionId`                     | `OrgSnapshot`        | Archive (orphan prevention: no reports, not a head or coordinator, no unfinished work)                  |
| `assign_oversight`        | `overseerId`, `targetId`, `role` | `OrgSnapshot`        | Make an on-demand position a lead's team reviewer, QA evaluator, or security auditor                    |
| `end_oversight`           | `oversightId`                    | `OrgSnapshot`        | End an oversight assignment                                                                             |
| `give_objective`          | `positionId`, `objective`        | `AgentSessionDetail` | Give a staffed persistent position's agent an objective; Core chooses its session                       |

Router commands (Phase 6). Every change returns the model settings as they are afterwards
(`RoutingSnapshot`); a refused change rejects with the reason and changes nothing.

| Command               | Input                        | Returns           | Purpose                                                                                   |
| --------------------- | ---------------------------- | ----------------- | ----------------------------------------------------------------------------------------- |
| `get_routing`         | —                            | `RoutingSnapshot` | Models, AI tools (sign-in, usage limits), every role's policy and next route, models seen |
| `save_model`          | `input` (`ModelInput`)       | `RoutingSnapshot` | Add a model (no `id`) or change one; model names are validated like every model name      |
| `remove_model`        | `modelId`                    | `RoutingSnapshot` | Remove an owner's model; it leaves every role's list (built-in entries stay)              |
| `set_role_policy`     | `roleId`, `policy`           | `RoutingSnapshot` | Replace a role's model policy                                                             |
| `set_routing_options` | `options` (`RoutingOptions`) | `RoutingSnapshot` | What a usage limit does: wait (default) or use the next choice                            |
| `clear_usage_limit`   | `runtimeId`                  | `RoutingSnapshot` | Try an AI tool again now, although it reported a usage limit                              |

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

## 7. Liaison (Phase 4)

Decision record: [ADR-008](../adr/ADR-008-liaison.md).

- **Workers never control each other.** In a session the owner started with handoffs allowed,
  a worker asks for help by ending its answer with fenced `plenipo-handoff` JSON blocks
  (`to`, `objective`, `acceptanceCriteria`, `context`, `artifacts`, `capabilities`,
  `priority`; protocol `plenipo-liaison/1`). Liaison parses them as untrusted input: unknown
  or identity fields (sender, IDs, correlation) are refused; the sender is whoever Plenipo's
  own records say is running that turn.
- **Destinations are runtimes** (`claude-code`, `codex`, or `runtime:<id>`) for sessions the
  owner starts in Workers, and **roles** (`role:<title>`) for organization members, resolved
  by the Workforce directory (§8). Another worker's session can never be addressed. A request
  never falls back to another provider.
- **One transaction per decision.** At the end of the answer's step, the step's result, each
  request (accepted with a queued child task, or refused with a reply that says why), and the
  requester's move to `blocked` are written together (`liaison_messages`, migration 0003,
  immutable except for state). A duplicate block in one answer creates one child; replaying the
  same answer creates nothing.
- **Handoff workers** are new sessions on the destination runtime, with the same least-
  privilege posture as any worker. They receive a context packet (`plenipo-context/1`): the
  objective, acceptance criteria, and only the context the requester referenced (its answer,
  excerpts, or tasks and artifacts of the same workflow), capped and delimited with a nonce the
  requester cannot know. Capability requests are recorded and never granted before Guard.
- **Replies** carry the child's normalized result. When all of a task's replies are in, the
  task continues as a new step of the same turn in the same provider session. Replies to a
  task that stopped waiting are discarded, never delivered.
- **Correlation.** Each owner objective starts a workflow with a new correlation ID; every
  child task, message, and `liaison.*` event carries it. Replies must match their request's
  correlation and child, or they are refused and recorded.
- **Reconciliation.** Liaison reacts to Ledger events (and a 2 s tick): answer finished
  children, cancel handoffs whose requester ended (stopping their workers, down the tree),
  dispatch accepted handoffs when a worker slot is free, deliver replies, retire finished
  handoff workers. Every action is guarded by recorded state, so repeating it changes nothing.
- **Limits.** Depth 3, 3 requests per answer, 5 reply rounds per task, 12 handoffs per
  workflow. Beyond a limit the request is refused and the worker told to do it itself.
- **Restarts.** Waiting and running turns are recorded as interrupted on the next start; their
  handoffs are answered or cancelled, and nothing is resumed automatically.

## 8. Workforce (Phase 5)

Decision records: [ADR-009](../adr/ADR-009-workforce.md) (the engine) and
[ADR-010](../adr/ADR-010-plain-titles.md) (the words on screen).

- **Words on screen.** The app shows the owner as President and the superintendent, department
  manager, and project coordinator as VP, Manager, and Supervisor, with "AI tool" for runtime
  and "full-time" / "on call" for persistent / on-demand
  ([word list](../design/vocabulary.md)). The code keeps the names used below. The owner's
  `TitleTheme` (stored in the organization's settings, returned in `OrgSnapshot.titles`) swaps
  the rank names for a U.S. military branch's or the Mafia's in the UI only
  (`apps/desktop/src/org/titles.ts`); agents always get the plain titles. Seeded role templates
  carry their former names, and seeding renames such a role in place (`org.role_renamed`).

- **Positions and agents.** A position is a place in the organization chart (title, role,
  supervisor, runtime, optional model). A _persistent_ position (superintendent, department
  manager, project coordinator, or a custom persistent role) is held by one agent at a time and
  keeps one conversation (a runtime session found by `workforce.agentId`), so it survives
  restarts; it can be vacant. An _on-demand_ position spawns a new agent instance for every
  task handed to it; that worker retires when its task ends.
- **Structure in the Ledger** (migration 0004: `positions`, `oversight`, `settings`, and new
  columns on departments, projects, and agent instances). Every rule is checked in the
  transaction that changes the structure: no cycles; only persistent positions supervise;
  department heads report to the owner or a superintendent; a coordinator reports to its
  department's head (moving it reassigns the project); titles are unique within a team; nothing
  that leads, heads, coordinates, or has unfinished work can be archived. Department and project
  membership is computed from the tree, so it cannot disagree with the reporting lines. Every
  change writes an `org.*` event.
- **Workers follow their task.** The task state machine starts and retires a task's worker in
  the same transaction as the task's own state change (`org.worker_started`,
  `org.worker_retired`), so no ending path can leave a worker in the active workforce; its
  history remains.
- **Teams through Liaison.** The owner gives objectives to staffed persistent positions only.
  Workforce starts or resumes the agent's session through Liaison with the member's identity and
  team. A member hands work to its team — its on-demand reports plus the positions assigned to
  oversee it — with `role:<title>`; Liaison's `Directory` hook (implemented by Workforce)
  places the request: the worker is recorded with the child task in one transaction and runs on
  the position's runtime and model. Unknown roles, raw runtime addresses from members, and
  runtimes the project does not allow are refused with the reason.
- **Policy.** Projects record allowed runtimes (explicit; none allows none), a local folder, and
  a capability profile name; the folder and profile are recorded only — no capability is granted
  before Guard (Phase 7). A position's runtime is either fixed by the owner or automatic (chosen
  by the Router, §9). Delegation between persistent positions waits for Phase 8.
- **Organization canvas** (`apps/desktop/src/org`, `components/org`): a topology map in the
  style of a network topology view — owner → organization → teams, left to right, with bus
  connectors, labelled link chips, collapse toggles, live status (a dot plus text), animated
  links where work is running, dotted oversight links, controls, and a minimap. Layout and camera
  are pure, tested modules (camera adapted from the owner's Coastline plan canvas). Nodes are
  focusable buttons over an SVG link layer. Hiring (drag a role from the palette onto a lead),
  reassigning, and assigning oversight (drag a position onto a lead) use pointer events only,
  and each has a keyboard path through the details panel and dialogs; a filterable list view
  complements the map. The view reloads the snapshot (debounced) on `org.*`, `task.*`,
  `liaison.*`, and `session.*` Ledger events and on runtime readiness changes.

## 9. Router (Phase 6)

Decision record: [ADR-011 (how Plenipo picks each worker's AI model)](../adr/ADR-011-model-policy-routing.md).

- **Words on screen.** Settings → **AI models**: "model choices" (policy), "first choice" and
  "backups" (fallback order), "Automatic" (a position following its role's policy), "AI company"
  (provider).
- **Configuration** is the Ledger's `routing` setting: the model registry, a policy per role,
  options, and usage-limit clears, changed atomically and recorded as `router.*` events.
- **Model Registry.** Each AI tool's default model (built in) plus the owner's models: the name
  the tool accepts, the owner's name for it, what it can do, context size, cost class. Models the
  tools reported running are listed as "seen in use"; no model name is assumed.
- **Role policy.** Ordered models (first choice, then fallbacks), required capabilities, minimum
  context, AI companies never used, cost preference (orders the registry when no models are
  listed), cross-company review (off, prefer, require). API billing is off: a tool signed in with
  an API key is never chosen.
- **Engine** (`crates/router/src/engine.rs`, pure): candidates in order → cross-company
  reordering → per-model checks (tool present, company allowed, project allows the tool,
  capabilities, context, subscription sign-in, usage limit) → the first that passes, with one
  plain explanation and a verdict per model. With "wait" (default), a usage limit never moves work
  to another AI company.
- **Effort.** Each adapter lists the effort levels its CLI accepts and passes the chosen one on
  every turn (Claude Code `--effort`, Codex `-c model_reasoning_effort=`). A model has an optional
  effort; a role can set its own for any model. The decision carries it, the reason says it, and
  the runtime session stores it (Ledger schema 5) so a conversation keeps it.
- **Model menus.** Every place a model is chosen offers the AI tool's default, the models its
  adapter lists (`known_models`: the CLI's own aliases or model picker, each with the effort
  levels it accepts), the owner's models, and models seen in use, with a typed name as a last
  resort; none is added to the registry on the owner's behalf.
- **Usage limits** come from the Ledger's turn results (reported reset time, or an hour; lifted by
  a later success or "try again now"), so they survive restarts.
- **Where it applies.** Automatic on-call positions: every handoff, in the directory, recorded
  with the worker (`workforce.routing`, `org.worker_spawned`); unroutable requests are refused with
  the reason. Automatic full-time positions: when the agent's conversation starts
  (`org.agent_routed`); it keeps that AI tool for the conversation. Fixed positions keep the
  owner's AI tool, explained as such. Positions from before Phase 6 are fixed.
- **Shown** as "Auto · <AI tool>" on the map, "Why this AI model" (with every model considered) in
  the details panel, a worker's reason, the reason in the activity trail, and each role's next
  worker in Settings.

## 10. Launch smoke test

With `PLENIPO_SMOKE_TEST=1`, the app launches normally, the UI calls `frontend_ready` once it
has rendered **and** successfully called Core, and the process exits 0. If that does not
happen within `PLENIPO_SMOKE_TIMEOUT_SECS` (default 60) a watchdog exits 1. The outcome is
tracked in shared state rather than trusting the runtime's exit-code propagation, which is not
reliable on every platform. CI runs this against the release build on Windows.

## 11. Target component map

From the rollout plan. **Desktop**, **Core**, **Runtime** (supervisor and agent runtime
adapters), **Ledger**, **Liaison**, **Workforce**, and **Router** exist today.

| Component    | Responsibility                                 | Introduced |
| ------------ | ---------------------------------------------- | ---------- |
| Desktop      | UI                                             | Phase 0    |
| Core         | Orchestration and domain logic, shared DTOs    | Phase 0    |
| Runtime      | Supervisor ✅, Codex / Claude Code adapters ✅ | Phase 1, 3 |
| Ledger       | SQLite system of record ✅                     | Phase 2    |
| Liaison      | Task/message/event bus ✅                      | Phase 4    |
| Workforce    | Departments, roles, coordinators, workers ✅   | Phase 5    |
| Router       | Role → provider/model selection ✅             | Phase 6    |
| Capabilities | Filesystem, shell, Git, SSH, browser, MCP      | Phase 7    |
| Guard        | Permissions, approvals, policy enforcement     | Phase 7    |
| Vault        | Credential references (OS-protected storage)   | Phase 7    |
| Integrations | Paperclip, GitHub, CrewOS                      | Phase 8+   |

## 12. Invariants every phase must keep

- **Local-first** ([ADR-002](../adr/ADR-002-local-first-architecture.md)): the desktop app owns
  execution; remote surfaces never become the privileged runtime.
- **Provider-independent roles** ([ADR-003](../adr/ADR-003-provider-independent-roles.md)): no
  vendor names in Core types; provider specifics live in adapters.
- **No credentials to launch.** Launching Plenipo must never require provider keys.
- **No secrets in source control, logs, or prompts.**
- **Capabilities enforced in code**, not described in prompts.
- **Every long-running operation has an ID and a cancel path** (from Phase 1 onward).
