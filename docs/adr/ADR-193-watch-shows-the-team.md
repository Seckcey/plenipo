# ADR-193: Watch shows the team's work, says why it is empty, and is on every tile

- **Status:** Accepted (the owner's report, 2026-10-03: "When i click on 'watch' an agent … it
  says 'No file changes yet in this objective' … Watch is not on every agents tile"; items 1.8 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), accepted the same day).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 1 (item 1.8)
- **Amends:** [ADR-055 (Watch: seeing a worker write code as it happens)](ADR-055-watch-a-worker-write-code.md)
  §1 (where the Watch button is) and the rule that a Watch tab shows one agent's own changes only.

> **On screen** (ADR-010, plain words and rank names): an empty Watch says "Changes show here as
> the worker, or the team it hands work to, makes them. Watch shows the files they write with
> Plenipo's file tools; changes made by commands they run aren't shown yet." When Plenipo knows
> more, it says that instead, in Guard's own words, for example "Senior Developer got no tools:
> this work belongs to no project."

## In short

Watch showed only the clicked agent's **own** file changes. Managers and supervisors hand coding to
their team, so their Watch was always empty, and the canvas offered Watch on them exactly while
they were handing work out.

**Accepting this record means:**

1. A Watch tab shows the agent's own changes **and its team's**: every change made in the tasks it
   handed on, in its current objective. Each change still names the worker who made it.
2. An empty Watch says **why**, when Plenipo knows.
3. The Watch button is on **every active tile** on the canvas (quieter when the agent isn't
   working) and on the agent's own page.

## Decision

1. **Whose changes.** Plenipo finds the agent's current objective (its latest task's root), then
   the agent's own tasks in it, then every task under those: the work it handed on. Watch shows
   the changes made in those tasks, from memory while Plenipo runs and from the Ledger's record
   after a restart. A worker that hands nothing on sees only its own work, as before.
2. **Live.** The page hears every change as it lands. It keeps the ones from its tasks. A change in
   the same objective from a task it doesn't know yet may be a new hand-off, so it reads the list
   again.
3. **Why it is empty.** "It hasn't been given any work yet", or the reason Guard recorded when a
   worker on the team got no tools to change files with. Otherwise the general line above,
   which also says plainly that changes made by commands aren't shown yet (item 3.2 adds them).
4. **Where the button is.** Every active position's tile on the canvas, and the agent's own page.
   The Task page, Home's "Who's working" rows, and the list view come with the live conversation
   (item 3.1).

## Consequences

- Watch on a supervisor shows its developers' work as it is written.
- A worker that shares an objective with others doesn't see their changes: only its own and its
  team's.
- More Watch buttons on the canvas, so the ones on idle tiles are drawn quieter.

## Alternatives considered

- **Every change in the objective, for every agent in it.** Rejected: a documentation writer's Watch
  would fill with the developer's files.
- **Show Watch only on tiles that are working (as before).** Rejected by the owner's report: they
  couldn't find it on the tiles they wanted to watch.
