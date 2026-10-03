# ADR-158: On a Mac and Linux, a program that leaves its group still ends with its work

- **Status:** Proposed by the builder in the Wave 1 Guard safety review (2026-10-02). Merging the
  pull request that adds it accepts it.
- **Date:** 2026-10-02
- **Phase:** 23
- **Part of:** [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)
- **Amends:** [ADR-157 (a keeper ends programs after a crash)](ADR-157-a-keeper-ends-programs-after-a-crash.md):
  removes the limit it listed under "Consequences"

> **On screen:** nothing. Stop, a time limit, and quitting work as before; on a Mac and Linux they
> now also end the programs that slipped out of the worker's group.

## In short

When Plenipo stops a worker, every program that worker started must stop too. On Windows that is
always true. On a Mac and Linux, a program can step out of the worker's group: a build tool's
background server does it, and so do `tmux`, `ssh-agent`, and `setsid`. Today such a program
keeps running after Stop and even after Plenipo quits or crashes. From now on, every program a
worker starts carries a small mark that the programs it starts inherit, and Plenipo ends whatever
still carries the mark when the work is over.

**Accepting this record means** that on a Mac and Linux, Stop, a time limit, the end of a step, and
quitting end those programs too, as Windows does. ADR-150 says no Mac or Linux download ships
until "program trees hold" there; this is what makes them hold.

## Context

- **Windows** runs each worker's programs in a job object that does not let a program leave it.
  When the work ends, or Plenipo ends however it ends, Windows ends them all.
- **A Mac and Linux** run them in a process group. Plenipo ends the group when the work ends, and
  the keeper (ADR-157) ends it if Plenipo crashes. A program that starts a session of its own is
  no longer in the group. ADR-157 recorded this as a known limit.
- The Wave 1 Guard safety review found that this is common, not rare. Gradle and Bazel keep a
  server running after a build (`./gradlew build` is on the approved list), many test tools start
  one, and anything a worker starts with `setsid`, `tmux`, or `screen` leaves the group. Such a
  program keeps running as the owner, after the owner pressed Stop.
- The tool server already refuses such a program: once it leaves, it no longer descends from the
  worker's AI tool (ADR-034, ADR-156). So the gap is about stopping, not about tools.

## Decision

1. **A mark.** On a Mac and Linux, every program Plenipo starts for a worker (an AI tool, a
   command, the browser, a short check of an AI tool) gets `PLENIPO_RUN=<this Plenipo>/<this run>`
   in its environment. Programs inherit their parent's environment, so everything the program
   starts carries it too, in a group of its own or not. The first part is random for each start of
   Plenipo, so two Plenipos, or one after a restart, never end each other's programs.
2. **When a run ends, is stopped, or runs past its time limit,** Plenipo ends the group as before,
   then looks for the owner's own programs still carrying that run's mark: it asks them to stop,
   and ends any still there a second later.
3. **When Plenipo goes away,** however it goes, the keeper ends the groups on its list as before,
   then every program still carrying this Plenipo's mark. Plenipo hands the keeper its part of
   the mark when it starts it.
4. **What it cannot do.** The system lets Plenipo look at, and signal, only the owner's own
   programs. A program that empties its own environment, or one the system hides (a program that
   makes itself unreadable, such as `ssh-agent` on Linux), is not found. Such a program is also
   cut off from Plenipo's tools. Programs started by the system's own service managers
   (`systemd-run`, `launchctl`, `at`) never carried the mark; Guard blocks those programs
   (Wave 1 review).
5. **Windows does not change.** Its job object already does all of this, so nothing is marked
   there.

## Consequences

- On a Mac and Linux, pressing Stop, a time limit, the end of a step, and quitting stop
  everything the work started, within about a second.
- A program a worker opens that the owner keeps using (an app that `xdg-open` started on Linux)
  carries the mark and ends with the step, as it would in Windows' job object. A Mac's `open`
  starts apps through the system, without the mark, so they stay open.
- Ending a run reads the list of the owner's programs once. That takes a few hundredths of a
  second, off the screen's way.

## Alternatives considered

- **Linux control groups (`systemd-run --scope`).** Hold every program for certain, but need a
  systemd user session, which not every Linux PC has, and do nothing for the Mac.
- **Linux's "subreaper" setting.** Keeps escaped programs as Plenipo's children while it runs, but
  they go to the system the moment Plenipo crashes, and the Mac has no such setting.
- **Block every program that can leave its group.** Not possible: many ordinary tools do it on
  their own, and a worker's own code can do it.
- **Keep the limit (ADR-157 as it is).** Stop would not mean stop on a Mac or Linux.
