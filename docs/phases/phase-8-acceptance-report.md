# Phase 8 — Acceptance Report

|              |                                                                                                                                                                                                                       |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 8 — Development Department MVP                                                                                                                                                                                        |
| **Branch**   | `claude/phase-8` ([PR #19](https://github.com/Seckcey/plenipo/pull/19))                                                                                                                                               |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, full `pnpm e2e` against the release build. GitHub CI: Rust, Frontend, E2E (Linux), Windows — see PR #19.                                                     |
| **Date**     | 2026-09-27                                                                                                                                                                                                            |
| **Result**   | Both acceptance criteria and all eight synthetic development scenarios pass end to end against stand-ins for Claude Code, Codex, and `gh`. Version **1.0.0**. The owner's Windows check with the real CLIs is §7, O2. |

Screenshots (from the end-to-end run): [setting up Development](evidence/phase-8/development-setup.png)
· [the department and team in the organization](evidence/phase-8/development-organization.png) ·
[an objective at work, its pull request waiting for approval](evidence/phase-8/development-waiting.png)
· [the result](evidence/phase-8/development-result.png) ·
[tests, reviews, findings, branch, and pull request](evidence/phase-8/development-result-checks.png)
· [working copies](evidence/phase-8/development-working-copies.png).

Test totals: **570 Rust** (Linux) · **144 frontend** · **43 end-to-end**
against the real release binary (6 Phase 1 + 6 Phase 2 + 8 Phase 3 + 5 Phase 4 + 5 Phase 5 + 5
Phase 6 + 4 Phase 7 + 4 Phase 8).

On screen the plan's Development Superintendent is the **Development VP**, project coordinators
are **Supervisors**, git worktrees are **working copies**, and the final result is the
objective's **result** ([word list](../design/vocabulary.md)). Quotes from the plan keep the
plan's words.

CI has no AI tool or GitHub accounts, so every automated test drives `plenipo-fake-agent`,
installed as `claude` and `codex`, and as two helper programs: `gh` (it records pull requests in
a file next to it) and `verify FILE WORD` (a project test that passes when the file contains the
word). In Phase 8 the fake workers follow a **script**: for each position title, one step per
turn — what it says, the Plenipo tools it calls (through the real tool relay), the tasks it hands
on, its review verdict, or a crash or usage limit. So every scenario runs the same way every
time, on a real git repository with a bare "server" repository next to it.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                                                                                                                                                                   | Result (stand-ins) | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | The user can type an objective comparable to "Have Development implement feature X in project Y and get it ready for review", and Plenipo delegates it to the right coordinator and mixed-provider workers without the user opening Codex or Claude sessions. | **Pass**           | Integration `acceptance_development_implements_a_feature_and_gets_it_ready_for_review`: the owner gives the Development VP that objective for Website; the VP hands it to the Website Supervisor (in its own conversation), whose team implements and commits (Senior Developer), reviews (Code Reviewer, on **Codex**, a different AI company than the Supervisor's Claude Code), tests (QA Engineer), and opens a draft pull request after approval. E2E `acceptance: Development implements a feature and gets it ready for review` does the same in the real app from the Projects page; [waiting](evidence/phase-8/development-waiting.png). |
| 2   | The final result includes: tasks performed; agents/models used; files changed; tests executed; commit/branch/PR information where applicable; unresolved findings; approvals still required.                                                                  | **Pass**           | Same tests: the result lists all six tasks and who did them, five workers with AI tool and model, `src/login.txt` (+1 −0, committed), `verify src/login.txt login` passed, the branch `plenipo/…` from `main` with its commit, pushed, pull request #1 with its link, the reviewer's open minor finding, the approval (waiting while it waited, then approved), and the VP's answer. [Result](evidence/phase-8/development-result.png), [details](evidence/phase-8/development-result-checks.png). The owner's checkout is unchanged (`git status` clean, still on `main`).                                                                       |

## 2. Required Phase 8 tests → evidence

`crates/capabilities/tests/development.rs` runs the whole stack — Workforce, Router, Liaison,
the agent runtime and supervisor, Guard, the broker with its tool server and relay, and a
file-backed Ledger — with the Development department set up from its template (Development VP
and Website Supervisor on Claude Code, the team automatic), on a real git repository.

| Test (ROLLOUT_PLAN.md)               | Result   | Evidence                                                                                                                                                                                                                                                                                                                           |
| ------------------------------------ | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Documentation-only change            | **Pass** | `scenario_documentation_only_change`: the Documentation Writer changes README.md in the objective's working copy and the reviewer approves; one file, no tests run, no findings. The writer's set cannot save to git, so the result says the change is not committed                                                               |
| Small bug fix                        | **Pass** | `scenario_small_bug_fix`: given straight to the Supervisor; one commit changing `src/app.txt` (+1 −1), the test passes, no problems                                                                                                                                                                                                |
| Feature with implementation + review | **Pass** | `acceptance_development_implements_a_feature_and_gets_it_ready_for_review` (above)                                                                                                                                                                                                                                                 |
| Failed tests and repair              | **Pass** | `scenario_failed_tests_and_repair`: QA's first run fails and its verdict requests changes with the test output; the Supervisor sends the findings to the developer, who fixes and commits; QA's second run passes. The result shows both runs, two commits, and no open findings                                                   |
| Concurrent workers                   | **Pass** | `scenario_concurrent_workers`: two developers of one objective change files at the same time; the second gets its own working copy on `<branch>-2`, each file is on exactly one branch, and the result says to merge the second into the first                                                                                     |
| Reviewer requests changes            | **Pass** | `scenario_reviewer_requests_changes`: while the first review stands, the result shows its blocking finding; the developer fixes it, the second review approves, and the finding is gone                                                                                                                                            |
| Provider failure mid-task            | **Pass** | `scenario_provider_failure_mid_task`: the developer's AI tool crashes; the Supervisor is told, hands the task again, and the objective succeeds (the failed task stays in the result). Also `scenario_a_usage_limit_mid_task_never_switches_ai_company`: a usage limit fails the task and the retry stays with the same AI company |
| Coordinator restart                  | **Pass** | `scenario_coordinator_restart`: Plenipo stops while the developer works for the Supervisor. On restart nothing is left running, the objective ends as interrupted and the result says so, and its working copy stays. The Supervisor keeps its agent and conversation: the VP's next objective goes to it and finishes             |

Also for working copies and GitHub: `an_objective_gets_its_own_branch_and_working_copy_that_its_team_shares`,
`workers_stay_on_their_objectives_branch` (no switching branches, pushing only its own branch),
`a_second_worker_changing_files_at_the_same_time_gets_its_own_working_copy`,
`a_project_can_work_in_its_folder_instead`, `removing_a_finished_objectives_working_copy_keeps_its_branch`,
`a_developer_opens_a_draft_pull_request_for_its_branch_only_after_approval`, and
`github_tools_use_only_the_projects_repository_and_the_workers_permissions`. Delegation
(`crates/workforce/tests/workforce.rs`): `a_manager_hands_an_objective_to_its_supervisor_who_does_it_in_its_own_conversation`,
`a_busy_supervisor_takes_a_handed_over_objective_when_it_is_free`,
`a_handover_starts_a_supervisors_first_conversation_and_work_only_goes_down`,
`cancelling_a_handed_over_objective_stops_only_that_task`, and
`the_development_template_sets_up_a_department_project_and_team`. Unit tests cover branch names,
repository detection, `gh` output, GitHub addresses, tool arguments, verdict blocks, the result's
problems, the Ledger's workspaces (schema 6), and the upgrade of unchanged built-in permission
sets. IPC boundary tests cover the four new commands (through IPC, input validation, refused extra
fields, denial for ungranted windows and remote origins) and the new limits. Frontend tests cover
the Projects page, the result card, giving an objective for a project, removing a working copy,
live reloading, and setting up Development. All earlier phases' tests pass.

## 3. Defects found and fixed during Phase 8

| Found by          | Problem                                                                                                                                                                                 | Fix                                                                                              |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Integration tests | A second worker could start changing files before the first one's hold on the working copy was recorded.                                                                                | Deciding who holds a working copy and recording it happen under one lock.                        |
| Integration tests | A task waiting in a busy member's queue counted as one of its conversation's turns, so the next turn got the wrong number.                                                              | Only started tasks count.                                                                        |
| Integration tests | Scripted steps were remembered across scripts, so a later test's worker skipped steps.                                                                                                  | Writing a script clears the record of used steps.                                                |
| Local test run    | A project folder inside a repository it is not committed to (ignored, or another project's build folder) got a working copy of that outer repository, which does not contain its files. | Only a folder committed to its repository gets working copies; others work in place (unit test). |
| Screenshot review | The project folder hint showed doubled backslashes (`D:\\projects\\website`).                                                                                                           | Single backslashes.                                                                              |
| Screenshot review | A worker whose AI tool named no model showed only the AI tool.                                                                                                                          | "Codex · its default model".                                                                     |

## 4. Deliverables

| Deliverable (plan)                          | Location                                                                                                                                                                                         |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Development Superintendent (Development VP) | Template `crates/workforce/src/templates.rs` (`DEVELOPMENT`); the VP hands objectives to the project's Supervisor (`directory.rs`, `conversation.rs`) and reports back (`prompt.rs`, delegation) |
| Project coordinators (Supervisors)          | Take handed-over objectives in their own conversation, queued when busy; the development playbook (`prompt.rs`)                                                                                  |
| Developer, reviewer, QA, documentation      | Template roles Senior Developer, Code Reviewer, QA Engineer, Documentation Writer; reviewers and QA give `plenipo-review` verdicts                                                               |
| Git integration                             | Working copies and the branch guard: `crates/capabilities/src/worktrees.rs`, `broker.rs`; Ledger `workspaces` (migration 0006)                                                                   |
| GitHub integration                          | `crates/capabilities/src/github.rs`, `tools.rs` (five tools through `gh`)                                                                                                                        |
| Task decomposition                          | The Supervisor's playbook; bounded tasks through Liaison, 8 rounds and 16 handoffs                                                                                                               |
| Review loop                                 | Review → repair → review again (`scenario_reviewer_requests_changes`, `scenario_failed_tests_and_repair`)                                                                                        |
| Project dashboard                           | Projects page: `apps/desktop/src/views/ProjectsView.tsx`, `components/ObjectiveResult.tsx`; `get_project_work`                                                                                   |
| The result                                  | `crates/workforce/src/outcome.rs` (`ObjectiveReport`, built from the Ledger); `get_objective_report`                                                                                             |
| Commands                                    | `set_up_development`, `get_objective_report`, `get_project_work`, `remove_workspace`; `give_objective` takes `projectId` (architecture overview §3)                                              |
| Decision record                             | [ADR-016 (the Development department: delegation, working copies, GitHub, and the result)](../adr/ADR-016-development-department.md)                                                             |

## 5. Security notes

- **The owner's checkout is never touched.** Workers of an objective are confined to its working
  copy, outside the repository, and may not switch branches or push anything but the objective's
  own branch. Other working copies are outside their folder.
- **Publishing always asks.** Pushing and opening a pull request stop for the owner's approval,
  even when a permission set allows them. Plenipo never merges.
- **GitHub tools act only on the project's repository**, named by its repository address; a
  worker cannot name another.
- **Plenipo's own git** (making and removing working copies, reading their facts) runs without
  the repository's hooks, with a cleared environment, never asking for a password, with a time
  limit.
- **A GitHub token** can be kept in Windows Credential Manager as a secret for `gh` only
  (`GH_TOKEN`); workers never see it, and it is hidden wherever it would appear.
- **Known limits** (ADR-016, Consequences): merging is the owner's; working copies use disk
  space until removed; reviews depend on the reviewer's verdict block; the Phase 7 limits
  (Codex's own reads, trusted approved programs) still apply.

## 6. Deviations from the plan

| Deviation                                                                                                   | Why                                                                                | Record       |
| ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ------------ |
| The Development Superintendent is a department head (the Development VP) rather than a separate rank        | Phase 5's engine already has department heads; the VP is data over it (a template) | ADR-016 §10  |
| Delegation between persistent positions, deferred in ADR-009 §9, is now allowed (down reporting lines only) | Needed for VP → Supervisor; no deadlocks because work only goes down               | ADR-016 §1   |
| Git worktrees live in Plenipo's data folder, one per objective, with a second one for a concurrent writer   | The owner's checkout is never touched; two writers never share files               | ADR-016 §2–3 |
| GitHub through the `gh` program, not GitHub's API directly                                                  | Uses the owner's own sign-in; no new credential flow                               | ADR-016 §6   |
| Limits raised: 8 reply rounds per task (was 5), 16 handoffs per workflow (was 12)                           | Review and repair loops                                                            | ADR-016 §12  |

## 7. Owner items

| ID  | Item                                                                                                                                                                                                                                                                                                                                                        | Recommendation                                |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| O1  | ADR-016 (the Development department: delegation, working copies, GitHub, and the result) — **Proposed**. Accepting it means: a VP hands work to its Supervisors, each objective works on its own branch in its own copy of the project folder, workers can open draft pull requests only with your approval, and the result is built from Plenipo's record. | Accept.                                       |
| O2  | Windows check with the **real** CLIs and `gh` (~30 min): the steps in [phase-8-checklist.md](phase-8-checklist.md#owner-check-on-windows-30-minutes). Use a scratch repository.                                                                                                                                                                             | Recommended with v1.0.0; fixes go in a patch. |
| O3  | Working copies use disk space: remove finished ones from **Projects → Working copies**.                                                                                                                                                                                                                                                                     | Remove after merging.                         |
| O4  | Version **1.0.0**: Phase 8 is the MVP boundary (plan §4).                                                                                                                                                                                                                                                                                                   | Released with this phase.                     |
| O5  | Phase 9 builds on the Development department.                                                                                                                                                                                                                                                                                                               | The owner starts it in a new branch.          |

## 8. Verification

| Check                                                                     | Result                                         |
| ------------------------------------------------------------------------- | ---------------------------------------------- |
| `pnpm check` (versions, format, lint, typecheck, tests)                   | Pass — 144 frontend tests                      |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | Pass                                           |
| `cargo test --workspace`                                                  | Pass — 570 tests                               |
| `pnpm e2e` against the release build (Linux, Xvfb)                        | Pass — 43 of 43, including the 4 Phase 8 tests |
| Generated TypeScript bindings                                             | Up to date (`pnpm bindings` leaves no diff)    |
| GitHub CI on the PR                                                       | Linked from the PR                             |

## 9. Phase boundary

Phase 8 completes the MVP (plan §4) and is released as v1.0.0. The owner's Windows check with
the real CLIs (§7, O2) is recommended with this release. Phase 9 has not been started.
