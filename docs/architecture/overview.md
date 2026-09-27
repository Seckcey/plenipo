# Architecture Overview

This document is the architectural contract for Plenipo. It describes what exists today
(through Phase 12 and v1.8) and the boundaries later phases must respect. Decisions behind it are in
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
│   └────────────────────────────────────────│    Code, Codex, Grok, Kimi, Ollama)  │   │
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
│                                            │ Plenipo Guard (crates/guard)         │   │
│                                            │  - permission sets, rules, decisions │   │
│                                            │ Capabilities (crates/capabilities)   │   │
│                                            │  - broker: grants, tool server,      │   │
│                                            │    approvals, file/program/git work, │   │
│                                            │    Vault (OS credential store)       │   │
│                                            │    working copies, GitHub via gh     │   │
│                                            │  - Plenipo's browser (DevTools, own  │   │
│                                            │    profile), screen, mouse, keyboard │   │
│                                            │  - SSH to the owner's servers        │   │
│                                            │    (russh, pinned server IDs)        │   │
│   ▲ events: plenipo://control              │  - control center: sign, Stop, Take  │   │
│                                            │    over                              │   │
│                                            │    ▲ 127.0.0.1, per-step ticket      │   │
│                                            └────┼─────────────┬───────────────────┘   │
└─────────────────────────────────────────────────┼─────────────┼───────────────────────┘
                                                  │             │ spawns approved profiles,
                                                  │             │ adapter-built turns, and
                                                  │             │ Guard-approved programs only
                                                  │   ┌─────────▼─────────────────────────┐
                                                  │   │ child process tree                │
                                                  │   │ (Job Object / process group,      │
                                                  │   │  cleared env, stdin = objective,  │
                                                  │   │  stdout/stderr piped)             │
                                                  │   │   AI tool ──stdio MCP──▶ relay    │
                                                  └───┼── (plenipo-desktop                │
                                                      │    --plenipo-tools=<ticket file>) │
                                                      └───────────────────────────────────┘
```

## 2. Trust boundary

The **WebView is untrusted**. It renders UI and asks Core to do things. It never executes OS
operations itself. This is enforced by:

1. **Explicit command manifest.** `src-tauri/build.rs` declares every app command. A command
   not listed there cannot be invoked.
2. **Capability grants.** `src-tauri/capabilities/default.json` grants the `main` window
   `core:default` plus each app command by name. There are no filesystem, shell, HTTP, or
   process plugins installed. The one plugin, `tauri-plugin-notification` (Phase 12), is used
   from Rust only: its own commands are granted to no window.
3. **Single IPC client.** `apps/desktop/src/api/commands.ts` is the only module allowed to
   call `invoke` (ESLint `no-restricted-imports`).
4. **Content Security Policy.** `tauri.conf.json` restricts scripts to `'self'` and network
   connections to Tauri IPC.
5. **Input validation in Rust.** Commands validate their arguments and return a typed
   `CommandError`; they never panic on bad input.

`apps/desktop/src-tauri/src/lib.rs` (`ipc_boundary_tests`) verifies this against the real
capability configuration: granted commands succeed from `main`; unknown commands, OS plugin
commands, remote origins, and windows without a grant are all rejected.

A third window, the **control indicator** (`control-indicator`, shown above all windows while a
worker uses the mouse and keyboard), has its own grant (`capabilities/indicator.json`): it may
read who has control, stop all control, and take over. Nothing else.

Privileged operations (process supervision, filesystem, Git, …) exist **only** as validated
Core operations behind this boundary. Workers reach files, programs, and git only through
Plenipo Guard (§10); the UI configures Guard and answers approvals, and can never run a tool
itself.

A second, narrower door serves workers: Plenipo's tool server listens on `127.0.0.1` only and
admits a connection only with the ticket of a step that is running now (§10).

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

| Command                   | Input                                   | Returns              | Purpose                                                                                                         |
| ------------------------- | --------------------------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------- |
| `get_organization`        | —                                       | `OrgSnapshot`        | Roles, departments, projects, positions with live status and workers, oversight, stats                          |
| `get_work`                | `positionId?`                           | `WorkView`           | A position's running, waiting, queued, and recent tasks, and its team's unfinished tasks                        |
| `rename_organization`     | `name`                                  | `OrgSnapshot`        | The organization's display name                                                                                 |
| `set_organization_titles` | `titles` (`TitleTheme`)                 | `OrgSnapshot`        | What the app calls the ranks (display only; ADR-010)                                                            |
| `create_role`             | `input` (`RoleInput`)                   | `OrgSnapshot`        | A custom role (class and staffing)                                                                              |
| `update_role`             | `roleId`, `input` (`RoleUpdate`)        | `OrgSnapshot`        | Change a role you created: name, what it does, and its working instructions (ADR-019); built-in roles refused   |
| `create_department`       | `input` (`DepartmentInput`)             | `OrgSnapshot`        | A department with its head position (and agent, unless left vacant)                                             |
| `update_department`       | `departmentId`, `input`                 | `OrgSnapshot`        | Name, description, active                                                                                       |
| `remove_department`       | `departmentId`                          | `OrgSnapshot`        | Delete a department without projects; its head position is archived                                             |
| `create_project`          | `input` (`ProjectInput`)                | `OrgSnapshot`        | A project in a department with its coordinator; allowed runtimes, recorded path/profile                         |
| `update_project`          | `projectId`, `input`                    | `OrgSnapshot`        | Settings (allowed runtimes are checked against every position under the project)                                |
| `archive_project`         | `projectId`                             | `OrgSnapshot`        | Archive the project and its whole team (refused while any of it has unfinished work)                            |
| `hire_position`           | `input` (`HireInput`)                   | `OrgSnapshot`        | A new position under a lead (or the owner); a persistent one gets its agent; no `runtimeId`: automatic          |
| `fill_position`           | `positionId`                            | `OrgSnapshot`        | Hire an agent into a vacant persistent position                                                                 |
| `vacate_position`         | `positionId`                            | `OrgSnapshot`        | Retire a persistent position's agent; the position stays                                                        |
| `update_position`         | `positionId`, `input`                   | `OrgSnapshot`        | Title, runtime (`""`: automatic), model (a new runtime or model hires a new agent for a persistent one)         |
| `move_position`           | `positionId`, `reportsTo`               | `OrgSnapshot`        | Change who it reports to (`null`: the owner); a moved coordinator takes its project along                       |
| `archive_position`        | `positionId`                            | `OrgSnapshot`        | Archive (orphan prevention: no reports, not a head or coordinator, no unfinished work)                          |
| `assign_oversight`        | `overseerId`, `targetId`, `role`        | `OrgSnapshot`        | Make an on-demand position a lead's team reviewer, QA evaluator, or security auditor                            |
| `end_oversight`           | `oversightId`                           | `OrgSnapshot`        | End an oversight assignment                                                                                     |
| `give_objective`          | `positionId`, `objective`, `projectId?` | `AgentSessionDetail` | Give a staffed persistent position's agent an objective (for a project its team runs); Core chooses its session |

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

Guard commands (Phase 7). Settings changes return `PermissionsSnapshot` as it is afterwards;
Guard's input types reject unknown fields, and a refused change rejects with the reason.

| Command                 | Input                                          | Returns               | Purpose                                                                                    |
| ----------------------- | ---------------------------------------------- | --------------------- | ------------------------------------------------------------------------------------------ |
| `get_permissions`       | —                                              | `PermissionsSnapshot` | Sets, who has which, rules, secret references (never values), grants in use, recent blocks |
| `save_permission_set`   | `input` (`PermissionSetInput`)                 | `PermissionsSnapshot` | Add a permission set (no `id`) or change one                                               |
| `remove_permission_set` | `setId`                                        | `PermissionsSnapshot` | Remove a set nothing uses (built-in sets stay)                                             |
| `assign_permissions`    | `target` (`role`/`department`), `id`, `setId?` | `PermissionsSnapshot` | A role's set (it grants) or a department's limit (it narrows); no `setId`: none / no limit |
| `set_command_rules`     | `rules` (`CommandRules`)                       | `PermissionsSnapshot` | Approved, always-ask, and never-run command lists                                          |
| `set_blocked_files`     | `patterns`                                     | `PermissionsSnapshot` | Files no worker may open (gitignore-style)                                                 |
| `set_sensitive_rule`    | `kind`, `rule` (`ask`/`block`)                 | `PermissionsSnapshot` | What a kind of sensitive action does; never "allow"                                        |
| `set_guard_options`     | `options` (`GuardOptions`)                     | `PermissionsSnapshot` | How long an approval waits (1–60 minutes)                                                  |
| `save_secret`           | `input` (`SecretInput`)                        | `PermissionsSnapshot` | Store a secret: the value goes to the OS credential store, only the reference to Plenipo   |
| `remove_secret`         | `secretId`                                     | `PermissionsSnapshot` | Remove a secret's value and its reference                                                  |
| `get_approvals`         | —                                              | `ApprovalQueue`       | Approvals waiting (oldest first) and recent outcomes                                       |
| `resolve_approval`      | `approvalId`, `approve`                        | `ApprovalQueue`       | Approve or refuse one waiting request                                                      |
| `revoke_grant`          | `grantId`                                      | `PermissionsSnapshot` | End a worker's permissions now: stop its programs, refuse its waiting requests             |

A project's permission limit is part of its settings (`create_project` / `update_project`
`capabilityProfile`), checked against the permission sets.

Development commands (Phase 8). A project's branch setting is part of its settings
(`ProjectInput.branchPerObjective`).

| Command                | Input                        | Returns           | Purpose                                                                                                        |
| ---------------------- | ---------------------------- | ----------------- | -------------------------------------------------------------------------------------------------------------- |
| `set_up_development`   | `input` (`DevelopmentInput`) | `OrgSnapshot`     | The Development department and its VP (when missing), then a project with its Supervisor and the standard team |
| `get_objective_report` | `taskId`                     | `ObjectiveReport` | Plenipo's result for an objective (any of its tasks), built from the Ledger                                    |
| `get_project_work`     | `projectId`                  | `ProjectWork`     | A project's objectives (newest first, in brief) and working copies                                             |
| `remove_workspace`     | `workspaceId`                | `ProjectWork`     | Remove a finished objective's working copy; its branch stays                                                   |

Browser and computer commands (Phase 10). The website lists are part of Guard's settings.

| Command              | Input                    | Returns               | Purpose                                                                                                   |
| -------------------- | ------------------------ | --------------------- | --------------------------------------------------------------------------------------------------------- |
| `get_control_status` | —                        | `ControlStatus`       | Who uses Plenipo's browser or the mouse and keyboard now, and whether control is stopped                  |
| `stop_all_control`   | —                        | `ControlStatus`       | The emergency Stop: every session halts, those workers' permissions end, no new control until allowed     |
| `take_over_control`  | `sessionId`              | `ControlStatus`       | The owner takes one session (`browser:<grant>`, `desktop:<grant>`, or `server:<grant>`): its worker stops |
| `allow_control`      | —                        | `ControlStatus`       | Allow control again after a Stop                                                                          |
| `set_website_rules`  | `rules` (`WebsiteRules`) | `PermissionsSnapshot` | Allowed and blocked websites, and what other websites do (ask or blocked)                                 |
| `get_browser_status` | —                        | `BrowserStatus`       | Which browser Plenipo uses, its profile folder, whether it is open                                        |
| `open_browser`       | `url?`                   | `BrowserStatus`       | Open Plenipo's browser for the owner (to sign in to a website workers will use)                           |
| `get_screenshot`     | `artifactId`             | `Screenshot`          | A kept screenshot as a `data:` URL (only files in Plenipo's screenshot folder recorded in the Ledger)     |

Server commands (Phase 11). Servers are part of Guard's settings; their sign-ins are in the Vault.

| Command                 | Input                   | Returns           | Purpose                                                                                           |
| ----------------------- | ----------------------- | ----------------- | ------------------------------------------------------------------------------------------------- |
| `get_servers`           | —                       | `ServersSnapshot` | Settings → Servers: each server, whether its sign-in is stored, identity changes, who may connect |
| `save_server`           | `input` (`ServerInput`) | `ServersSnapshot` | Add or change a server; a key, passphrase, or password goes straight to the Vault                 |
| `remove_server`         | `id`                    | `ServersSnapshot` | Remove a server and its stored sign-in                                                            |
| `check_server_identity` | `host`, `port`          | `ServerIdentity`  | Read a server's ID (host key fingerprint) for the owner to compare and pin                        |
| `test_server`           | `id`                    | `ServerTest`      | Check the pinned server ID and sign in, running nothing                                           |

Switches and learning commands (v1.4). The switches are part of Guard's settings
(`PermissionsSnapshot.settings.switches`); learning is a Workforce setting.

| Command             | Input                       | Returns               | Purpose                                                                                                    |
| ------------------- | --------------------------- | --------------------- | ---------------------------------------------------------------------------------------------------------- |
| `set_switches`      | `switches` (`Switches`)     | `PermissionsSnapshot` | Settings → Switches. Switching the browser or the screen off also stops the workers using it now           |
| `get_learning`      | —                           | `LearningSnapshot`    | Worker learning on or off, the roles that learn on their own, lessons waiting (oldest first) and kept      |
| `set_learning`      | `enabled`                   | `LearningSnapshot`    | Worker learning on or off                                                                                  |
| `set_role_learning` | `roleId`, `auto`            | `LearningSnapshot`    | Whether a role learns on its own (its lessons are kept without asking, except from websites or the screen) |
| `decide_lesson`     | `lessonId`, `keep`, `text?` | `LearningSnapshot`    | Keep (in the owner's words, if given) or discard a waiting lesson                                          |
| `remove_lesson`     | `lessonId`                  | `LearningSnapshot`    | Remove a kept lesson                                                                                       |

Events (Rust → UI): `plenipo://runtime` carries `RuntimeEvent`
(`{ kind: "output", executionId, lines[] }` batched and `seq`-ordered, or
`{ kind: "lifecycle", record }`); `plenipo://ledger` carries each committed `LedgerEvent`;
`plenipo://agents` carries `AgentUpdate` (`activity`, `turn`, `session`, or `runtimes`);
`plenipo://control` carries `ControlStatus` (with a `revision`, so the UI ignores an older one).
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
  executions (runtime/provider/model/session/usage), approvals, artifacts, and (schema 6,
  Phase 8) workspaces: each objective's working copy and branch.
- Every mutation writes its event in the same transaction; `events` is append-only (triggers)
  and globally ordered. Rejected task transitions are recorded.
- Task states: `queued → running → blocked | awaitingApproval → running → succeeded | failed |
cancelled` (terminal states are final).
- Forward-only migrations with checksums and a verified pre-migration backup.
- Corruption: quick check on open → quarantine + fresh ledger + prominent notice.
- Backups (`VACUUM INTO`, verified, keep 10) and JSON export.
- **Activity over time** (schema 8, Phase 12A): `Ledger::activity(scopes, from, to, buckets)`
  counts events into fixed time buckets (with problems and requests for approval apart) for
  everything, a department, a project, or a position and its team; the index
  `events_by_created` keeps it fast. Desktop command `get_activity`; see
  [the design system](../design/design-system.md#activity-over-time).

## 6. Agent runtimes (Phase 3)

Decision records: [ADR-007](../adr/ADR-007-runtime-adapters.md) (how Plenipo runs Claude Code
and Codex), [ADR-015](../adr/ADR-015-acp-ai-tools.md) (running AI tools over ACP), and
[ADR-027](../adr/ADR-027-acp-file-access-through-plenipo.md) (Kimi over ACP, with its file reads
and writes going through Plenipo).

- **Contract.** `RuntimeAdapter` (`crates/runtime/src/agent/adapter.rs`) is provider-neutral:
  detection, sign-in check, capabilities, turn arguments (new or resumed provider session),
  a stream parser producing normalized `AgentEvent`s, and a normalized `TurnResult`. Vendor
  names appear only in `agent/claude_code.rs`, `agent/codex.rs`, `agent/grok.rs`,
  `agent/kimi.rs`, and `agent/ollama/`.
- **Surface.** The official non-interactive CLIs: `claude -p --output-format stream-json` and
  `codex exec --json`. One turn = one supervised execution; the objective is written to stdin.
- **Tasks that talk (ADR-015).** Grok's one-task mode cannot read stdin, so it runs
  `grok agent --no-leader stdio` and talks ACP (JSON-RPC, one message per line) through the
  shared driver `agent/acp.rs`: `initialize`, `session/new` or `session/resume`, then the prompt,
  all on stdin. The parser's `open()` gives the first lines; the supervisor keeps stdin open
  (`StdinFeed`) for the lines each `Parsed` sends, and closes it when the task is over. The
  driver answers the tool's permission requests (Plenipo's tool server yes, anything else no)
  and never asks it to sign in. Cancel asks the tool to stop (`session/cancel`) for up to five
  seconds before the process tree is ended.
- **File access through Plenipo (ADR-027).** Kimi (`kimi acp`) uses the same driver, with the
  options its own unswitchable tools need. `initialize` offers file reads and writes, so Kimi
  asks Plenipo for every file (`fs/read_text_file`, `fs/write_text_file`). The driver turns each
  into a `FileRequest` (`Parsed::files`); the service carries it out through
  `ToolProvider::file_access` — the broker's `read_file` / `write_file` path, so Guard decides,
  secrets are hidden, and the use is recorded (`capability.used` with `fileRequest: true`) — and
  gives the answer back with `TurnParser::file_answered` while the task goes on. A worker without
  a grant has every file refused. Kimi's own shell is refused, its file changes are allowed once
  only for a worker whose grant offers `write_file` (a change it reports done that never came to
  Plenipo stops the task), and nothing is approved for a whole session. Its mode, model, and
  thinking level are set with `session/set_config_option` before the prompt and checked; a mode
  other than `default` or `plan` stops the task.
- **Boundary.** The UI names a runtime ID, an objective, an optional (validated) model, and a
  session ID. Executables come only from detection (PATH + known install locations; Windows
  `.exe` only), are allowlisted by Core, and re-checked at spawn.
- **Billing.** Sign-in is checked before every turn with the CLI's own status command; signed
  out, API-key, and third-party-cloud sign-ins are refused. Claude Code's reported credential
  source is checked again in each stream. API-key variables are never passed to children.
  Grok also runs with `GROK_DISABLE_API_KEY_AUTH=1`, so it refuses API keys itself. Kimi's
  check (`kimi provider list`) must show the subscription provider (`managed:kimi-code`,
  `source=oauth`), and Plenipo runs only its `kimi-code/…` models (model names may carry one
  provider prefix, `provider/model`).
- **Least privilege.** Claude Code: no built-in tools, no MCP servers but Plenipo's. Codex:
  read-only sandbox. Grok: an agent profile with none of its own tools, no subagents, memory,
  web fetch, or Claude Code/Cursor settings. Kimi: its files through Guard, its shell refused,
  its `plan` mode for a worker without permissions. Each session has its own empty workspace. Organization workers with
  permissions get Plenipo's tools for each step (§10).
- **Sessions.** `runtime_sessions` (migration 0002) maps Plenipo's session to the provider
  session ID. Each turn is a task (`metadata.sessionId`) with an execution, `agent.*` activity
  events, and an `agent.result` event written together with the task's final state.
- **Outcomes.** `completed`, `failed`, `cancelled`, `timedOut`, `usageLimited`,
  `authRequired`, `billingNotAllowed`, `providerUnavailable`, `malformedOutput`, `crashed`,
  `interrupted`. A usage limit never switches provider. Turns running when Plenipo stopped are
  recorded as interrupted on the next start.
- **Tests** run against `plenipo-fake-agent`, a test double that speaks each tool's format
  (Grok and Kimi: ACP).
- **Adding an AI tool** ([ADR-014](../adr/ADR-014-adding-ai-tools.md), adding AI tools ahead of
  Phase 15): the [adapter guide](../development/adding-an-ai-tool.md) is the contract, and
  `crates/runtime/tests/contract.rs` checks it for every adapter in `builtin_adapters()`. The
  fake CLI has one persona per AI tool; test helpers install every persona it lists.

## 7. Liaison (Phase 4)

Decision record: [ADR-008](../adr/ADR-008-liaison.md).

- **Workers never control each other.** In a session the owner started with handoffs allowed,
  a worker asks for help by ending its answer with fenced `plenipo-handoff` JSON blocks
  (`to`, `objective`, `acceptanceCriteria`, `context`, `artifacts`, `capabilities`,
  `priority`; protocol `plenipo-liaison/1`). Liaison parses them as untrusted input: unknown
  or identity fields (sender, IDs, correlation) are refused; the sender is whoever Plenipo's
  own records say is running that turn.
- **Destinations are runtimes** (`claude-code`, `codex`, `grok`, `kimi`, or `runtime:<id>`) for sessions the
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
  requester cannot know. Capability requests are recorded for the owner and grant nothing:
  a worker's permissions come only from the owner's settings (§10).
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
- **Limits.** Depth 3, 3 requests per answer, 8 reply rounds per task, 16 handoffs per
  workflow (5 and 12 before Phase 8). Beyond a limit the request is refused and the worker told to do it itself.
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
- **Policy.** Projects record allowed runtimes (explicit; none allows none), a local folder (the
  workers' workspace from Phase 7), and a permission limit (a Guard permission set). A position's
  runtime is either fixed by the owner or automatic (chosen
  by the Router, §9). Since Phase 8 a lead's team also includes its staffed full-time direct
  reports, which take the task in their own conversation (§11).
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

## 10. Guard, capabilities, and approvals (Phase 7)

Decision record: [ADR-013 (how Plenipo lets workers use your computer safely)](../adr/ADR-013-guard-capability-broker.md).

- **Words on screen.** Settings → **Permissions**: "permission set" (capability profile),
  "Allowed / Ask me / Blocked" (levels), "permission limit" (project or department policy),
  "Approved / Always ask me first / Never run" (command rules), "Secrets" (the Vault). The
  **Approvals** page: "waiting for your approval", "Approve / Deny", "Revoke".
- **Registry** (`crates/guard/src/registry.rs`): filesystem.read/write, shell.exec,
  powershell.exec, git.read/write, (Phase 8) github.read/write, (Phase 10) browser.\* and
  computer.\*, and (Phase 11) ssh.connect have tools now; mcp.invoke, network.local, and
  process.manage are registered for later phases.
- **Configuration** is the Ledger's `guard` setting: permission sets (7 built in), each role's
  set, department limits, command rules, blocked files, the sensitive-action rules, options, and
  secret references. Every change is a `guard.*` or `vault.*` event. Each built-in role template
  gets its starting set once.
- **Engine** (`crates/guard/src/engine.rs`, pure): the strictest of role set, project limit,
  and department limit (an unknown limit fails closed); then the target (inside the folder,
  blocked files, `.git` internals), blocked commands, sensitive actions (ask or block, never
  allow), always-ask commands, an Ask level, and — for programs — the approved list. One plain
  explanation and a note per layer.
- **Broker** (`crates/capabilities`): the agent runtime's `ToolProvider` hook opens a grant for
  each step of an organization worker's task (`guard.grant_opened`) and closes it when the step's
  program ends (`guard.grant_closed`), stopping its programs and expiring its approvals. The AI
  tool gets Plenipo's MCP server over stdio (Claude Code `--mcp-config` +
  `--allowedTools mcp__plenipo`; Codex `-c mcp_servers.plenipo.*`), which is Plenipo's own
  executable in relay mode (`--plenipo-tools=<ticket file>`, handled in `main()`). The relay
  presents the step's ticket to the tool server on `127.0.0.1`.
- **Tools**: list, read, search, write, edit, move, delete files; run a program (a name and
  arguments, never a shell line); run a PowerShell script; git status, diff, log, add, commit,
  branch, push. Programs run through the supervisor (`capability.program`: own process tree,
  cleared environment, time limit).
- **Approvals.** "Ask" pauses the call: the Ledger records the approval and moves the task to
  `awaitingApproval` in one transaction, and back to `running` when it is answered. It expires
  after the approval window (default 10 minutes), and approvals left pending at shutdown are
  expired at startup. The UI shows a banner on every page, a sidebar count, and a card with
  exactly what will run.
- **Revocation** stops a grant's programs, refuses its waiting approvals, and blocks its later
  calls; a settings change applies to the next call.
- **Logging and redaction.** Every call is `capability.used`, `guard.denied`, or `approval.*`.
  Known secret values and common key and token formats are hidden in results, recorded text, and
  all AI tool activity (`[hidden by Plenipo: …]`).
- **Vault.** Secret values live in Windows Credential Manager (macOS Keychain, Linux keyring);
  Plenipo stores only references and injects a value as an environment variable into the
  programs the owner named.

## 11. Development department (Phase 8)

Decision record: [ADR-016 (the Development department: delegation, working copies, GitHub, and
the result)](../adr/ADR-016-development-department.md).

- **Words on screen.** The plan's Development Superintendent is the **Development VP**, project
  coordinators are **Supervisors**, git worktrees are **working copies**, and the final result
  is the objective's **result**. The **Projects** page is the plan's project dashboard.
- **Delegation to full-time members** (`crates/workforce/src/directory.rs`,
  `conversation.rs`). A lead's team includes its staffed full-time direct reports. Liaison's
  `Directory` hook places a request to one as a new turn in that member's own session
  (`ChildConversation`); a busy member's turn waits (`SessionBusy`) until it is free. Work only
  goes down reporting lines, which cannot loop, so members never wait on each other. Stopping a
  delegated task stops only its turn (`cancel_task`).
- **Working copies** (`crates/capabilities/src/worktrees.rs`, `broker.rs`). When a worker of an
  objective first needs a project folder that is committed content of a git repository,
  the broker makes a `git worktree` on `plenipo/<objective>-<id>` in `<app data>/working-copies`
  and confines every worker of that objective to it. One writer at a time: a second writer of
  the same objective gets `<branch>-2`, made from the first. In a working copy the git tools may
  not switch or create branches, and push only the objective's branch. After each step that used
  it, its commits and changed files are recorded (`workspace.updated`). Plenipo's own git runs
  without hooks, prompts, or inherited environment, with a time limit.
- **GitHub tools** (`crates/capabilities/src/github.rs`, `tools.rs`): pull request list, view,
  and checks, issue view (github.read), and a draft pull request for the objective's branch
  (github.write; it pushes first, and always asks the owner). They run GitHub's `gh` on the
  project's own repository only, with prompts, pager, and color off. `gh` is signed in by the
  owner, or given a `GH_TOKEN` secret.
- **Playbook and verdicts** (`crates/workforce/src/prompt.rs`). A lead with full-time reports is
  told to hand each objective on and report back briefly; a Supervisor with a team gets the
  development playbook. Reviewers, QA, and security auditors end with a `plenipo-review` block
  (verdict and findings).
- **The result** (`crates/workforce/src/outcome.rs`, `ObjectiveReport`): built only from the
  Ledger — the task tree, workers and models, files, programs and tests, branches, pull
  requests, verdicts and open findings, approvals, blocked requests, and problems — beside the
  lead's own answer.
- **Development template** (`crates/workforce/src/templates.rs`): the department, VP,
  Supervisor, and team as data over the Phase 5 engine (`set_up_development`).
- **Projects page** (`apps/desktop/src/views/ProjectsView.tsx`,
  `components/ObjectiveResult.tsx`): projects, the objective box, objectives, the live result,
  and working copies. It reloads (debounced) on `task.*`, `liaison.*`, `workspace.*`,
  `approval.*`, `session.*`, `capability.used`, and `agent.result` events.

## 12. Browser and computer use (Phase 10)

Decision records: [ADR-020 (Plenipo's browser and computer use, through Guard)](../adr/ADR-020-browser-and-computer-use.md)
and [ADR-019 (every role knows its job)](../adr/ADR-019-role-working-instructions.md).

- **Words on screen.** "Plenipo's browser", "Visit websites" (`browser.navigate`), "Use
  websites" (`browser.automate`), "See the screen" (`computer.observe`), "Use the mouse and
  keyboard" (`computer.control`), "Take over", "Stop all", "Allow again", and **Settings →
  Permissions → Websites** (the plan's domain policy).
- **Plenipo's browser** (`crates/capabilities/src/browser/`). The installed Edge or Chrome, as
  the owner chooses in Settings (ADR-028: Automatic, Edge, or Chrome, kept in Guard's settings;
  `PLENIPO_BROWSER` wins), started as a supervised program with its own profile in
  `<app data>/browser-profile` (Edge's) or `browser-profile-chrome` (Chrome's), with no password
  saving, sync, or extensions, and a random DevTools port on `127.0.0.1`. A new choice is used
  from the browser's next start. `cdp.rs` speaks the Chrome DevTools Protocol over a WebSocket (flattened
  sessions); `tab.rs` gives each grant its own tab, with page helpers (`page.js`) in an isolated
  world and a binding only that world sees; `classify.rs` decides what a click or submit is
  (sending, buying, signing in).
- **Tools** (`tools.rs`, `broker/operate.rs`): `browser_open/read/screenshot/scroll/back`
  (visit) and `browser_click/type/press/select` (use), by references from `browser_read`;
  `screen_view` (see) and `screen_take_control/click/type/keys/scroll/release_control` (use the
  mouse and keyboard). Every call goes through Guard: the permission set, the website lists
  (`crates/guard/src/websites.rs`, checked for every page the tab loads), and the sensitive kinds
  (sending, buying, **signing in**, **taking control of the mouse and keyboard** — ask or block,
  never allow).
- **Network gate.** While a worker's action runs, the tab intercepts requests (`Fetch`): a
  document or script request that is not a plain read is held until the owner approves, a
  form the page sends by itself is failed, and a page on a blocked website never loads.
- **Never:** typing into password, one-time-code, or card fields; typing a secret; trying a
  CAPTCHA more than 3 times (ADR-029); the Windows key. Page text reaches the worker marked as
  the website's.
- **Screenshots** (`screens.rs`): after every significant action and before every approval,
  kept in `<app data>/screenshots/<task>/` as a Ledger `screenshot` artifact with its SHA-256,
  linked from `capability.used` and approvals, given to the worker as an MCP image with a
  description in words, and shown in the Activity trail and on approval cards.
- **The desktop** (`desktop.rs`): `xcap` (Windows) or X11 (Linux) for screenshots, `enigo` for
  input; `SyntheticDesktop` stands in for tests. Coordinates are the last screenshot's.
- **Control center** (`control.rs`): every session (browser, desktop, or — Phase 11 — server; active, taken over, or
  stopped) and the sticky emergency stop, told to the app (`plenipo://control`), the tray, and
  the indicator window in order, with a revision. Stop halts every session, releases held input,
  revokes those grants, and refuses new control until `allow_control`. Take over (a button, the
  owner's own click or key in the page, or moving the mouse on the desktop) stops that worker and
  refuses its waiting approvals; the tab stays open for the owner.
- **Signs.** A banner on every page and the footer (`ControlBanner.tsx`), the tray menu line and
  **Stop all browser, desktop, and server work** (`tray.rs`), the indicator window above all others
  while the desktop is controlled (`indicator.rs`, `IndicatorView.tsx`), and in the browser a
  colored frame and label inside the page (in a closed shadow root) with **Take over**.
- **Events:** `browser.started`, `browser.tab_lost`, `browser.opened_by_owner`,
  `control.started`, `control.taken_over`, `control.stopped`, `control.allowed`,
  `control.ended`, `guard.websites_changed`, `guard.browser_chosen`.
- **Role instructions** (`crates/workforce/src/templates.rs`, `prompt.rs`): each role's job,
  returns, limits, and when to ask its lead, plus what its permissions allow and do not; custom
  roles take the same in the owner's words (`update_role`).

## 12a. Switches and learning (v1.4)

Decision records: [ADR-023 (on/off switches in Settings)](../adr/ADR-023-settings-switches.md)
and [ADR-024 (workers learn from their work)](../adr/ADR-024-workers-learn-from-work.md).

- **Switches** (`crates/guard/src/dto.rs` `Switches`, in the Guard settings, event
  `guard.switches_changed`): Plenipo's browser (on), the screen, mouse, and keyboard (off),
  sending, buying, and signing in without asking (off), handing CAPTCHAs to the owner (on),
  screenshots in the Activity trail (on). Worker learning is shown with them.
- **A feature switched off** (`engine.rs` `switched_off`): Guard's level for its capabilities is
  Blocked (layer: rule), whatever the permissions. `set_switches` stops its sessions
  (`ControlCenter::stop_kind`, no sticky stop) and refuses their waiting approvals
  (`control.switched_off`); a worker given no tools because of a switch is told why
  (`guard.grant_skipped`).
- **Without asking** (`engine.rs`, `broker/operate.rs` `decide_held`): a sending, buying, or
  signing-in check is allowed only for `browser.automate`, on a website on the Allowed list,
  when that sensitive kind's rule is Ask and the role's level is not Ask. Data the page sends
  after the action is released the same way when every website involved is allowed.
- **CAPTCHAs** (`browser_person_check`, `ControlWork::PersonCheck`; ADR-029, ADR-032): with a
  CAPTCHA on the page, the page read names the check's maker and lists its widget frame as a
  control (its checkbox, `page.js` `captchaState`), and a worker may try it up to 3 counted
  times (`Tab::count_captcha_try`, counted once the click or key press happened). After each try
  Plenipo watches the check for up to 4 seconds (`Tab::captcha_verdict`) and says whether it
  passed (the maker's answer field in the page holds a value), opened a puzzle, is gone, or is
  still there; a passed or gone check starts the count over, and a click on a passed check is
  refused. The sign is drawn only in the top page, never inside a frame. Then, or at once when
  the worker prefers, `browser_person_check` asks the owner to solve it: the tab goes to mode
  `handed` (purple sign; the owner's clicks are not a take over), comes to the front, and
  interception stops until the owner answers (`Tab::take_back`).
- **Screenshots off** (`keep()`): steps keep no picture; approval pictures are always kept.
- **Lessons** (`crates/ledger/src/lessons.rs`, migration 7 `lessons`; `crates/workforce/src/learning.rs`):
  a Ledger listener on `agent.result` reads `plenipo-lesson` blocks (at most 3 a task, 300
  characters each) and records them for the worker's role, waiting or kept (`lesson.added`).
  Lessons from a task that used the browser, the screen, or (Phase 11) a server (itself or any
  task handed on from it: `task_used_web_screen_or_servers`) always wait. The owner keeps (optionally edited), discards, or removes them (`lesson.kept`,
  `lesson.discarded`, `lesson.removed`). Each worker's instructions (`directory.rs`) carry its
  role's newest 20 kept lessons and how to write one, unless learning is off (Ledger setting
  `learning`, events `learning.switched` and `learning.role_changed`).
- **Screens:** Settings → Switches (`SwitchSettings.tsx`); Approvals → New lessons, and a role's
  "What it has learned" and **Learn on its own** in its details (`learning/Lessons.tsx`). The
  sidebar's Approvals count includes waiting lessons.

## 13. Servers over SSH (Phase 11)

Decision records: [ADR-025 (servers over SSH, through Guard)](../adr/ADR-025-servers-over-ssh.md)
and [ADR-026 (SSH built into Plenipo, not Windows' ssh.exe)](../adr/ADR-026-ssh-built-in.md).

- **Words on screen.** "Connect to servers" (`ssh.connect`), **Settings → Servers** (the plan's
  host registry), "server ID" and "pin" (host key fingerprint and pinning), "sign in as", "the
  kinds of commands" (command classes), Test / Staging / **PRODUCTION**, and
  "Disconnect".
- **Servers** (`crates/guard/src/servers.rs`, in Guard's settings, no migration): name, address,
  port, user, environment, how Plenipo signs in (a key or password in the Vault, or the SSH
  agent), the pinned server ID, the roles that may connect, the kinds of commands, folders, when
  to ask, and forwarded ports. Production always asks for every command and starts without
  "Delete, wipe, or shut down".
- **Kinds of commands.** A command is a program and its arguments, never a shell line.
  `classify` looks through `sudo` and wrappers and sorts it into Look around; Start, stop, and
  restart services; Install, deploy, and change files; Delete, wipe, or shut down; Run as
  administrator; or Other. The never list (reaching other computers, scanning, cracking) and
  blocked files apply first. Guard's engine gets a `ServerCheck` in the request: role, pinned
  server ID, never list, blocked files, folders, kinds, then when to ask. Any change on a production
  server is also the `Production` sensitive kind ("Deploying or changing live systems"); when a
  command is two kinds, the stricter of the owner's rules wins.
- **SSH** (`crates/capabilities/src/ssh.rs`, russh with ring). The server's key is compared with
  the pinned fingerprint before signing in; a change ends the connection and nothing is sent. No
  agent forwarding, no shell, no remote forwarding. A command runs as
  `cd '<folder>' && exec '<program>' '<arg>'…`, with each word quoted. Stop sends TERM, then
  KILL, then closes the channel. Keepalives notice a lost connection. Direct forwarding opens a
  local port on `127.0.0.1` for the grant only.
- **Tools** (`broker/servers.rs`): `ssh_servers`, `ssh_run`, `ssh_forward`, `ssh_disconnect`.
  One connection per server per grant, closed when the step ends. The server ID is checked before
  an approval card is shown. Output reaches the Activity trail every quarter second (lines of at
  most 4,000 characters) with secrets hidden, and the worker gets it marked as the server's
  words, never instructions.
- **The Vault** keeps long values (an RSA key) in pieces of 1,000 characters, since Windows
  Credential Manager holds 1,280 per entry: `server-<id>-key`, `-passphrase`, `-password`.
- **Control.** Each grant's servers are one control session (`server:<grant>`, with a
  `production` flag). The sign, footer, and tray name the worker and server; **Disconnect** stops
  its commands and closes its connections; **Stop all** covers servers.
- **The switch** (`Switches.servers`, ADR-023): **Remote computers (SSH)**, off to start. Off,
  `switched_off` blocks `ssh.connect` for every role (layer: rule), and `set_switches` stops
  every server session (`switch_off_control(ControlKind::Server)`). `ServersSnapshot.switchedOn`
  lets Settings → Servers say so.
- **Lessons** (ADR-024): a task that used a server counts as outside content, so its lessons
  always wait for the owner.
- **Events:** `ssh.connected`, `ssh.connect_failed`, `ssh.host_key_changed`,
  `ssh.identity_checked`, `ssh.tested`, `ssh.command_started`, `ssh.output`,
  `ssh.command_finished`, `ssh.forward_opened`, `ssh.forward_closed`, `ssh.disconnected`,
  `guard.server_added`, `guard.server_changed`, `guard.server_removed`,
  `vault.server_sign_in_stored`. None carries a key, passphrase, or password.

## 13a. Design system (Phase 12A)

Decision record: [ADR-030](../adr/ADR-030-design-system.md). Details:
[`docs/design/design-system.md`](../design/design-system.md).

- **`packages/ui` (`@plenipo/ui`)** holds the design tokens, the component library, and their
  styles; the desktop app imports it (`@plenipo/ui/styles.css` first, then its own page layouts).
- **Tokens** are written once in `packages/ui/src/tokens.ts` and generated as CSS custom
  properties (`--ui-…`, dark on `:root`, each theme on `[data-theme]`) and JSON; a test checks
  the generated files and WCAG AA contrast in both themes.
- **The frame:** `AppShell` with the left strip (`IconRail`, names under the icons), the top bar
  (`ScopeSelector`, the page title, `ThemeToggle`, `NotificationBell`), and `BannerSlot`. The
  theme is `<html data-theme>`, remembered in the webview's storage.
- **No raw colors** in feature code: ESLint (TypeScript) and `scripts/check-colors.mjs` (CSS), run
  by `pnpm lint`.
- **Windowing:** tables and card grids draw only the rows on screen (`useVirtualWindow`).
- **The Gallery** (Diagnostics → Open the gallery) renders every component in every state, in
  either theme or both; the end-to-end test compares its computed styles with checked-in
  snapshots.

## 13b. Home, the pages, the terminal, notices, and Settings (Phase 12)

Decision records: [ADR-031](../adr/ADR-031-terminal-panel.md) (the terminal panel) and
[ADR-033](../adr/ADR-033-pages-notices-settings.md) (Home, the pages, notices, and Settings).

- **Where you are** is a place (`components/views.ts`: a section, or the page of one department,
  project, worker, or task with its ID), kept on this computer (`plenipo.place`), with a Back
  trail for this window. The pages (`apps/desktop/src/pages/`) are built from `@plenipo/ui`'s page
  parts (`PageHeader`, `Panel`, `RowList`, `StatGrid`, `Hero`) and read Core live (`useLive`:
  reloaded after the Ledger events that may change them).
- **Page queries** (Ledger `pages.rs`, on the existing tables): a scope's events a page at a time
  (`scope_events`), a task tree's events (`tree_events`), what is stuck (`problems`: one problem
  per piece of work still in trouble, read from the last week's events only), and a project's or
  task's pull requests, screenshots, and decisions (`work_record`). Workforce `home()` adds the
  objectives going and those finished (the last to finish first). History pages hold at most 200
  events; a scope's history reads the newest 20,000 events in order first, then looks further
  back through the task index only when the page is not full.
- **The terminal** (ADR-031): `crates/capabilities/src/terminal.rs` runs the owner's shell in a
  pseudo terminal (`portable-pty`, ConPTY on Windows; a kill-on-close Job Object per shell) or
  relays a server's shell channel (`ssh.rs` `open_shell`, the owner's only); `broker/terminals.rs`
  opens, tracks, and records them (`terminal.opened`, `terminal.closed`), apart from every
  worker's grant, so Stop all never reaches them. Output streams to the page on a Tauri channel;
  nothing typed or shown is recorded. Watch tabs are built in the page from the Ledger's `ssh.*`
  events; their **Stop** ends only the worker's command running now (`stop_server_command`).
  The terminals end with the page that shows them (a reload of the main window closes them), when
  Plenipo quits, and, for servers, when Remote computers (SSH) is switched off. While a worker
  controls the screen, mouse, and keyboard, the terminal takes no typing (it could be the
  worker's) until the owner takes over. Closing the window while a terminal is open hides Plenipo
  to the tray, as running work does.
- **Notices:** a Ledger listener hands each committed event that may matter
  (`notices::may_notify`) to a background thread (`src-tauri/src/notices.rs`), which asks the
  Ledger what it means for the owner (`Ledger::notice_for`), keeps the kinds the owner wants
  (`NoticeSettings`, in the `preferences` setting), gathers those that arrive together
  (`NoticeGate`: one notice for a burst, none repeated within a minute), and shows it unless
  Plenipo's window is in front and the owner asked for that. "Send a test notice" shows one now.
- **Settings** is one list of sections (`views/SettingsView.tsx`, `settings/`); the last one comes
  back, and another page can open a section. Local paths are read-only (`get_local_paths`).

## 14. Launch smoke test

With `PLENIPO_SMOKE_TEST=1`, the app launches normally, the UI calls `frontend_ready` once it
has rendered **and** successfully called Core, and the process exits 0. If that does not
happen within `PLENIPO_SMOKE_TIMEOUT_SECS` (default 60) a watchdog exits 1. The outcome is
tracked in shared state rather than trusting the runtime's exit-code propagation, which is not
reliable on every platform. CI runs this against the release build on Windows.

## 15. Target component map

From the rollout plan. **Desktop**, **Core**, **Runtime** (supervisor and agent runtime
adapters), **Ledger**, **Liaison**, **Workforce**, **Router**, **Capabilities**, **Guard**,
**Vault**, the GitHub integration, Plenipo's browser and computer use, and SSH exist today.

| Component    | Responsibility                                                                          | Introduced  |
| ------------ | --------------------------------------------------------------------------------------- | ----------- |
| Desktop      | UI, on the design system (`packages/ui`) ✅; Home and the pages, terminal, notices ✅   | Phase 0, 12 |
| Core         | Orchestration and domain logic, shared DTOs                                             | Phase 0     |
| Runtime      | Supervisor ✅, Codex / Claude Code adapters ✅                                          | Phase 1, 3  |
| Ledger       | SQLite system of record ✅                                                              | Phase 2     |
| Liaison      | Task/message/event bus ✅                                                               | Phase 4     |
| Workforce    | Departments, roles, coordinators, workers ✅                                            | Phase 5     |
| Router       | Role → provider/model selection ✅                                                      | Phase 6     |
| Capabilities | Filesystem ✅, shell ✅, Git ✅, working copies ✅, browser ✅, computer use ✅, SSH ✅ | Phase 7+    |
| Guard        | Permissions, approvals, policy enforcement ✅                                           | Phase 7     |
| Vault        | Credential references (OS-protected storage) ✅                                         | Phase 7     |
| Integrations | GitHub ✅, HubSpot (Sales, postponed: ADR-018), CrewOS                                  | Phase 8+    |

## 16. Invariants every phase must keep

- **Local-first** ([ADR-002](../adr/ADR-002-local-first-architecture.md)): the desktop app owns
  execution; remote surfaces never become the privileged runtime.
- **Provider-independent roles** ([ADR-003](../adr/ADR-003-provider-independent-roles.md)): no
  vendor names in Core types; provider specifics live in adapters.
- **No credentials to launch.** Launching Plenipo must never require provider keys.
- **No secrets in source control, logs, or prompts.**
- **Capabilities enforced in code**, not described in prompts.
- **Every long-running operation has an ID and a cancel path** (from Phase 1 onward).
