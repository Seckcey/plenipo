# ADR-002: Local-first architecture

- **Status:** Accepted (specified by ROLLOUT_PLAN.md)
- **Date:** 2026-09-25
- **Phase:** 0

## Context

Plenipo's agents act on the owner's machine: repositories, shells, credentials, browsers.
The provider runtimes it drives (Codex, Claude Code) are authenticated locally. A future
remote surface (CrewOS) will offer visibility and limited control.

## Decision

- **The desktop application owns execution.** All privileged operations run in Plenipo's
  local Rust core on the owner's machine.
- **Local state is authoritative.** The Ledger (Phase 2) is a local SQLite database. No cloud
  database is required to operate.
- **Plenipo works offline** except where a task itself requires the network (e.g. a provider
  runtime).
- **Remote surfaces are clients, never the privileged runtime.** A remote request must travel
  `remote → authenticated local endpoint → Guard → organization/router → local runtime`.
  Local Guard policy is always authoritative. No unrestricted shell is ever exposed remotely.
- **No telemetry** leaves the machine unless the owner explicitly enables it.

## Consequences

- Owner data, credentials, and audit history stay on the owner's machine by default.
- Availability depends on the owner's machine being on; long-running work needs a background
  service (Phase 13).
- Remote features (Phase 14) must be designed as authenticated, policy-checked requests into
  the local core — never as moving execution to a server.
- Backup and recovery of local state are Plenipo's responsibility (Phases 2 and 13).

## Alternatives considered

- **Cloud-hosted orchestrator** — would need remote access to local machines and credentials,
  increasing attack surface and conflicting with provider authentication that is tied to the
  local machine.
