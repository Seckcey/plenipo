# ADR-157: On a Mac and Linux, a keeper ends a worker's programs if Plenipo stops suddenly

- **Status:** Accepted (by the owner, 2026-10-02: "Go ahead and merge", for pull request #138,
  which said that merging it accepts this record). It changes how Wave 1 does one item of
  [the Phase 23 checklist](../phases/phase-23-checklist.md), not what it does.
- **Date:** 2026-10-02
- **Phase:** 23
- **Part of:** [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)
- **Builds on:** [ADR-005 (the runtime supervisor)](ADR-005-runtime-supervisor.md), which noted
  that on Unix a crash leaves a worker's programs running
- **Amended by:** [ADR-158 (a program that leaves its group still ends with its work)](ADR-158-programs-that-leave-their-group.md):
  the limit under "Consequences" (a program that leaves its group) is closed

> **On screen:** nothing. A Mac's Activity Monitor shows one more Plenipo while Plenipo runs.

## In short

When Plenipo stops a worker, every program that worker started stops too. On Windows that is true
even if Plenipo crashes. On a Mac or a Linux PC it is not: a crash leaves them running. Plenipo
will start one small helper, the **keeper**, when it starts. If Plenipo ever goes away without
stopping its workers, the keeper stops their programs, then goes away too.

## Context

- **Windows** puts each worker's programs in a job object that Windows closes when Plenipo ends,
  however it ends (`crates/runtime/src/supervisor.rs`).
- **A Mac and Linux** put them in a process group. A normal stop, a timeout, and quitting end the
  whole group. A crash, or Plenipo being killed, ends nothing: the test that proves it on Windows
  (`children_do_not_outlive_a_crashed_owner`) runs only there.
- The checklist suggested Linux's `PR_SET_PDEATHSIG` and "a small watcher" for the Mac. Looking
  closer, `PR_SET_PDEATHSIG` fires when the **thread** that started the program ends, not
  Plenipo. Plenipo's worker threads come and go, so it would stop programs in the middle of their
  work. It also needs code the workspace forbids (`unsafe`), and it exists only on Linux.
- Plenipo's own program already has helper modes chosen by a switch at start (the tool relay, the
  Ollama bridge, the paid helper), so a helper mode needs no second program to ship or sign.

## Decision

1. **One keeper per Plenipo, on a Mac and Linux,** shared by every open organization's supervisor.
   When Plenipo starts, it starts its own program again with `--plenipo-keeper`. The keeper opens
   no window and loads nothing else. It runs in a process group of its own, so Ctrl+C in a
   terminal, or a signal sent to Plenipo's group, does not reach it.
2. **Each supervisor tells the keeper, over a private pipe, each program group it starts and each
   one that ends.** The keeper keeps only that list.
3. **When the pipe closes** (Plenipo quit, crashed, or was killed), the keeper asks every group
   still on its list to stop (`SIGTERM`), waits two seconds, ends what is left for certain
   (`SIGKILL`), and exits. A normal quit has already stopped the work, so the list is empty then.
4. **The keeper can reach only the owner's own programs.** The system does not let it signal
   another user's programs.
5. **Windows does not change:** its job object already does this.
6. **The test that proves it** (`children_do_not_outlive_a_crashed_owner`) runs on Windows, the
   Mac, and Linux.

## Consequences

- A worker's programs stop within a few seconds of a crash on every system.
- One more small process while Plenipo runs on a Mac or Linux.
- A program that leaves its group on purpose (a program that turns itself into a background
  service) is not caught on a Mac or Linux. Windows' job object catches it. This is the same limit
  as a normal stop on a Mac or Linux today.
- If the keeper itself is killed first, a later crash leaves programs running, as today. Plenipo
  starts a new keeper if its pipe breaks while Plenipo is still running.

## Alternatives considered

- **`PR_SET_PDEATHSIG` on Linux.** Stops programs when a worker thread retires; Linux only; needs
  `unsafe`.
- **Linux control groups through `systemd-run`.** Needs a systemd user session, which not every
  Linux PC has, and does nothing for the Mac.
- **Clean up at the next start.** Programs would keep running, as the owner, until Plenipo starts
  again, which may be days.
- **A watcher thread inside Plenipo.** It dies with Plenipo, which is exactly when it is needed.
