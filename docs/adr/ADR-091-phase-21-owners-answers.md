# ADR-091: Phase 21 — what the check found, and the owner's answers

- **Status:** Accepted (by the owner, 2026-09-30)
- **Date:** 2026-09-30
- **Phase:** 21
- **Carries out:** [ADR-090 (building Phase 21 alongside Phase 16's second wave)](ADR-090-phase-21-alongside-phase-16-wave-2.md)
  §1 and §4: the check before building, and the Free edition's limit on organizations
- **Amends:** [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md) (a limit on
  organizations); [ADR-033 (pages, notices, and Settings)](ADR-033-pages-notices-settings.md) §8
  (a terminal belongs to the window that shows it);
  [ADR-055 (Watch)](ADR-055-watch-a-worker-write-code.md) §14 (who hears Watch)
- **Number:** Phase 21 uses ADR-090 to ADR-099 (ADR-090). This is the first of its own records;
  [ADR-092](ADR-092-panels-and-windows.md) (panels and windows),
  [ADR-093](ADR-093-your-files-and-the-editor.md) (your files and the editor), and
  [ADR-094](ADR-094-more-than-one-organization.md) (more than one organization) follow.

> **On screen** (ADR-010, plain words and rank names): **panel**, **dock**, **pop out**, **Reset
> layout**, **Files** (the file view), **Open in Plenipo**, **Open in another program**, **Show in
> folder**, **Wait**, **Stop the worker**, **organization**, **Open in a new window**, **Archive
> organization**, **Delete for good**. Words to avoid are in `docs/design/vocabulary.md`.

## In short

Before building Phase 21, Plenipo's builder read the plan, the records it points at, and the code,
and listed where they disagree. The owner then answered eight questions. **Accepting this record
means:** Phase 21 ships as **one pull request**; the **Free edition keeps one organization** and Pro
has as many as you want (enforced once Phase 11A exists); **two organizations can work at the same
time**, each in its own window, never mixing; a new organization starts **from a template, as a
copy of another organization's setup, or from scratch**; an organization can be **archived and then
deleted for good**; every file offers **Open in Plenipo** first, and Plenipo **never starts a
program or script**; and files dropped on an objective can come **from anywhere** and are copied.

## Context

What the check found (the code read at `842dbbb`, v1.14.2 on `main`, 2026-09-30):

**Windows and panels**

1. **Only the main window hears anything.** Every live update goes to the window named `main`
   (`emit_to("main", …)` in `ledger_host.rs`, `agent_host.rs`, `runtime_host.rs`), and ADR-055 §14
   says Watch is "the main window's alone". A popped-out panel, or a second organization's window,
   would hear nothing.
2. **ADR-033 never states "each window has its own permission file".** The plan cites it as
   "ADR-033's rule". The code already works that way (`capabilities/default.json` for the main
   window, `capabilities/indicator.json` for the sign since Phase 10), and ADR-033 §3 keeps new
   commands to the main window; the rule itself was never written down.
3. **A terminal belongs to the main page.** Reloading the main window closes every terminal
   (ADR-033 §8). The plan asks that "a popped-out panel is the same panel, not a copy".
4. **Only two panels resize:** the terminal panel (bottom or right) and the details panel on the
   Organization page. The shared resize handle works only from a top or left edge. Nothing docks on
   the left, pops out, or drags. There is no file view, no editor, and no code highlighting library.

**Files and one writer at a time**

5. **Who is writing a working copy is known only in memory,** inside the broker
   (`crates/capabilities/src/broker.rs`, `State.writers`), and nothing reports it. A project that
   works in its own folder (no working copy) has no writer tracking at all. Watch's changes do not
   say which working copy they belong to.
6. **Nothing reads a project file for the owner.** ADR-039 §2.3 allowed it ("the owner opens and
   edits project files"); nothing is built. Guard's path checker (`crates/guard/src/paths.rs`) is
   the one to reuse; it has no tests for Windows junctions or letter case.
7. **An objective is text only.** "Drag files onto an objective" has nowhere to go.

**More than one organization**

8. **Plenipo opens one Ledger and builds one set of everything around it:** the Supervisor, the AI
   tools' sessions, Liaison, the Router, Guard and the broker, notices, and the Workforce. About 200
   commands use that one set. Nothing has an organization ID.
9. **Connection sign-ins would overwrite each other.** The Vault names them by the service
   (`connection-microsoft365-token`), so two organizations connected to Microsoft 365 would share
   one entry. The plan says the Vault keeps them "under that organization's name"; nothing does yet.
10. **Some of the PC's things are kept in the organization's Ledger:** the AI tools page's facts
    (updates, versions, models), start and close, notice choices, the terminal's shell, the last
    version run (which drives the backup before an upgrade), your Workforce (ADR-045 §11), and your
    tile (ADR-056 §5). ADR-045 and ADR-056 already promised that Phase 21 moves the last two where
    every organization shares them; the plan does not list it.
11. **Some things exist once per PC:** the screen, mouse, and keyboard; the folder of tickets that
    AI tools use to reach Plenipo's tools (a second broker empties it when it starts); Plenipo's
    browser profile.
12. **Deleting a whole organization** was promised for Phase 21 by ADR-045 §9 ("with the same
    offer"); the plan's list does not have it.
13. **Restoring a backup restarts all of Plenipo** (Phase 13). With two organizations open, it
    would close both.
14. **The Free edition has no limit on organizations** anywhere (`docs/editions.md`), and Phase 11A
    is not built.

## Decision

The owner's answers (2026-09-30), each as asked:

1. **One pull request.** "do 1 big PR". Phase 21 ships whole, as one release: **1.15.0** if Phase
   16's Wave 2 has not merged first, otherwise the next number (ADR-090 §6). The builder tells the
   owner the number before merging. Inside the pull request, the safe parts are still built first
   and more than one organization last (ADR-090 §3).
2. **The Free edition keeps one organization; Pro has as many as you want.** "as recommended".
   - `docs/editions.md` gains the row. Nothing is enforced until Phase 11A's editions exist
     (ADR-090 §4).
   - When Pro ends, extra organizations stay and still open, and their work finishes; only
     creating another one stops, as with departments and projects (`docs/editions.md`, "What
     happens if you stop paying").
   - Without the limit, a Free copy could make one organization per project and step around the
     one-project limit.
3. **Two organizations can work at the same time,** each in its own window, never mixing. "as
   recommended".
   - Work, approvals, permissions, and secrets never cross from one organization to another.
   - **The screen, mouse, and keyboard belong to the PC:** only one worker on the whole PC may use
     them at a time, whichever organization it belongs to (ADR-020).
   - **Stop all stops every worker in every organization,** and each organization's Ledger records
     it.
4. **What each organization keeps, and what the PC shares.** "as recommended".
   - **Each organization keeps its own:** the org chart, departments, projects, work, approvals,
     permissions, switches, AI model choices, servers and their sign-ins, connections and their
     sign-ins, lessons, the Activity trail, backups, working copies, screenshots, and Plenipo's
     browser's website sign-ins.
   - **The PC shares:** the AI tools' sign-ins (they stay with each AI tool, as now), the AI tools
     page, **your Workforce** (carries out ADR-045 §11), **your tile** (carries out ADR-056 §5),
     start and close, notice choices, the terminal's shell, and Plenipo's updates.
5. **A new organization asks how to start.** The owner's words: "When creating a new Org, ask if
   the user wants to use a template (going to add these later), copy from one of their other orgs,
   or start from scratch."
   - **Use a template:** shown as a choice, with "Templates are coming later" until the first
     template is added. Plenipo's code keeps a place for them.
   - **Copy from one of your organizations:** copies its **setup**: departments and positions (the
     org chart, with no one hired and no work), roles and specialties, permission sets, switches,
     AI model choices, learning choices, and titles. It never copies work, the Activity trail,
     approvals, lessons, experience, projects (their folders belong to that organization), servers,
     connections, or any secret.
   - **Start from scratch:** Plenipo's starting settings, as a new installation has.
6. **An organization can be archived, then deleted for good.** "as recommended".
   - **Archive organization** hides it from the list; it can be brought back.
   - From the archive, **Delete for good** asks first and offers to save its experienced workers
     to your Workforce, as ADR-045 §8 does for a department or project.
   - Nothing with unfinished work can be archived or deleted, and the last organization cannot be.
7. **The editor keeps you safe and keeps you in Plenipo.** "as recommended but if we have codemirror
   included, always offer to open in plenipo. We want users to stay in plenipo for as much as
   possible."
   - **Open in Plenipo is always offered first,** for every file, and is what a double-click
     does. Text and code open in the editor (CodeMirror), pictures are shown, and any other file
     shows what it is, with the choices below.
   - **Programs and scripts** (`.exe`, `.bat`, `.cmd`, `.ps1`, and the like) open in Plenipo as
     text and are **never started** by Plenipo; **Show in folder** is offered, never **Open in
     another program**.
   - **Blocked files** (like `.env` and keys) are yours, so you can open them. While a worker is
     using the screen, mouse, and keyboard, the editor hides them and takes no typing, as the
     terminal does (ADR-033 §8).
   - **If you are editing and a worker starts writing in the same working copy,** the worker goes
     ahead: the file turns read-only, your unsaved changes stay in the editor, and you save once
     the worker is done.
8. **Files dropped on an objective can come from anywhere.** "Accept files from anywhere".
   - From Plenipo's file view, or from File Explorer, for any file.
   - Plenipo copies each file when the objective is given, and the objective's workers find the
     copies in its folder; the original is never changed. The objective lists the files by name.
   - A file from File Explorer reaches Plenipo only through Windows' own drop, never as a path
     typed or sent by a page, so a web page cannot attach a file from the PC.

Settled by the check itself (no question needed):

9. **Each window type has its own permission file** listing only the commands its panels need
   (the plan's rule, now written here, next to ADR-033 §3). IPC tests prove it per window type.
10. **Live updates go to the organization's own windows:** its main window and its popped-out
    panels, never another organization's windows, the sign, or a web page. This widens ADR-055 §14
    from "the main window" to "the organization's own windows".
11. **A terminal belongs to the window that shows it.** Popping the terminal panel out moves its
    terminals to the new window without closing them; reloading a window closes only the terminals
    it shows (amends ADR-033 §8).
12. **The details:** panels and windows in ADR-092, your files and the editor in ADR-093, and more
    than one organization in ADR-094 (including the Vault's names, the PC's shared record, and
    restoring one organization's backup while another is open).

## Consequences

- One review and one release for the whole workspace; the pull request is large, so it is built
  and tested part by part inside it.
- The Free edition's limit is written now and enforced later; until Phase 11A, every copy can make
  as many organizations as it wants.
- Two organizations at work at once means Plenipo keeps a full set of workers, approvals, and Guard
  for each open organization, and one set of the PC's things (the screen, the AI tools, updates).
- Your Workforce and tile move out of the first organization's Ledger into a record every
  organization shares (ADR-094).
- Every file can be looked at inside Plenipo, and nothing on a file's say-so is ever started.

## Alternatives considered

- **Two or three parts, each its own pull request** (as Phase 20 was, ADR-067). Not chosen: the
  owner's answer.
- **Free with two organizations, or with no limit.** Not chosen: the owner accepted one, and no
  limit would step around the one-project limit.
- **One organization at work at a time.** Not chosen: the plan's acceptance opens a client's
  organization in its own window beside the first.
- **A new organization always empty.** Not chosen: the owner wants templates, copies, and scratch.
- **Programs and scripts opened in another program.** Rejected: opening a program starts it.
- **Only files already in the project's folder may be attached.** Not chosen: the owner's answer.
