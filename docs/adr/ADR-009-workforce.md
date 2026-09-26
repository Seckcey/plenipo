# ADR-009: Plenipo Workforce — organization engine and topology canvas

- **Status:** Proposed
- **Date:** 2026-09-26
- **Phase:** 5

## Context

Phase 5 represents the company as departments, managers, coordinators, roles, projects, and
ephemeral workers. Managers persist; most workers are ephemeral (plan §1.6). A coordinator may
spawn workers through Plenipo Core but may not bypass capability policy. The acceptance
criterion: view Development, select a project, give its coordinator an objective, and watch
workers appear under the coordinator and leave the active workforce when they finish, while
their history remains.

Earlier decisions constrain it: the Ledger is the system of record and every change writes its
event in the same transaction (ADR-006); roles are provider-independent and model policy is
configuration (ADR-003, Phase 6); workers have no capabilities before Guard (Phase 7, ADR-007
§5); agents hand work to one another only through Liaison, which reserved `role:<name>`
destinations for this phase (ADR-008 §2).

The owner asked for the Organization page to look and work like the UniFi Network topology map:
hire, drag, assign, and move agents, and assign supervisors, QA evaluators, and security
auditors, on the canvas itself.

## Decision

1. **Positions, agent instances, oversight.** Migration 0004 adds:
   - `positions` — the organization chart: title, role, `reports_to` (another position, or
     `NULL` for the owner), runtime, optional model, `active | archived`, sort key. Positions are
     archived, never deleted; a role's type and persistence cannot change once set (trigger).
   - Columns on `agent_instances`: `position_id`, `runtime_id`, `model`, `task_id` (the task a
     spawned worker exists for), `retired_at`.
   - `departments.head_position_id`; `projects.description`, `coordinator_position_id`,
     `allowed_runtimes` (JSON array), `capability_profile`, `status`.
   - `oversight` — `review | qa | security` assignments from one position to another (a team,
     identified by its lead), immutable except for ending them.
   - `settings` — key/value JSON (the organization's name).
   - Expression indexes for tasks by `metadata.workforce.positionId` and sessions by
     `metadata.workforce.agentId`.
2. **Staffing follows the role.** A position whose role is persistent (superintendent,
   department manager, project coordinator, or a custom persistent role) is held by at most one
   active agent instance — its _incumbent_ — and may be vacant. The incumbent's conversation is
   one runtime session found by its metadata (`workforce.agentId`); a new objective resumes it,
   so continuity survives restarts. Changing a persistent position's runtime retires the
   incumbent and hires a new one (a runtime session belongs to one runtime). A position whose
   role is not persistent is _on demand_: each task delegated to it creates a new agent
   instance, recorded with the child task in one transaction.
3. **Structure rules, enforced in the Ledger transaction** (no check-then-write races):
   - a position reports to the owner or to an active **persistent** position, never to itself
     or anything below it;
   - a department is headed by a superintendent or department-manager position; a department
     manager heads exactly one department; department heads report to the owner or a
     superintendent;
   - a project has exactly one coordinator position, which reports to the head of the project's
     department; moving a coordinator under another department's head **reassigns the
     project** in the same transaction;
   - titles are unique (case-insensitively) within a team: among a position's direct reports
     and the positions overseeing it;
   - **orphan prevention:** a position with active reports, a department head, the coordinator
     of an active project, or a position with unfinished work cannot be archived; a department
     with projects cannot be deleted; an incumbent with an unfinished turn cannot be retired.
     Archiving a position ends its oversight assignments in the same transaction;
   - membership in a department or project is computed from the tree (the nearest department
     head or coordinator at or above a position), so it can never disagree with the reporting
     lines.
4. **Worker lifecycle follows its task, in the Ledger.** Every task state change goes through
   one function; when a task with a spawned worker starts, the worker becomes `active`
   (`org.worker_started`); when the task ends, the worker becomes `retired` (or `failed` when
   the task failed) with `retired_at` (`org.worker_retired`), in the same transaction. No path
   that ends a task — completion, failure, cancellation, dispatch failure, restart recovery —
   can leave a worker in the active workforce. Its tasks, events, and agent row remain.
5. **Role destinations through a Workforce directory.** Liaison gains a `Directory` hook (the
   same pattern as the runtime's `TurnHook`): for a session whose metadata marks it as an
   organization member, the directory supplies the worker's identity and _team_, and places
   `role:<name>` requests. A member's team is its on-demand direct reports plus the on-demand
   positions overseeing it; a spawned worker's team is its position's team (its peers). A name
   matches a team member's title, or its role name when that is unambiguous. A request is
   refused, with the reason given to the requester, when the name is not on the team, the
   position is archived, its runtime is unknown, or the project does not allow that runtime.
   Members address roles only: a raw runtime address from a member is refused, so every worker
   is part of the organization. Sessions outside the organization (the Workers view) behave as
   in Phase 4. The accepted request's child task carries `workforce.positionId`,
   `workforce.agentId`, the project, and the position's model; the child session is started on
   the position's runtime and model. Depth, request, round, and workflow limits are unchanged.
6. **Objectives to persistent agents.** The owner gives an objective to a staffed persistent
   position. Workforce starts or resumes the incumbent's session through Liaison
   (`start_member_session`, `resume_member_session`) with the member's identity, its team as
   destinations, and the task's project. A busy incumbent refuses a second objective rather
   than queueing it. A member session cannot be continued from the Workers view (it would
   bypass the organization).
7. **Capabilities and runtimes.** Projects record allowed runtimes (explicit; an empty list
   allows none) and a default capability profile name. Positions under a project must use an
   allowed runtime, checked when hiring, when giving objectives, and when placing a worker. No
   capability is granted: Guard (Phase 7) will read the recorded profile. The local working
   directory is recorded, never opened: workers still get their own empty workspace.
8. **Runtime choice is explicit, not policy.** Each position names the runtime (and optionally
   the model) that fills it, chosen by the owner. There are no fallbacks and no automatic
   selection; the Model Policy Engine (Phase 6) will replace this per-position choice.
9. **Not yet: delegating to persistent positions.** In Phase 5 only on-demand positions receive
   delegated tasks; a manager cannot hand an objective to a coordinator. Dispatching into a
   persistent session needs queueing and deadlock rules (two persistent members waiting on each
   other through oversight), which belong with the Development MVP workflow (Phase 8).
   Oversight assignments are therefore limited to on-demand overseers.
10. **Role templates are data.** Workforce seeds ten role templates at startup if they are
    missing (Superintendent, Department Manager, Project Coordinator, Senior Developer, Code
    Reviewer, QA Engineer, Security Auditor, Documentation Writer, Researcher, Designer), with
    purpose text from the rollout plan (§5) that goes into each worker's instructions. The
    owner can add custom roles. No department, project, or position is created automatically.
11. **Events.** Departments: `org.department_created`, `_updated`, `_deleted`. Projects:
    `org.project_created`, `_updated`, `_reassigned`, `_archived`. Positions:
    `org.position_created`, `_updated`, `_moved`, `_archived`. Agents: `org.agent_hired`,
    `org.agent_retired`; workers: `org.worker_spawned`, `_started`, `_retired`. Oversight:
    `org.oversight_assigned`, `_ended`. Settings: `org.settings_changed`. Worker events are on
    the worker's task, so each task's trail says which worker ran it.
12. **Topology canvas.** The Organization view is a UniFi-style topology map built from
    Plenipo's own components and original glyphs (no UniFi assets): owner → organization →
    positions, left to right, with bus connectors, labelled link chips (department, project,
    a worker's runtime), collapse toggles, live status on every node (dot plus text), animated
    links where work is running, oversight as dotted labelled links, pan/zoom/fit controls, and
    a minimap.
    Dragging a node onto another opens a menu of the moves and assignments that are valid there;
    dragging a role from the hire palette onto a node hires into that team. Drags use pointer
    events (HTML5 drag-and-drop is intercepted by the Windows webview). Every drag has a keyboard
    and inspector equivalent, and a searchable, filterable directory lists the whole organization
    (plan: the chart must not be the only navigation). The camera model — centre-anchored
    camera, cursor-anchored zoom, pinch, click-versus-drag threshold, eased fly-to, minimap — is
    adapted from the owner's Coastline plan canvas; unlike Coastline, nodes are real, focusable
    HTML elements over an SVG link layer, because the chart must be accessible.
13. **Code layout.** New crate `crates/workforce` (`plenipo-workforce`, planned by ADR-004): the
    service (snapshots, operations, objectives, live status, task ownership) and the directory.
    Transactional repository code lives in `crates/ledger/src/workforce.rs`. The runtime's
    `TurnTask::New` gains an optional project so a turn's task can belong to a project.

## Consequences

- The organization is data: nothing about departments or projects is hard-coded, and the same
  engine serves any department (plan §15).
- A worker's appearance and retirement are exact: they are written with its task's own state
  changes.
- Coordinators can only reach the workers the owner hired into their team; the team roster is
  written into their instructions, and a request for anyone else is refused and explained.
- The first version of role routing is deliberately literal (title or role name within a team).
  Phase 6 replaces the per-position runtime with policy; Phase 8 adds delegation between
  persistent positions.
- A busy persistent agent refuses new objectives; the owner waits or cancels.
- The canvas gives a spatial overview, but large organizations rely on the directory and the
  inspector; both work without a mouse.

## Alternatives considered

- **Agents only (no positions)** — an on-demand role would need a template agent that never
  runs, and vacancies, replacements, and history per incumbent would be muddled.
- **Department and project as tree nodes** — puts containers between people; UniFi-style maps
  read better with people as nodes and departments and projects as link labels.
- **A stored department on every position** — can disagree with the reporting lines; computing
  membership from the tree cannot.
- **Retiring workers in a reconciliation pass** — would miss or delay some ending paths; the
  task state machine already sees every one.
- **Letting members address raw runtimes** — workers would run outside the organization and
  never appear under their coordinator.
- **A graph library for the canvas** — adds a dependency for layout Plenipo can do in a few
  hundred tested lines, and styling it to this look is as much work as drawing it.
- **HTML5 drag-and-drop** — the Windows webview intercepts it for file drops.
