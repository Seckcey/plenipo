# ADR-006: Plenipo Ledger (SQLite system of record)

- **Status:** Proposed
- **Date:** 2026-09-26
- **Phase:** 2

## Context

Before several AI workers generate work (Phases 3–8), Plenipo needs one durable, local,
auditable record of tasks, delegations, executions, approvals, and artifacts. ROLLOUT_PLAN
Phase 2 requires restart durability, a complete ordered activity trail per task, rejection of
invalid transitions, and that database corruption is never silently ignored. ADR-002 requires
local-first storage.

## Decision

1. **Engine.** SQLite, compiled in (`rusqlite` with `bundled`), so no system SQLite is needed
   on Windows. One database file per user.
2. **Location.** `%LOCALAPPDATA%\com.eightwest.plenipo\ledger\plenipo.db` (Linux:
   `$XDG_DATA_HOME/com.eightwest.plenipo/ledger/`). **Local, not Roaming**: roaming profiles
   can copy a SQLite file mid-write.
3. **Durability settings.** WAL journal, `synchronous=FULL`, `foreign_keys=ON`, 5 s busy
   timeout. Every write is one `BEGIN IMMEDIATE` transaction.
4. **Trail.** `events` is append-only (triggers reject `UPDATE`/`DELETE`) with a global
   `AUTOINCREMENT` sequence. Every task, approval, artifact, execution, and org mutation writes
   its event **in the same transaction** as the change. Rejected task transitions are recorded
   as `task.transition_rejected`.
5. **Integrity rules in two layers.** Rust enforces the task/approval/agent state machines on
   the only write paths; SQL `CHECK` and foreign-key constraints back them up. Tasks and events
   are never deleted; org entities can be deleted only when nothing references them.
6. **Migrations.** Numbered SQL files embedded in the binary, each with `up` and `down`,
   recorded with a checksum. Production is **forward-only**: before upgrading an existing
   database, Plenipo writes a verified `pre-migration-v<N>-<ts>.db` backup, which is the
   rollback path. `down` scripts are for development and are tested round-trip. A database
   newer than the app, an edited migration, or a gap in history is refused.
7. **Corruption.** `PRAGMA quick_check` on every open. A failing file (and its WAL/SHM) is
   renamed to `plenipo.db.corrupt-<ts>`, a fresh ledger starts, and a notice explains where the
   damaged file and backups are. If the ledger cannot be opened at all (or is too new), Plenipo
   runs on a temporary in-memory ledger and says so prominently; nothing is silently lost.
8. **Backups and export.** `VACUUM INTO` produces a consistent snapshot while in use; each
   backup is reopened and integrity-checked. The newest 10 manual backups are kept
   (pre-migration backups are never pruned). A JSON export contains every table. The UI can
   request both but never chooses the path.
9. **Scope of data.** Metadata and events only. Process output and environment values are not
   stored. `metadata`/`payload` columns are JSON objects for forward-compatible extension.
10. **Runtime integration.** The runtime supervisor persists through an `ExecutionStore`
    interface; the desktop app backs it with the ledger. Phase 1's `executions.json` is
    imported once and renamed.

## Consequences

- Kill -9 or power loss after a commit cannot lose that commit; a crash mid-transaction loses
  only that transaction.
- `synchronous=FULL` costs some write throughput; acceptable for an audit ledger at Plenipo's
  scale, revisit only with measurements.
- The schema is extensible via JSON columns and forward migrations, but every schema change
  needs a migration and a test.
- Secret redaction in payloads is Phase 7's job; until then, callers must not put secrets in
  event payloads (none do today).

## Alternatives considered

- **Separate event store + state tables updated asynchronously** — risks trails that disagree
  with state; single-transaction writes are simpler and exact.
- **Roaming app data** — convenient for multi-PC users but unsafe for SQLite.
- **Automatic down-migrations in production** — riskier than restoring a verified snapshot.
