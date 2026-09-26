# Phase 8 — Implementation Checklist

**Status:** in progress on `claude/phase-8`.

Source: `ROLLOUT_PLAN.md`, Phase 8 — Development Department MVP. Phase 7 is accepted and released
as v0.8.0 ([PR #14](https://github.com/Seckcey/plenipo/pull/14)); the owner asked to begin Phase 8
on 2026-09-26. This checklist keeps the plan's words where it quotes the plan; the app uses the
plain words in [`docs/design/vocabulary.md`](../design/vocabulary.md) (VP, Supervisor, and Worker,
not superintendent, coordinator, or ephemeral worker).

**Goal:** reproduce the useful management behavior of the Codex development hierarchy under
Plenipo, across Codex and Claude Code: the owner gives the Development VP an objective and a
project, and gets back one result with everything the plan asks for.

This is the MVP boundary (plan §4): at the end of Phase 8 the version becomes **1.0.0**.

## What already exists (Phases 0–7)

Departments, projects, full-time and on-call positions, and a team per lead (Phase 5); model
choices per role, with cross-company review (Phase 6); permissions, a project folder, approvals,
and git tools (Phase 7). Missing for the MVP: the VP cannot hand work to a Supervisor ("delegation
between persistent positions waits for Phase 8", ADR-009 §9), workers share the project folder,
there are no GitHub tools, and the result is only the VP's own text.

## Design decisions (details in ADR-016, the Development department)

- **Leads hand work to full-time reports.** A lead's team now includes its staffed full-time
  direct reports (a VP's Supervisors and Managers), addressed like any member
  (`role:Website Supervisor`). The task runs in that member's own conversation, so it keeps its
  memory. No new worker is brought in for it.
  - **Queueing:** a member that is busy (running or waiting on its own team) takes the next task
    when it is free; the task shows "waiting for Website Supervisor to finish its current task".
  - **No deadlocks:** work only goes down the reporting lines (to direct full-time reports) or to
    on-call team members, and reporting lines never loop (the Ledger refuses cycles), so no two
    members can wait on each other. Oversight stays on-call only.
  - **Stopping** a delegated task stops only that task in the member's conversation, never
    another one it is working on.
  - **Limits:** reply rounds per task 5 → 8 and handoffs per workflow 12 → 16, so a Supervisor can
    run review and repair loops (implement → review → fix → review again → test → fix → test).
- **A branch and a working copy per objective (the plan's git worktrees).** When a worker of an
  objective first needs the project folder and the folder is a git repository, Plenipo creates a
  working copy (a `git worktree`) on a new branch `plenipo/<objective>-<id>` from the project's
  current commit, in Plenipo's own data folder. All workers of that objective work there, so the
  reviewer and the tester see the developer's changes, and your own checkout is never touched.
  - **Writers take turns:** while one worker that can change files holds the working copy, a
    second one of the same objective gets its own working copy on its own branch
    (`…-2`), and the result says to merge it.
  - **No worker can overwrite another's branch or working copy:** other working copies are outside
    its folder, and in a working copy the git tools may not switch branches, and may push only
    that objective's branch (pushing still asks you).
  - **Recorded:** each working copy (`workspace.created`), and after each step that used it, its
    branch, commits, and changed files (`workspace.updated`). Projects that are not git
    repositories, or where you turn it off (**Edit project → Work on a separate branch for each
    objective**), work in the project folder as in Phase 7.
  - **Remove** a finished objective's working copy from the Projects page; its branch stays.
- **GitHub tools** (github.read, github.write), through GitHub's own `gh` program, signed in with
  `gh auth login` or given a GitHub token from **Secrets** (never shown to workers): list, view,
  and check pull requests, read an issue, and open a draft pull request for the objective's
  branch (it pushes the branch first). Opening a pull request is publishing, so it always asks
  you. The tools act only on the project's own repository. Built-in permission sets gain GitHub
  reading (and the Developer set GitHub writing) — for existing installations only when you had
  not changed that set.
- **Development playbook** in the instructions: the VP picks the Supervisor for the project; a
  Supervisor with a team breaks the objective into bounded tasks, has the developer implement and
  commit, the reviewer review (another AI company when configured), repairs until approved, has
  QA run the acceptance checks, and puts the result together. Reviewers, QA, and security
  auditors end with a short `plenipo-review` block: their verdict and findings.
- **The result (Plenipo's record, not the agent's word).** For every objective, Plenipo builds a
  result from the Ledger: tasks performed, workers and AI models used, files changed, tests and
  programs run with their outcome, branch, commits and pull request, unresolved review findings,
  approvals still waiting (and those refused), and blocked requests — next to the VP's own
  summary. Live while it runs.
- **Projects page** (the project dashboard): each project with its Supervisor, folder,
  repository, current objective, workers now, approvals waiting, recent objectives and their
  results, and working copies. **Give Development an objective** from there: pick the project,
  type the objective, and it goes to the Development VP (or straight to the Supervisor).
- **Development template:** **Set up Development** creates the Development department with its VP,
  and a project with its Supervisor and the standard team (Senior Developer, Code Reviewer,
  QA Engineer, Documentation Writer), in one change. It is data, not code: the same engine as
  every department.

## Deliverables (plan)

- [ ] Development Superintendent (the Development VP, a department head that takes objectives and
      hands them to the right Supervisor)
- [ ] Project coordinators (Supervisors that take delegated objectives in their own conversation)
- [ ] Developer worker role
- [ ] Reviewer role (verdict and findings)
- [ ] QA role (acceptance checks, verdict)
- [ ] Documentation role
- [ ] Git integration (a branch and working copy per objective, branch guard, commits recorded)
- [ ] GitHub integration (pull requests and issues through `gh`)
- [ ] Task decomposition (the Supervisor's playbook; bounded tasks through Liaison)
- [ ] Review loop (review → repair → review again, within the limits)
- [ ] Project dashboard (Projects page)
- [ ] ADR-016; architecture, README, setup, vocabulary updated

## Phase 8 tests (plan: synthetic development scenarios)

- [ ] Documentation-only change
- [ ] Small bug fix
- [ ] Feature with implementation + review
- [ ] Failed tests and repair
- [ ] Concurrent workers
- [ ] Reviewer requests changes
- [ ] Provider failure mid-task
- [ ] Coordinator restart

## Acceptance criteria (plan)

- [ ] The user can type an objective comparable to "Have Development implement feature X in
      project Y and get it ready for review", and Plenipo delegates it to the right coordinator
      and mixed-provider workers without the user opening Codex or Claude sessions.
- [ ] The final result includes: tasks performed; agents/models used; files changed; tests
      executed; commit/branch/PR information where applicable; unresolved findings; approvals
      still required.

## MVP boundary (plan §4)

- [ ] 1 Launch on Windows · 2 View Development · 3 Select or name a project · 4 Give the VP an
      outcome · 5 Route it to the project's Supervisor · 6 Workers on Codex and Claude Code ·
      7 Models by role · 8 Only through Liaison · 9 Controlled file, git, and program permissions ·
      10 Implementation, review, and tests · 11 Live status on screen · 12 The whole task tree and
      history kept · 13 Approval for sensitive actions · 14 A synthesized result

## Out of scope

Fully autonomous production releases, every 8 West project, the sales and marketing departments
(plan). Also: merging branches automatically (the result says what to merge), GitHub reviews and
merges by workers, and operating-system sandboxing of approved programs.

## Owner check on Windows

Written with the acceptance report (it needs the finished screens).
