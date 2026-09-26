# Phase 5 — Implementation Checklist

**Status:** implemented on `claude/phase-5`; awaiting owner acceptance (see the
[acceptance report](phase-5-acceptance-report.md)).

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

- [x] Organization schema: Ledger migration 0004 (positions, oversight, settings; project,
      department, and agent columns), up/down and v3 → v4 tested
- [x] Department definitions (with their head position)
- [x] Role templates (seeded as data; custom roles)
- [x] Persistent managers (superintendent, department manager)
- [x] Persistent project coordinators (project configuration: repository, local directory,
      department, coordinator role, allowed runtimes, default capability profile)
- [x] Ephemeral workers (spawned per task, retired when it ends)
- [x] Org tree UI: UniFi-style topology canvas (pan, zoom, fit, minimap, collapse, drag to
      move and assign, hire palette)
- [x] Agent cards (inspector) with status indicators
- [x] Task ownership views: running, waiting, queued, blocked, recently completed
- [x] Searchable list view (directory) with filters
- [x] Liaison role destinations through the Workforce directory
- [x] ADR-009; architecture, README, setup, configuration updated

## Phase 5 tests (from plan)

- [x] Create department
- [x] Create role
- [x] Assign manager
- [x] Create project coordinator
- [x] Coordinator creates child worker
- [x] Worker finishes and retires
- [x] Persistent coordinator survives restart
- [x] Department/project reassignment
- [x] Orphan prevention

Also: routing refusals (unknown role, raw runtime from a member, runtime not allowed by the
project), oversight routing, cycle prevention, title uniqueness, worker retirement on every
task end (success, failure, cancel, interruption), IPC boundary tests for every new command,
end to end through the real app.

## Acceptance criteria (from plan)

- [x] The user can view Development as a department, select a project, give a coordinator an
      objective, and observe one or more workers appear under that coordinator and disappear
      from active workforce after completion while history remains. (Fake CLIs, end to end;
      owner check with the real CLIs below.)

### Owner check on Windows (~20 minutes)

1. Both CLIs installed and signed in (setup guide §3): **Runtimes** → **Re-check** shows both
   **Ready**.
2. **Organization** opens with You → Organization and a "Build your organization" card.
   **Rename** it (for example _8 West Ventures_).
3. **Create a department** → Name _Development_ → **Create department**. A _Development Manager_
   appears to the right of the organization, with a "Development" chip on its link.
4. **+ Project** → Department _Development_, Name _Website_, keep both runtimes ticked →
   **Create project**. A _Website Coordinator_ appears under the manager with a "Website" chip.
5. Build the team: drag **Senior Developer** from the Hire palette onto _Website Coordinator_ →
   **Hire**; do the same for **Code Reviewer**. (Or select the coordinator, click a role in the
   palette, and choose **Hire**.)
6. Oversight: drag **Security Auditor** onto _Development Manager_ → **Hire**; then drag the new
   _Security Auditor_ node onto _Website Coordinator_ → **Security auditor for Website
   Coordinator's team**. A dotted "Security" link appears.
7. Select _Website Coordinator_ and give it an objective: _"Write a Python function that checks
   whether a string is a valid ISO 8601 date. Ask the Senior Developer to write it and the Code
   Reviewer to review it, then give me the final version."_ → **Give objective**.
   Expected: the coordinator shows **Working**, then **Waiting on team** while one or more
   worker nodes appear under _Senior Developer_ / _Code Reviewer_ (dashed links, runtime chip,
   **Working**); they disappear when done; the coordinator finishes **Idle**. (Criterion.)
8. History remains: select _Senior Developer_ → **Former agents** shows the retired worker;
   **Work → Recent** lists its task. **Activity** → the coordinator's task shows the delegation
   tree and "Worker spawned for …" / "Worker finished and left the organization" in the
   worker's trail.
9. Close Plenipo (tray → **Quit**) and start it again: the organization is unchanged, and the
   coordinator's details say "Continues its conversation."
10. Optional: **List** view → filter by status; search for a position; collapse a team with
    the "−" toggle on its trunk.

Things only real CLIs can confirm (report anything odd): that a coordinator follows its
instructions and hands work to its team by title (a request for anyone else is refused and
explained, so the task still finishes), and how long a team round trip takes on real
subscriptions.

## Out of scope

Paperclip import, model policy intelligence (Phase 6), cross-device org sync, capability grants
(Phase 7), delegation from managers to coordinators (Phase 8).
