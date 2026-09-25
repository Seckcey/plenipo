# Architecture Overview

This document is the architectural contract for Plenipo. It describes what exists today
(Phase 0) and the boundaries later phases must respect. Decisions behind it are in
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
│                                            └──────────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────────────────────────────┘
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

| Command          | Returns   | Purpose                                                    |
| ---------------- | --------- | ---------------------------------------------------------- |
| `get_app_info`   | `AppInfo` | Name, version, build profile, OS, arch for the shell       |
| `frontend_ready` | `()`      | UI signals successful render; only acts in smoke-test mode |

All commands return `Result<T, CommandError>`; the TS client converts rejections into
`PlenipoCommandError { kind, message }`.

## 4. Launch smoke test

With `PLENIPO_SMOKE_TEST=1`, the app launches normally, the UI calls `frontend_ready` once it
has rendered **and** successfully called Core, and the process exits 0. If that does not
happen within `PLENIPO_SMOKE_TIMEOUT_SECS` (default 60) a watchdog exits 1. The outcome is
tracked in shared state rather than trusting the runtime's exit-code propagation, which is not
reliable on every platform. CI runs this against the release build on Windows.

## 5. Target component map

From the rollout plan. Only **Desktop** and **Core** exist today.

| Component    | Responsibility                                | Introduced |
| ------------ | --------------------------------------------- | ---------- |
| Desktop      | UI                                            | Phase 0    |
| Core         | Orchestration and domain logic, shared DTOs   | Phase 0    |
| Runtime      | Supervisor, then Codex / Claude Code adapters | Phase 1, 3 |
| Ledger       | SQLite system of record                       | Phase 2    |
| Liaison      | Task/message/event bus                        | Phase 4    |
| Workforce    | Departments, roles, coordinators, workers     | Phase 5    |
| Router       | Role → provider/model selection               | Phase 6    |
| Capabilities | Filesystem, shell, Git, SSH, browser, MCP     | Phase 7    |
| Guard        | Permissions, approvals, policy enforcement    | Phase 7    |
| Vault        | Credential references (OS-protected storage)  | Phase 7    |
| Integrations | Paperclip, GitHub, CrewOS                     | Phase 8+   |

## 6. Invariants every phase must keep

- **Local-first** ([ADR-002](../adr/ADR-002-local-first-architecture.md)): the desktop app owns
  execution; remote surfaces never become the privileged runtime.
- **Provider-independent roles** ([ADR-003](../adr/ADR-003-provider-independent-roles.md)): no
  vendor names in Core types; provider specifics live in adapters.
- **No credentials to launch.** Launching Plenipo must never require provider keys.
- **No secrets in source control, logs, or prompts.**
- **Capabilities enforced in code**, not described in prompts.
- **Every long-running operation has an ID and a cancel path** (from Phase 1 onward).
