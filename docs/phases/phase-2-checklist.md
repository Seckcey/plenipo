# Phase 2 — Implementation Checklist

**Status:** complete — see [phase-2-acceptance-report.md](phase-2-acceptance-report.md).

Source: `ROLLOUT_PLAN.md`, Phase 2 — Plenipo Ledger: Durable Task and Event Model.
Phase 1 accepted (v0.2.0). Owner approved starting Phase 2.

**Goal:** create the durable system of record before multiple agents begin generating work.

## Design decisions (details in ADR-006)

- **SQLite, bundled** (`rusqlite` with the `bundled` feature): no system SQLite dependency on
  Windows. WAL journal, `synchronous=FULL` (an audit ledger favors durability over speed),
  foreign keys on, busy timeout for concurrent writers.
- **Location:** `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\plenipo.db`. Local, not Roaming:
  SQLite files must not be synced by roaming profiles.
- **Migrations:** numbered, embedded SQL with `up` and `down` scripts, recorded with a checksum.
  Production strategy is forward-only with an automatic **pre-migration backup** as the
  rollback path; `down` scripts exist for development and are tested round-trip. A database
  newer than the app is refused, never silently opened. Edited migrations are detected.
- **Every change leaves a trail:** each task mutation writes its event in the same
  transaction. Events are **append-only** (SQLite triggers reject UPDATE/DELETE) and globally
  ordered by an autoincrement sequence.
- **Task state machine** enforced in the only write path; state values also constrained by
  `CHECK`.
- **Corruption is never ignored:** integrity check on open; a corrupt file is quarantined,
  a fresh database is started, and a blocking notice is shown with the quarantine path.
- **Backups and export:** `VACUUM INTO` snapshots (consistent while in use) and a JSON export,
  both written to a backend-chosen directory (the UI never supplies a path).
- **Phase 1 executions move into the Ledger**; the interim JSON history is imported once.

## Deliverables

- [x] `crates/ledger`: connection setup, migration runner, repository layer
- [x] Schemas: department, role, agent instance, project, task, event, execution, approval,
      artifact (extensible `metadata` JSON columns)
- [x] Provider/model execution metadata (execution table carries runtime, provider, model,
      session, usage)
- [x] Runtime supervisor persists executions through the Ledger (store trait), one-time import
      of `executions.json`
- [x] Commands: ledger status, tasks, task timeline, recent events, synthetic tasks (diagnostic),
      integrity check, backup, export — each granted in capabilities
- [x] Activity timeline UI: task list, ordered per-task trail, all-events feed, live updates
- [x] Diagnostics: ledger health, integrity check, backup, export, synthetic task tools
- [x] ADR-006 (Ledger design); architecture/config docs updated

## Phase 2 tests (from plan)

- [x] Migration up/down strategy
- [x] CRUD tests
- [x] Parent/child task relations
- [x] Event ordering
- [x] Restart durability (including a hard-killed writer process)
- [x] Invalid state transition tests
- [x] Concurrent event writes (threads and separate connections)
- [x] Backup/export smoke test

## Acceptance criteria (from plan)

- [x] Kill and relaunch Plenipo while a synthetic task exists; task history remains intact
- [x] A task has a complete ordered activity trail
- [x] Invalid task transitions are rejected
- [x] Database corruption is not silently ignored

## Out of scope

Semantic memory, cloud database, multi-user sync, advanced analytics, secret redaction
(Phase 7), real agent tasks (Phase 3+).
