# ADR-016: The Development department — delegation, working copies, GitHub, and the result

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 8

> **On screen** (ADR-010, plain words and rank names): the plan's Development Superintendent is
> the **Development VP**, project coordinators are **Supervisors**, git worktrees are **working
> copies**, and the final result is the objective's **result**. This ADR keeps the plan's and the
> code's words where they differ.

## Context

Phase 8 is the MVP boundary (plan §4). The owner types an objective such as "Have Development
implement feature X in project Y and get it ready for review", and Plenipo delegates it to the
right Supervisor and to workers on both AI tools, without the owner opening Codex or Claude
sessions. The final result must include the tasks performed, the agents and models used, the
files changed, the tests run, commit, branch, and pull request information, unresolved
findings, and approvals still required. The plan's deliverables are the Development
Superintendent, project coordinators, developer, reviewer, QA, and documentation roles, git and
GitHub integration, task decomposition, a review loop, and a project dashboard. Its tests are
eight synthetic development scenarios.

What Phases 0–7 left missing:

- **A VP could not hand work to a Supervisor.** ADR-009 §9 deferred delegation between
  persistent positions: a lead's team held only on-call members.
- **All workers of a project shared its folder**, the owner's own checkout. Two objectives, or a
  developer and a tester, could step on each other.
- **No GitHub tools.** The GitHub capabilities were registered in Phase 7, but had no tools
  (ADR-013 §14).
- **The only result was the lead's own text**, so what was done depended on what the model
  chose to say.

## Decision

1. **Leads hand work to full-time reports** (supersedes the deferral in ADR-009 §9).
   - A lead's team now includes its staffed full-time direct reports (a VP's Managers and
     Supervisors), addressed like any member (`role:Website Supervisor`).
   - The task runs as a new turn in that member's own conversation, so it keeps its memory. No
     worker is brought in for it.
   - **Queueing:** a member that is busy takes the next task when it is free. Its turn is
     reserved atomically, and a second request gets `SessionBusy` and waits.
   - **No deadlocks:** work only goes down reporting lines or to on-call members, and reporting
     lines cannot loop (the Ledger refuses cycles), so two members never wait on each other.
     Oversight stays on-call only.
   - **Stopping** a delegated task stops only that turn (`cancel_task`), never another turn of
     the member.
   - **Giving an objective for a project** (`give_objective` with `projectId`): the position's
     team must run the project, and the objective names it. The objective's tasks carry the
     project, so the Projects page and the result can find them.
2. **A branch and a working copy per objective** (the plan's git worktrees).
   - When a worker of an objective first needs the project folder, and the folder is committed
     content of a git repository with at least one commit, Plenipo runs `git worktree add` on a
     new branch `plenipo/<objective>-<id>` from the checkout's current commit. The working copy
     goes in Plenipo's data folder (`working-copies`), never in the repository.
   - All workers of that objective work there, so the reviewer and the tester see the
     developer's changes, and the owner's checkout is never touched.
   - A folder inside a repository it is not committed to (ignored, not yet added, or another
     project's build folder) works in place, as in Phase 7. So do projects that turn it off
     (**Work on a separate branch for each objective**, on by default).
   - Plenipo's own git operations run without hooks, with a cleared environment, never asking
     for a password, and with a time limit.
3. **One writer per working copy.**
   - A worker that can change files holds its objective's working copy for its step.
   - A second writer of the same objective, at the same time, gets its own working copy on
     `<branch>-2` (and so on), made from the first. The result says to merge it into the
     objective's branch.
   - Readers (reviewers, QA) share the working copy.
4. **The branch guard.** In a working copy, the git tools may not switch or create branches,
   and may push only the objective's own branch. Pushing still asks the owner (ADR-013). Other
   working copies are outside the worker's folder, so no worker can overwrite another's branch
   or files.
5. **Working copies are recorded.**
   - Each working copy is a row in the Ledger's `workspaces` table (schema version 6), with
     `workspace.created`, `workspace.updated` (after each step that used it: its commits,
     changed files, uncommitted count, pushed), and `workspace.removed` events.
   - Removing a working copy (`remove_workspace`, the Projects page) deletes its folder and
     keeps its branch. It is refused while a worker is using it or its objective is still going.
6. **GitHub tools, through GitHub's own `gh` program.**
   - github.read: `github_pr_list`, `github_pr_view`, `github_pr_checks`, `github_issue_view`.
   - github.write: `github_pr_create`, which pushes the objective's branch and opens a **draft**
     pull request for it.
   - Opening a pull request is publishing, so it always asks the owner (sensitive kind
     "outbound"), even when a permission set allows GitHub writing.
   - The tools act only on the project's own repository, from its repository address
     (`https://github.com/owner/name` or `git@github.com:owner/name.git`); a project without one
     has no GitHub tools.
   - `gh` runs with prompts, pager, color, and update checks off. It is signed in with
     `gh auth login`, or given a token as a Vault secret for the `gh` program (`GH_TOKEN`),
     never shown to workers.
7. **Built-in permission sets gain GitHub.** The Read only, Developer, Reviewer, and Tester sets
   can read GitHub, and the Developer set can also write. An existing installation gets the new
   sets only for sets the owner had not changed (recorded as `guard.sets_updated`).
8. **The Development playbook and verdicts.**
   - A lead with full-time reports is told to hand each objective to the member who leads the
     work and to report back in a few lines.
   - A Supervisor with a team is told the playbook: bounded tasks, implement and commit on the
     objective's branch, review (passing the developer's task as context), repair until
     approved, QA runs the tests and acceptance checks, documentation if it changed, and a short
     report. It opens a pull request only when the objective asks for one.
   - Code Reviewers, QA Engineers, and Security Auditors (and custom roles marked so) end their
     answer with a `plenipo-review` block: a verdict (`approve` or `request-changes`) and
     findings (`blocker`, `major`, `minor`, with a file and a one-line summary).
9. **The result is Plenipo's record, not the model's word.** `get_objective_report` builds it
   purely from the Ledger: tasks performed (the whole tree, in order), workers with their AI
   tool and model, files changed (from the working copy facts), programs run and whether they
   passed (tests recognized by name), branches with their commits, pull requests (from
   `github_pr_create` results), every verdict, the findings of each reviewer's latest verdict,
   approvals (waiting, approved, denied, expired), blocked requests, and problems to know about
   (failed tasks, uncommitted work, branches to merge). The VP's own answer sits beside it. It
   is live while the objective runs.
10. **The Development template.** `set_up_development` creates the Development department with
    its VP when missing, then the project with its Supervisor and the standard team — Senior
    Developer, Code Reviewer, QA Engineer, Documentation Writer, all on call and automatic (each
    role's model choices pick) — in one change. It is data over the Phase 5 engine, not a
    special department.
11. **The Projects page** (the project dashboard): each project with its Supervisor, folder,
    repository, and branch setting; give an objective to the department's head or the
    Supervisor; the project's objectives with their state, counts, approvals waiting, and
    branch; the selected objective's result, live; and its working copies with **Remove**.
    `get_project_work` feeds it.
12. **Limits.** Reply rounds per task 5 → 8 and handoffs per workflow 12 → 16, so a Supervisor
    can run review and repair loops.

## Consequences

- The owner can give Development an objective in one sentence and get back one result with
  everything the plan asks for, recorded rather than claimed.
- **Merging is the owner's.** Plenipo never merges branches or pull requests. A second working
  copy's branch must be merged by hand (or by a worker, in the objective's own working copy),
  and the result says so.
- **Working copies take disk space.** Each objective gets a full checkout of the repository in
  Plenipo's data folder until the owner removes it. The Projects page lists them.
- **A project folder must be committed** to get working copies. A new folder that is not yet
  in any commit works in place until it is committed.
- **`gh` must be installed and signed in** for GitHub tools. Without it the tools say so; the
  rest of the work goes on.
- **The verdict block is the model's.** Plenipo reads verdicts and findings as given; a reviewer
  that leaves the block out is recorded as giving no verdict, and the result shows no review.
- **A full-time member works on one task at a time.** A VP with several objectives for the same
  Supervisor queues them.
- Schema version 6 adds one column and one table. An older Plenipo refuses a newer Ledger, as
  before (ADR-006).

## Alternatives considered

- **New sessions for delegated work to full-time members.** Simpler, but the member would lose
  its memory of the project, which is why it is full-time.
- **Workers in the owner's own checkout, on a branch.** One `git switch` changes the owner's
  files under them, and two objectives cannot run at once.
- **Clones instead of worktrees.** Independent, but slower, heavier, and they need their own
  remote setup. Worktrees share the object store and branches with the owner's repository.
- **Letting workers merge or push anything.** Rejected: the plan rules out autonomous releases,
  and one objective must never overwrite another's branch.
- **GitHub's REST API with Plenipo's own token.** It would need a new credential flow. `gh` is
  already how developers sign in, it handles enterprise hosts, and a token can still come from
  the Vault.
- **The result as the VP's summary only.** Rejected by the acceptance criteria: the files,
  tests, and approvals must be facts, not the model's account of them.
