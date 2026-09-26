# Phase 5 — Acceptance Report

|              |                                                                                                                                                                       |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 5 — Workforce and Organization Engine                                                                                                                                 |
| **Branch**   | `claude/phase-5` ([PR #7](https://github.com/Seckcey/plenipo/pull/7))                                                                                                 |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, full `pnpm e2e`. GitHub CI: Rust, Frontend, E2E (Linux), Windows — see PR #7.                                |
| **Date**     | 2026-09-26                                                                                                                                                            |
| **Result**   | **The Phase 5 acceptance criterion passes end to end against fake CLIs.** Owner verification with the real Claude Code and Codex CLIs on Windows is pending (§7, O2). |

Screenshots: [empty organization](evidence/phase-5/org-empty.png) ·
[Development, its project, and the team](evidence/phase-5/org-team.png) ·
[workers under their positions](evidence/phase-5/org-workers-live.png) ·
[after the workers left](evidence/phase-5/org-after-workers.png) ·
[a worker's Ledger trail](evidence/phase-5/org-worker-trail.png) ·
[U.S. Army titles after a restart](evidence/phase-5/org-army-titles.png) ·
[list view after a restart](evidence/phase-5/org-list-after-restart.png).

Test totals: **352 Rust** (Linux) · **108 frontend** · **30 end-to-end**
against the real release binary (6 Phase 1 + 6 Phase 2 + 8 Phase 3 + 5 Phase 4 + 5 Phase 5).

**Owner direction during the phase:** plain, non-technical words on every screen, the chain of
command Worker → Supervisor → Manager → VP → President (you), and a **Titles** choice that names
the ranks after a U.S. military branch or the Mafia. The app now shows the plan's Superintendent /
Department Manager / Project Coordinator as VP / Manager / Supervisor and says "AI tool" instead
of "runtime" ([ADR-010](../adr/ADR-010-plain-titles.md),
[word list](../design/vocabulary.md)). Quotes from the plan below keep the plan's words.

CI has no AI tool accounts, so every automated test drives `plenipo-fake-agent`, the test
double installed as `claude` / `codex`. For Phase 5 a supervisor's objective such as
`Ship the pricing page [handoff:role:Senior Developer+delay:6000]` makes the fake supervisor
hand the Senior Developer a task — exactly the `role:` request a real supervisor is instructed
to write — whose worker takes six seconds, so it can be watched on the canvas.

## 1. Acceptance criterion → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                                                                                                                                                 | Result (fake CLIs) | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | The user can view Development as a department, select a project, give a coordinator an objective, and observe one or more workers appear under that coordinator and disappear from active workforce after completion while history remains. | **Pass**           | E2E `starts empty, then builds Development, its Website project, and the team` builds it on the canvas ([screenshot](evidence/phase-5/org-team.png)). E2E `acceptance: a supervisor's objective puts workers under it, and they leave when done`: the supervisor (the plan's coordinator) is selected, given an objective, and waits on its team while a worker appears under the Senior Developer and one under the Code Reviewer — dashed links, AI tool chips, **Working**, "Live workers 2" ([screenshot](evidence/phase-5/org-workers-live.png)); both leave, the supervisor finishes **Idle**, "Live workers 0"; the Senior Developer still shows its retired worker and the task under **Work → Recent** ([screenshot](evidence/phase-5/org-after-workers.png)). E2E `the Ledger keeps the organization's trail`: the delegation tree and each worker's trail ("Worker brought in for …", "Worker finished and left the organization") ([screenshot](evidence/phase-5/org-worker-trail.png)). Integration `acceptance_workers_appear_under_the_coordinator_and_leave_when_done` checks the snapshot at every stage, the event order, and that agents, tasks, and trails remain. |

## 2. Required Phase 5 tests → evidence

Integration tests in `crates/workforce/tests/workforce.rs` run the real Workforce, Liaison,
agent runtime, supervisor, adapters, and a file-backed Ledger against the fake CLIs; unit tests
in `crates/ledger/src/workforce.rs` test every repository operation and rule.

| Test (ROLLOUT_PLAN.md)                  | Result   | Evidence                                                                                                                                                                                                                                                                                                               |
| --------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Create department                       | **Pass** | `plan_create_department_role_manager_and_project_coordinator` (the department comes with its head position, events in one transaction); Ledger `a_department_comes_with_its_head_and_a_project_with_its_coordinator`; IPC `the_organization_is_built_and_changed_through_ipc`; E2E build test                          |
| Create role                             | **Pass** | Same integration test (a custom on-demand role next to the seeded templates; duplicate names refused); `templates_cover_every_class_and_the_owners_oversight_roles`; frontend role dialog                                                                                                                              |
| Assign manager                          | **Pass** | Same integration test (a vacant head is filled: `org.agent_hired`); Ledger `a_persistent_position_keeps_one_incumbent_at_a_time` (one incumbent; a runtime change retires and rehires)                                                                                                                                 |
| Create project coordinator              | **Pass** | Same integration test (the project comes with its coordinator under the department head; only known runtimes; the project's allowed runtimes bind the coordinator); E2E build test                                                                                                                                     |
| Coordinator creates child worker        | **Pass** | `acceptance_workers_appear_under_the_coordinator_and_leave_when_done` (two `role:` requests → two workers recorded with their child tasks in the Liaison transaction, on the positions' runtimes); Liaison `a_member_hands_work_to_its_team_and_the_worker_leaves_when_done`; E2E acceptance test                      |
| Worker finishes and retires             | **Pass** | Same tests (the worker retires in the transaction that ends its task); `a_worker_that_fails_leaves_as_failed_and_the_coordinator_carries_on`; Ledger `a_spawned_worker_starts_and_retires_with_its_task` (success, failure, cancellation)                                                                              |
| Persistent coordinator survives restart | **Pass** | `plan_persistent_coordinator_survives_restart` (a new stack on the same Ledger file: same position, same agent, and the next objective resumes the same provider session); E2E `the organization, its supervisor, and the chosen titles survive a restart` ([screenshot](evidence/phase-5/org-list-after-restart.png)) |
| Department/project reassignment         | **Pass** | `plan_department_and_project_reassignment` (moving a coordinator under another department's head moves its project, `org.project_reassigned`; a worker moved to another team leaves the project); Ledger `moves_prevent_cycles_and_reassign_projects`                                                                  |
| Orphan prevention                       | **Pass** | `plan_orphan_prevention` (nobody left without a supervisor, no cycles, only persistent supervisors, no letting an agent go with unfinished work; archiving a project archives its whole team and ends oversight of it); Ledger `orphans_are_prevented`                                                                 |

Also: routing refusals (`requests_outside_the_team_are_refused_and_explained`: an unknown role,
a raw runtime from a member, a runtime the project does not allow — each refused with the reason
given to the coordinator, nothing switched); `members_address_their_team_by_role_and_never_a_raw_runtime`;
objectives only to staffed persistent positions and member sessions not continued from Workers
(`objectives_go_only_to_staffed_persistent_positions`); oversight routing (an overseer is on the
team it oversees); title uniqueness; the v3 → v4 upgrade (`phase4_ledger_upgrades_to_the_workforce`)
and the down migration; snapshot unit tests (tree membership, statuses from open tasks, vacancies,
unready runtimes, project policy on the nodes); IPC boundary tests for all 20 new commands (input
validation, no extra fields, refusals record nothing, denied for ungranted windows and remote
origins); frontend tests for the camera, the layout, the structure hints, the canvas (drag to
hire, drag to reassign or assign oversight, refused drops, collapse, search, list view, dialogs,
live reload), organization members in Workers, and the `org.*` trail descriptions. For the owner's
words (ADR-010): title sets, plurals, and articles; rule messages in the chosen ranks; the map and
details panel under Army titles; choosing titles in Settings; a template seeded under a former
name renamed in place (`a_renamed_template_keeps_its_role_and_positions`); settings merges that
keep other fields; and, end to end, Army titles chosen in Settings that survive a restart.

## 3. Defects found and fixed during Phase 5

| Found by              | Problem                                                                                                                                                                                                                                                                                                         | Fix                                                                                                                                                                                                                                                     |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Integration test      | A worker spawned for an overseer (a QA engineer reporting to the department head but overseeing a project's team) got no project, so the project's runtime policy did not apply to it.                                                                                                                          | Work for a team belongs to the team lead's project: the directory checks that project's allowed runtimes and files the worker's task under it; the worker's instructions name the team it serves.                                                       |
| Ledger tests          | Following every task transition queried the new agent columns, which broke opening a Phase 3 Ledger in the upgrade tests.                                                                                                                                                                                       | Only tasks whose metadata names a worker (and whose worker row names the task) are followed.                                                                                                                                                            |
| Integration test      | The orphan test expected a department head moved under its own coordinator to be refused as "not a superintendent"; the Ledger refused it first as a cycle.                                                                                                                                                     | Kept the Ledger's order (cycles first) and tested both refusals separately.                                                                                                                                                                             |
| Browser check         | Under React's StrictMode the canvas's fly-to-selection was cancelled by an effect cleanup and never retried, so a selected node could stay off screen.                                                                                                                                                          | The reveal is re-armed after an unmount, so the selection is always brought into view.                                                                                                                                                                  |
| Browser check         | The first view fitted large organizations at 40 % zoom, where node text is unreadable.                                                                                                                                                                                                                          | Organizations that do not fit readably open at 72 % on the chain of command (top left); **Fit** still shows everything.                                                                                                                                 |
| Browser check         | The drop menu could extend past the bottom of the window.                                                                                                                                                                                                                                                       | It is measured and kept on screen.                                                                                                                                                                                                                      |
| Frontend test (flaky) | The canvas attached its window pointer listeners in a passive effect, so a drag that began immediately after the first render could be missed.                                                                                                                                                                  | Listeners attach in a layout effect, as soon as the canvas is in the document.                                                                                                                                                                          |
| E2E (real app)        | In the default 1200 × 780 window the palette and the details panel left the map about 420 px wide, so **Fit** showed the organization at 20 %.                                                                                                                                                                  | The details panel floats over the map, and fit, fly-to, zoom, the minimap, drops, and edge panning use the part it leaves uncovered; the palette is compact and folds to a rail; the header is two compact rows.                                        |
| E2E (real app)        | The link glow used a blur filter, which is slow under software rendering (the map moved in ~0.5 s steps).                                                                                                                                                                                                       | The glow is a faint wide stroke under each link.                                                                                                                                                                                                        |
| E2E (real app)        | "QA evaluator" lost its capitals mid-sentence ("the team's qa evaluator").                                                                                                                                                                                                                                      | Oversight roles have their own mid-sentence names.                                                                                                                                                                                                      |
| E2E (harness, flaky)  | A relaunch in the Phase 3 suite could ask for a WebDriver session before the native driver that tauri-driver starts was listening, and was refused.                                                                                                                                                             | The launcher waits for both drivers' ports.                                                                                                                                                                                                             |
| CI (Windows, flaky)   | Since the Phase 4 fix merged from `main`, a finished turn reads as not running once its result is recorded, a moment before the runtime releases its session. Test helpers returned in that moment and a follow-up in the same session was refused as "already running" (`main` fails the same way on Windows). | The runtime, Liaison, and Workforce test helpers also wait for the release. Holding the release back 150 ms (not committed) failed 6 tests before the fix and none after.                                                                               |
| CI (Windows)          | The same Phase 4 change had a second window: a turn reads as waiting as soon as its wait is recorded, a moment before the runtime moves it there. A cancel in that moment stopped the step that had already finished, and the turn went on waiting — the cancel was lost.                                       | Cancelling ends a wait the turn has just entered (claimed for the cancel in the same look). A new test cancels from inside that exact moment and fails without the fix; the waiting-turn test helper waits until the runtime holds the turn as waiting. |
| E2E (real app)        | Right after a restart, **Fit** showed the organization at 48 % instead of 60 %: the canvas learns its size from a resize observer that reports only with the next painted frame, so a command in that moment used the 960 × 640 placeholder (a drop then would have mapped to the wrong node).                  | Fit, zoom, reveal, the drop hit-test, wheel, keys, and pointer-down measure the canvas when they run.                                                                                                                                                   |

## 4. Deliverables

| Deliverable (plan)                   | Location                                                                                                                                                                 |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Organization schema                  | `crates/ledger/migrations/0004_workforce.{up,down}.sql`; `crates/ledger/src/workforce.rs` (rules in the transaction, `org.*` events)                                     |
| Department definitions               | Departments with their head position (`create_department_with_head`); membership computed from the tree                                                                  |
| Role templates                       | `crates/workforce/src/templates.rs` (ten templates seeded as data; custom roles; leadership shown as VP, Manager, Supervisor, renamed in place in existing Ledgers)      |
| Persistent managers and coordinators | Positions with one incumbent agent and one conversation (`crates/workforce/src/service.rs`: hire, fill, vacate, objectives)                                              |
| Project configuration                | Name, repository, local directory (recorded), department, coordinator, allowed runtimes, capability profile (recorded)                                                   |
| Ephemeral workers                    | Recorded with their child task by Liaison (`insert_worker`), retired with it (`follow_task`)                                                                             |
| Role routing                         | `crates/liaison/src/directory.rs` (hook), `crates/workforce/src/directory.rs` (teams, placement, refusals)                                                               |
| Org tree UI                          | `apps/desktop/src/views/OrganizationView.tsx`, `components/org/*` (topology canvas, hire palette, drop menu, dialogs), `org/*` (camera, layout, rules — pure and tested) |
| Agent cards and status indicators    | Nodes (status dot plus text) and the details panel (`components/org/Inspector.tsx`)                                                                                      |
| Task ownership views                 | Details panel → **Work** (running, waiting, queued, recent, team) via `get_work`                                                                                         |
| Directory                            | List view with filters (`components/org/Directory.tsx`) and search                                                                                                       |
| Commands                             | 20 Workforce commands (architecture overview §3), each granted by name                                                                                                   |
| Plain words and Titles (owner)       | [Word list](../design/vocabulary.md); `apps/desktop/src/org/titles.ts`; Settings → Personalization → Titles (`set_organization_titles`)                                  |
| Test double                          | `plenipo-fake-agent`: `role:` destinations and `[delay:MS]`                                                                                                              |
| Decision records                     | [ADR-009 — how the AI organization works](../adr/ADR-009-workforce.md), [ADR-010 — plain words and rank names](../adr/ADR-010-plain-titles.md)                           |

## 5. Security notes

- The UI names positions, roles, and teams only. It cannot address a runtime session, pick a
  worker's session, or grant anything; every command validates its input, refuses unknown
  fields, and is granted by name (IPC tests).
- Supervisors reach only the positions the owner put on their team; a request for anyone else,
  or for a raw AI tool address, is refused with the reason, so every worker appears in the
  organization.
- Title sets are display only: agents never receive a military or Mafia rank, so a persona
  cannot reach an agent's instructions.
- A project's allowed runtimes are enforced when hiring, when moving, when giving objectives,
  and when placing each worker; an empty list allows none. There is no fallback to another
  runtime.
- No capabilities are granted: a project's local folder and capability profile are recorded
  for Guard (Phase 7) and never used. Workers keep the Phase 3 least-privilege posture in their
  own empty workspace.
- Structure rules live in the Ledger transaction, so concurrent changes cannot create cycles or
  orphans; the canvas only mirrors them to highlight valid drops.

## 6. Deviations from the plan

| Deviation                                                                                      | Why                                                                                               | Recorded        |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | --------------- |
| Positions (the chart) are separate from agent instances (who fills them)                       | Vacancies, replacements, and per-incumbent history need both                                      | ADR-009 §1–2    |
| Departments and projects are link labels on the canvas, not nodes                              | People as nodes read better on a topology map; membership is computed from the tree               | ADR-009 §3, §12 |
| Only on-demand positions receive delegated tasks; managers do not delegate to coordinators yet | Dispatching into persistent sessions needs queueing and deadlock rules (Development MVP, Phase 8) | ADR-009 §9      |
| Each position's runtime is the owner's explicit choice                                         | Model policy is Phase 6                                                                           | ADR-009 §8      |
| Members address roles only, never raw runtimes                                                 | Every worker must appear in the organization                                                      | ADR-009 §5      |
| Plain words on screen: VP / Manager / Supervisor / President instead of the plan's names       | Owner direction during the phase; the code keeps the plan's names                                 | ADR-010         |
| Titles personalization (U.S. military branches, Mafia), not in the plan                        | Owner request; display only, stored with the organization                                         | ADR-010         |

## 7. Owner items

| ID  | Item                                                                                                                                                                                             | Recommendation                   |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------- |
| O1  | ADR-009 (how the AI organization works: seats, teams, and the org map) and ADR-010 (plain words and rank names) are **Proposed**.                                                                | Accept or amend.                 |
| O2  | Windows check with the **real** CLIs and your subscription sign-ins (~20 min): the steps in [phase-5-checklist.md](phase-5-checklist.md#owner-check-on-windows-20-minutes). Report anything odd. | Required for acceptance.         |
| O3  | Version stays **0.5.0** until O2 passes; then **0.6.0** per the phase convention.                                                                                                                | Bump after O2.                   |
| O4  | Phase 6 (Model Policy Engine) replaces the per-position AI tool choice with policy.                                                                                                              | Say "start Phase 6" after O1–O2. |
| O5  | Title sets: the military sets use rank names only (no insignia, seals, or logos, which are protected); the Mafia set is opt-in because some people find Mafia stereotypes offensive.             | Keep, rename, or drop any set.   |

Things only the real CLIs can confirm (O2): that a supervisor follows its instructions and
hands work to its team by title (a request for anyone else is refused and explained, so the task
still finishes), and how long a team round trip takes on real subscriptions.

## 8. Verification

| Check                                                                     | Result                                                                           |
| ------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `pnpm check` (versions, format, lint, typecheck, tests)                   | Pass — 108 frontend tests                                                        |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | Pass                                                                             |
| `cargo test --workspace`                                                  | Pass — 352 tests, including 8 Workforce integration tests and 2 new Liaison ones |
| `pnpm e2e` against the release build (Linux, Xvfb)                        | Pass — 30 of 30, including the 5 Phase 5 tests                                   |
| Generated TypeScript bindings                                             | Up to date (`pnpm bindings` leaves no diff)                                      |
| GitHub CI on the PR                                                       | Linked from the PR                                                               |

## 9. Phase boundary

Phase 5 is implemented and verified against fake CLIs. It is complete once the owner accepts it
(§7). Phase 6 has not been started.
