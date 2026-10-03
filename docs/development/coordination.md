# How the work is coordinated

Several AI sessions can work on Plenipo at the same time. This page is how they avoid stepping on
each other. One session, the **Development Coordinator**, runs it. The sessions that build are
**workers**. Plenipo is made by 8 West Ventures, LLC, and the owner decides what gets built.

## Who does what

| Role            | Does                                                                                                                                                               | Never does                                                                  |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- |
| **Coordinator** | Decides what each worker builds and in what order. Reviews pull requests, merges them, and deploys. Keeps the shared documents. Hands out version and ADR numbers. | Writes feature code (it sends the work to a worker).                        |
| **Worker**      | Writes code and tests on its own branch, commits, pushes, opens a draft pull request, reports, and waits.                                                          | Merges, deploys, bumps a version, or edits the Coordinator's files (below). |
| **Owner**       | Chooses what Plenipo does. Approves the steps that touch live money, secrets, servers, the attorney, or the final security review.                                 |                                                                             |

The Coordinator chooses each worker's AI model and effort level for the task and sets them. The
owner has approved that arrangement. A worker must not change its own.

## Where a worker works

- **Never work inside the shared checkout** (`C:\it\plenipo` on the owner's PC). Anyone may switch
  its branch at any time, and a long test run there has broken halfway before.
- Make your own git worktree from `origin/main`, outside any synced folder, and install there:
  `git worktree add C:/it/_wt/<name> -b claude/<topic> origin/main`, then
  `pnpm install --frozen-lockfile`.
- **One Cargo target folder per worktree, never shared.** Set `CARGO_TARGET_DIR` to a folder of its
  own, inside the repository's ignored `target\` folder, and type it with backslashes from the very
  first build. Two worktrees on one target folder made Cargo reuse a crate from the other one.
- Branch names are `claude/<topic>`. Touch only branches you made.
- Cloud sessions already have their own copy of the repository, so they only need to stay on their
  own branch.

## Who edits which file

**A worker may edit:** its own code and tests; its own phase checklist and acceptance report in
`docs/phases/`; new ADR files `docs/adr/ADR-NNN-*.md` that use **only numbers the Coordinator gave
it**; and pages that describe only its own feature.

**Only the Coordinator edits these.** A worker sends the text it would have written, and the
Coordinator adds it when the work merges:

- `ROLLOUT_PLAN.md`
- `docs/roadmap.md`
- `docs/adr/README.md` (the ADR index)
- `docs/releases/*` (release notes)
- `docs/development/versioning.md`
- `README.md` and `CLAUDE.md`
- **Every version number**: the root, desktop, remote, types, UI and end-to-end `package.json`
  files, `Cargo.toml`, version lines in lockfiles, and `tauri.conf.json`.

Why: when two branches each bump the version and add release notes, both claim the same number and
the second one breaks. (On 2026-10-03, two pull requests both claimed v1.23.0.) With one owner for
these files, numbers go out one at a time, in the order the work merges.

**Lockfiles and generated files.** Never hand-merge `Cargo.lock` or `pnpm-lock.yaml`. If main
changed them, take main's copy and run the install or build again, so only your own changes are
added. After every merge of main, run `pnpm bindings` and commit the result. CI fails on any
difference in `packages/types/src/generated`.

**Stay in your lane.** Each worker gives a _touch list_ (the folders, crates, and files it expects
to change) when it checks in. Anything outside it needs a heads-up first. Contract files, such as
`contracts/community/v1`, change only in a reviewed commit the Coordinator approves.

## Numbers

- **Version numbers** are handed out at merge time. The release that merges first takes the next
  number and the next one takes the one after. A worker's branch carries no version change.
- **ADR numbers** are handed out on request, one at a time, in order. A worker never picks its own.
  The Coordinator keeps the list of numbers in use, including numbers held by branches that have
  not merged yet.

## Reports

A worker reports to the Coordinator by message. A cloud session cannot send a message, so it
reports in a **comment on its draft pull request** instead. The first line of every report names
its kind.

| Kind                   | When                                                                                                                                                           |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CHECK-IN`             | First, before any work: the task, branch, base commit, touch list, ADR numbers needed, work already in flight, open questions. The worker then waits for `GO`. |
| `STATUS`               | After each milestone, and at least every 45 minutes of work: branch and commit, what is done, what is next.                                                    |
| `BLOCKED` / `QUESTION` | Right away. What was tried and what is needed. The worker keeps going on anything not blocked.                                                                 |
| `HEADS-UP`             | Before touching a file outside the touch list, a shared file, a new dependency, a migration, a desktop command or a contract, and when two branches overlap.   |
| `READY FOR REVIEW`     | The work is done and the checks pass (below).                                                                                                                  |
| `FIXES PUSHED`         | After review comments: each one fixed, or not fixed and why; the new commit; checks run again.                                                                 |
| `DONE`                 | After the merge: the worker confirms it is idle.                                                                                                               |

**`READY FOR REVIEW` holds:**

1. The branch, the draft pull request, the head commit, and the main commit last merged in.
2. What changed in plain words, and what a person sees or does differently.
3. The files touched, and any new command, dependency, migration, contract change, or ADR.
4. Every check from [setup](setup.md), its result, and when it ran, marked **ran** or **not run**.
   Never claim a check that did not run. Mark other claims **seen**, **reported**, or **not
   checked**.
5. Text for the Coordinator's files: a roadmap row, a rollout-plan note, release-note bullets, and
   ADR index rows.
6. Known gaps, follow-ups, and anything the owner must decide, in plain words.
7. Anything that could clash with another open branch.

After sending, the worker stops and waits. It does not start new work, and it does not take another
session's task.

## What the Coordinator sends back

`GO` (the task, branch, touch list, ADR numbers, and what "done" means), `HOLD` (stop at a safe
point, commit, and push at once), `CHANGE` (the scope moved), `REVIEW` (things to fix), `STATUS?`
(answer on your next turn), and `MERGED`.

## How the Coordinator reviews and merges

1. Read the draft pull request and the diff against the touch list. Anything outside it is a
   question for the worker.
2. Check that the reported checks match CI, and that the proper checks ran. Docs-only changes run
   only the quick **Docs** check.
3. Merge main into the branch if it moved and look for clashes with other open pull requests.
4. Merge in an order that keeps clashes small: small and docs-only first, the largest last.
5. Add the version, release notes, roadmap row, rollout-plan note, and ADR index rows at merge
   time, in one small change.
6. Publishing a release and any step that touches live money, secrets, servers, the attorney, or the
   final security review needs the owner's approval for that exact step.

## Standing rules every session follows

These come from the owner and apply to every worker:

- No AI model names in commits, branch names, or pull request titles and bodies. The commit trailer
  is `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Never ask for keys, passwords, or tokens in chat, never commit them, and never have the owner
  type one into a terminal. Keys are typed only into Plenipo's own screen.
- Anything that touches files, programs, the network, the browser, or the screen goes through
  Guard and the capability broker. Nothing loads code at run time
  ([ADR-014 (adding AI tools)](../adr/ADR-014-adding-ai-tools.md)).
- New desktop commands run from the main window only, with tests that refuse the sign window and
  web pages. Logs and diagnostics never hold secrets or terminal input.
- Plain words on screen ([vocabulary](../design/vocabulary.md)). Credit 8 West Ventures, LLC.
- Dates are in Pacific time.
- Never skip a check or a hook to get green, and never mark a check as run when it did not.
- No deploys, production servers, live payment or cloud-account changes, releases, emails, or posts
  from a worker. If the task needs one, the worker says so in its report and the Coordinator does
  it, with the owner's approval for that exact step.
