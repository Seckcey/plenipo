# ADR-199: Stop on every worker, and Stop all work

- **Status:** Accepted (the owner, 2026-10-03: "We need a stop button on all agents popups", and
  "A stop all emergency button on all pages that monitor tasks. Especially on the org canvas.";
  items 3.3 and 3.4 of [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), accepted the same
  day).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 3 (items 3.3 and 3.4)
- **Amends:** [ADR-020 (browser and computer use)](ADR-020-browser-and-computer-use.md) and
  [ADR-094 (more than one organization)](ADR-094-more-than-one-organization.md) §7: **Stop all**
  now stops all work, not only the browser, the screen, and servers.

> **On screen** (ADR-010, plain words and rank names): **Stop** on each working tile, in the
> details panel, on the Worker and Task pages, and in Home's "Who's working" rows, asking "Stop
> Alex's task?" first ("Keep working" or "Stop"). A red **Stop all** in the top bar of Home,
> Organization, Projects, Workers, a task's page, and a worker's page, and on the map's toolbar;
> then **Allow again**. The sign says "All work is stopped." The tray says "Stop all work".

## In short

Stop was only on the Workers page and in Watch. "Stop all" stopped the browser, the screen, and
servers, but the AI work went on, and new work kept being handed out.

**Accepting this record means:**

1. **Stop on every worker.** One Stop part, the same everywhere, that asks first. A full-time
   agent stops only its current task: its conversation stays, so it can take a new objective
   after. An on-call position stops each of its workers that has started; work still queued ends
   with the task that asked for it. The lead that asked for the work is told it was stopped
   ("cancelled") and carries on. It is the same command the phone's Stop uses.
2. **Stop all work**, in every organization, at once:
   - browser, screen, and server control stop, as before;
   - every task running or waiting now stops;
   - nothing new starts until you press **Allow again**: work already handed out waits, and new
     work you give (an objective, a conversation, Run again, from the window or a phone) is
     refused at once with "All work is stopped (you pressed Stop all). Press Allow again to start
     work.";
   - each organization's Ledger records it ("You pressed Stop all: 3 tasks stopped") and Allow
     again ("You pressed Allow again: work can start again").
3. **One switch for the PC.** The red button, the sign's Stop all, the tray's "Stop all work",
   and the phone's Stop all all do the same, and Allow again from any of them lets everything go
   again. An organization opened while work is stopped is stopped too.
4. **No question first for Stop all.** It is the emergency stop; Allow again undoes the hold.
   Stop on one worker does ask first.

## Decision

- Each organization's AI tools (the agent runtime) keep a **hold**: while it is on, a turn about
  to start waits (it can still be stopped), and every AI tool shows "All work is stopped. A new
  task starts when you press Allow again." It uses the same wait as an AI tool's update
  (ADR-059 §4).
- The hold is kept in memory, like the browser and screen stop. After Plenipo restarts, nothing
  that was running resumes anyway (Phase 13).

## Consequences

- One press stops everything, everywhere, and nothing sneaks back in.
- Tasks stopped by Stop all are stopped for good; Run again starts one again after Allow again.

## Alternatives considered

- **Queue new objectives while stopped.** Rejected: a request that hangs with no answer looks
  broken. Saying "Press Allow again" at once is clearer.
- **Ask before Stop all.** Rejected: it is the emergency button.
