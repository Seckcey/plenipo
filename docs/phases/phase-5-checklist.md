# Phase 5 — Implementation Checklist

**Status:** in progress on `claude/phase-5`.

Source: `ROLLOUT_PLAN.md`, Phase 5 — Workforce and Organization Engine. Phase 4 is implemented
and merged (PR #5); the owner asked to begin Phase 5 on 2026-09-26. Owner direction for the UI:
the Organization page should look and work like the UniFi Network topology map
(`unifi.ui.com`) — hire, drag, assign, and move agents, and assign supervisors, QA evaluators,
and security auditors, directly on the canvas.

**Goal:** represent the company as departments, managers, coordinators, roles, projects, and
ephemeral workers.

## Design decisions (details in ADR-009)

- **Positions and agents are separate.** A _position_ is a place in the organization chart
  (title, role, supervisor, runtime). A _persistent_ position (superintendent, department
  manager, project coordinator) is held by one agent instance at a time and keeps its
  conversation in one long-lived runtime session; it can be vacant. An _on-demand_ position
  (developer, reviewer, QA engineer, …) starts a new, ephemeral agent instance for every task
  delegated to it, and that instance retires when its task ends.
- **Supervisor = reports-to.** Every position reports to a persistent position or to the owner.
  Rules are enforced in the Ledger transaction: no cycles, only persistent positions supervise,
  a department manager heads exactly one department, a coordinator coordinates exactly one
  project and reports to a department head, titles are unique within a team.
- **Oversight assignments** make an on-demand position the QA evaluator, security auditor, or
  reviewer for a team, in addition to its own reporting line.
- **Coordinators spawn workers through Liaison.** The `role:<name>` destinations reserved by
  ADR-008 now resolve through a Workforce directory: a member addresses the positions on its
  team (its on-demand reports and the team's overseers). The worker's agent instance is
  recorded in the same transaction as its child task. Members address roles only, never raw
  runtimes, so every worker appears in the organization.
- **Capability policy is not bypassed.** Projects record allowed runtimes and a default
  capability profile; a worker on a runtime the project does not allow is refused. No
  capability is granted before Guard (Phase 7); workers keep the Phase 3 posture.
- **Runtime per position is an explicit owner choice**, not model policy (Phase 6).
- **Delegating to persistent positions** (a manager to a coordinator) waits for the
  Development MVP (Phase 8): the owner gives persistent agents their objectives in Phase 5.
- **Canvas:** UniFi-style topology, built from Plenipo's own components and original glyphs
  (no UniFi assets). Camera and gesture model adapted from the owner's Coastline plan canvas.
  Every drag action has a keyboard path, and a searchable directory list complements the map.

## Deliverables

- [ ] Organization schema: Ledger migration 0004 (positions, oversight, settings; project,
      department, and agent columns), up/down and v3 → v4 tested
- [ ] Department definitions (with their head position)
- [ ] Role templates (seeded as data; custom roles)
- [ ] Persistent managers (superintendent, department manager)
- [ ] Persistent project coordinators (project configuration: repository, local directory,
      department, coordinator role, allowed runtimes, default capability profile)
- [ ] Ephemeral workers (spawned per task, retired when it ends)
- [ ] Org tree UI: UniFi-style topology canvas (pan, zoom, fit, minimap, collapse, drag to
      move and assign, hire palette)
- [ ] Agent cards (inspector) with status indicators
- [ ] Task ownership views: running, waiting, queued, blocked, recently completed
- [ ] Searchable list view (directory) with filters
- [ ] Liaison role destinations through the Workforce directory
- [ ] ADR-009; architecture, README, setup, configuration updated

## Phase 5 tests (from plan)

- [ ] Create department
- [ ] Create role
- [ ] Assign manager
- [ ] Create project coordinator
- [ ] Coordinator creates child worker
- [ ] Worker finishes and retires
- [ ] Persistent coordinator survives restart
- [ ] Department/project reassignment
- [ ] Orphan prevention

Also: routing refusals (unknown role, raw runtime from a member, runtime not allowed by the
project), oversight routing, cycle prevention, title uniqueness, worker retirement on every
task end (success, failure, cancel, interruption), IPC boundary tests for every new command,
end to end through the real app.

## Acceptance criteria (from plan)

- [ ] The user can view Development as a department, select a project, give a coordinator an
      objective, and observe one or more workers appear under that coordinator and disappear
      from active workforce after completion while history remains.

## Out of scope

Paperclip import, model policy intelligence (Phase 6), cross-device org sync, capability grants
(Phase 7), delegation from managers to coordinators (Phase 8).
