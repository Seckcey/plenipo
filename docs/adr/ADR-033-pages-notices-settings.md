# ADR-033: Home, a page for each thing, pop-up notices, and Settings in one place

- **Status:** Proposed
- **Date:** 2026-09-27
- **Phase:** 12
- **Number:** ADR-032 is taken by pull request #36, so this is ADR-033.

## Context

Phase 12 of `ROLLOUT_PLAN.md` turns the engine into a product the owner can run without reading
raw output. Its deliverables name views ("Home / Company", "Department View", "Project View",
"Agent View", "Task View"), Settings contents, notifications, and tests, but not how they fit
together. The owner approved the design in `docs/phases/phase-12-checklist.md` (Design,
2026-09-27): a Home page, a page for each department, project, worker, and task, Windows pop-up
notices through `tauri-plugin-notification`, and Settings in one tidy place, with Settings →
Servers unchanged. The terminal panel has its own record (ADR-031, the terminal panel).

A few choices go beyond the plan's words or shape the app for later phases. This record says
what they are, so they can be accepted or changed in one place.

## Decision

### 1. Home, and where you are

- **Home** is the first section on the left strip and the page Plenipo opens on. Pip greets the
  owner, and his pose follows what matters most (something stuck, something waiting, work
  going, work finished, or all quiet).
- The plan's views are **pages of one thing**, opened from other pages, never from the strip:
  department, project, worker, task. The strip marks the section a page belongs to
  (Organization, Projects, Workers, Activity), the top bar names the page ("Department ·
  Operations"), and **Back** returns to the page before.
- **Where you are is kept on this computer** (the page and what it is about), so Plenipo comes
  back to the same page after a restart. Back remembers this window's trail only; after a
  restart, Back goes to the page's section.
- The top bar's **Showing** picker (ADR-030, the design system) now opens the department's or the
  project's page; on one of those pages, choosing "everything" opens Home. Elsewhere it shows
  "everything", because every other page shows all of the company. Phase 12A left it naming
  "the place a pick opened".

### 2. The plan's "Agent View" is the page of a position

Workers on call come and go, one per task; a full-time position keeps one agent. So the page is
about the **position** ("Worker page"): its role, its AI tool and model and why the Router chose
them, what it is working on, the permissions it is using now, its conversation, and its
history. A worker on call shows on its position's page while it works.

### 3. New queries behind a few commands

The Ledger answers the pages with new queries on the existing tables (no migration): a scope's
events a page at a time, a task tree's events, what is stuck (one problem per piece of work
still in trouble), and a project's or task's pull requests, screenshots, and decisions. The
commands are the main window's only (`get_home`, `get_scope_events`, `get_task_events`,
`get_project_record`, `get_task_record`, `get_local_paths`), with IPC tests that the sign window
and web pages are refused.

### 4. Pop-up notices are decided in Rust, from the Ledger

- A Ledger listener hands each committed event that may matter to a background thread, which
  asks the Ledger what it means for the owner: a worker waiting for the owner's OK, a check to
  solve, a problem, finished work, or a lesson. It keeps the kinds the owner wants, gathers those
  that arrive together into one notice, and does not repeat the same notice within a minute.
- Notices are shown through `tauri-plugin-notification`. **Its own commands are granted to no
  window**, so the page can never send a notice or choose its words; only Plenipo decides.
  Workers have no way to send one either (it is not a tool).
- The owner's choices are kept in the Ledger's settings (`preferences`, next to the terminal's
  shell). "Only while Plenipo's window is not in front" is on to start with.
- The plugin is kept at 2.4 so that Tauri itself stays at 2.11 in this release.
- A real Windows notice needs the installed app's identity, so the decisions are tested by unit
  and IPC tests, and the owner checks real notices on Windows.

### 5. Settings in one place

- A list of sections on the left, one at a time; the last one comes back. Another page can open
  a section (Home: "Fix it in Settings → Servers").
- **Local paths are shown, not changed.** Moving Plenipo's data is installer work (Phase 13).
- Settings → Servers is the same component and behavior as in Phase 11, in its own section.

### 6. The older pages use the library

Buttons, status pills and badges, tabs, and cards on the older pages come from `@plenipo/ui`
(ADR-030 §8). What each page does, and its words, stay the same. Links inside text and the pages'
own layout stay page-level.

### 7. The owner's brand

The owner's approved Plenipo + Pip kit is kept byte for byte in `docs/brand/pip-brand-kit` (the
same files, identical to the byte, reached `main` in `assets/` through pull request #66). The
logo (the three-rail P, "lenipo", and Pip on the n) replaces Phase 12A's placeholder mark, and
the app icon is the P on the kit's navy. Pip appears on Home, beside "nothing here yet", in the
terminal panel, in Settings → About, and in the Gallery.

### 8. The terminal, in a few places ADR-031 leaves open

ADR-031 (the terminal panel) is accepted; these follow from it and were settled while building:

- **A terminal belongs to the page that shows it.** When the main window's page loads again (a
  reload), the terminals it showed are closed, instead of running unseen.
- **Remote computers (SSH) off closes the owner's server terminals**, as it disconnects every
  worker (ADR-031 says the server terminal works only while the switch is on).
- **Closing the window while a terminal is open hides Plenipo to the tray**, as it does while
  work runs; quitting closes the terminals and records it.
- **While a worker controls the screen, mouse, and keyboard, the terminal takes no typing**
  (what reaches it could be the worker's) until the owner takes over; a new line or Ctrl+J typed
  by a worker asks the owner, as Enter does.
- **F6 takes the keyboard from the terminal back to its tabs** (Tab belongs to the shell), and
  Settings → Terminal has a switch for screen reader support.

## Consequences

- The owner can run the company from Home and the pages without reading raw output or session
  IDs; Diagnostics keeps the technical details.
- Pages read the Ledger live (they reload when relevant events commit), so they cost queries;
  history lists read a page at a time (at most 200 events) to stay fast on a large Ledger.
- A new page needs a place in the navigation (`components/views.ts`) and a section on the strip
  to belong to.
- Notices depend on Windows' notification settings; "Send a test notice" checks them, and says
  plainly when the system did not show one.

## Alternatives considered

- **Filtering every page by the Showing picker** — more work on every page for little gain; a
  department's or project's page shows its part of the company in one place.
- **Deciding notices in the web page** (listening to Ledger events there) — notices would stop
  whenever the window's page is not running, and the page would need permission to send notices.
- **A page per agent instance** — on-call workers last one task; their position is what the
  owner manages.
- **Editable local paths** — moving the Ledger safely needs the installer and recovery work of
  Phase 13.
