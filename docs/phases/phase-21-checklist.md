# Phase 21 — Implementation Checklist

**Status: Phase 21 delivered as v1.16.0** (2026-09-30; [acceptance report](phase-21-acceptance-report.md)).
Built beside Phase 16's second wave (ADR-090), which merged first as v1.15.0, so this is the next
number. Builds on v1.15.0. Below, "[x]" is done. Plenipo is made by 8 West Ventures, LLC.

Source: `ROLLOUT_PLAN.md`, Phase 21 — Workspace: Panels, Windows, Files, and More Than One
Organization, and the records written for it:

- [ADR-090 (building Phase 21 alongside Phase 16's second wave)](../adr/ADR-090-phase-21-alongside-phase-16-wave-2.md)
- [ADR-091 (what the check found, and the owner's answers)](../adr/ADR-091-phase-21-owners-answers.md)
  — one pull request; the Free edition keeps one organization; **accepted**
- [ADR-092 (panels and windows: resize, dock, pop out, drag out, and Reset layout)](../adr/ADR-092-panels-and-windows.md)
- [ADR-093 (your files and the editor; one writer at a time; files on an objective)](../adr/ADR-093-your-files-and-the-editor.md)
- [ADR-094 (more than one organization: each its own Ledger, window, backups, and Vault names)](../adr/ADR-094-more-than-one-organization.md)

**Numbers:** ADR-091 to ADR-094 (ADR-090 set aside 090 to 099 for Phase 21). **No new Ledger
layout** (it stays at 11): a new organization is a new Ledger file, and the list of organizations
is `organizations.json` in Plenipo's data folder.

Dates are Pacific time. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): **panel**, **Pop out**, **Put back**,
**Reset layout**, **Files**, **Open in Plenipo**, **Wait**, **Stop the worker**, **organization**,
**Open in a new window**, **Archive organization**, **Delete for good**.

**Goal (plan):** "Let the owner lay out Plenipo their way: resize, dock, and pop out panels;
browse, open, and edit project files; and run more than one organization, each in its own window
if wanted."

## In short, for the owner

- **Panels.** The Terminal and the new Files panel can sit on the left, right, or bottom, be made
  bigger or smaller, and **Pop out** into their own window (from the panel's menu, or by dragging
  its tab off Plenipo's window). **Put back** brings one home. Plenipo remembers the layout, even
  after a restart; **Reset layout** (the panel's menu, or Settings → Personalization) starts over.
- **Files.** The Files panel shows each project's folder and working copies. Double-click a file
  to **Open in Plenipo**: code with colors, pictures shown. **Save** is recorded in the Activity
  trail as yours. Plenipo never starts a program or script; **Show in folder** is offered instead.
- **One writer at a time.** A working copy a worker is writing opens read-only and says who is
  writing, with **Wait** or **Stop the worker**. What the worker writes shows in the editor as it
  lands.
- **More than one organization.** The **Organizations** menu at the top makes a **New
  organization** (from scratch, as a copy of another one's setup, or from a template once there
  are some), **Switch to** another one in this window, or **Open in a new window**. Each keeps its
  own work, approvals, backups, and secrets; your Workforce, your tile, and the AI tools are
  shared.

## Deliverables

- [x] **Panels resize, dock (left, right, bottom), move by dragging their tab, and pop out** into
      their own window; dragging a tab outside Plenipo's window pops it out there (ADR-092)
- [x] **The layout is saved** (each organization's own), and comes back after a restart, pop-outs
      included; **Reset layout** in the panel menu and in Settings → Personalization
- [x] **A popped-out panel is the same panel:** its terminals keep running (xterm.js moves to the
      new window's page); a pop-out has a permission file with no commands (`popout.json`)
- [x] **Files:** each project's folder and working copies as a tree, with who is writing where,
      and the files a worker is changing now marked (ADR-093)
- [x] **Open and edit files** in the built-in editor (CodeMirror 6: text and code with colors,
      pictures shown); **Save**, with "changed on disk" caught; **Open in another program** and
      **Show in folder**; programs and scripts open as text and are never started
- [x] **Owner saves are recorded** in the Activity trail (`file.saved`), and only project folders
      and working copies Plenipo knows can be read or written (Guard's path checker)
- [x] **Drag files onto an objective:** from Files, or from File Explorer (through Windows' own
      drop and a ticket, never a path sent by a page); copied into the objective's working copy
- [x] **One writer at a time:** a working copy a worker is writing opens read-only, names the
      worker, and offers **Wait** or **Stop the worker** (ADR-016)
- [x] **Watch in the editor:** a worker's change lands live in the open file, like Phase 18's
      Watch tab; the tree marks files being changed
- [x] **More than one organization:** create (scratch, copy, template later), rename, switch,
      **Open in a new window**, archive, bring back, and delete for good (ADR-094)
- [x] **One Ledger per organization** with its own backups, working copies, screenshots, browser
      profile, and tool tickets; the first organization stays where it always was
- [x] **The Vault keeps each organization's secrets under its own name**; the uninstaller forgets
      every organization's
- [x] **AI tool sign-ins belong to the PC:** every organization's AI tools use one settings
      folder; the AI tools page is the PC's
- [x] **Your Workforce and your tile** are shared by every organization (carries out ADR-045 §11
      and ADR-056 §5)
- [x] **Each window type has its own permission file** with only what its panels need:
      organizations' windows (`default.json`, `main` and `org-*`), pop-outs (`popout.json`, none),
      the sign (`indicator.json`)
- [x] **Stop all, Quit, the tray, and work going** cover every organization; an AI tool's update
      holds every other organization's new work for that tool
- [x] **The Free edition keeps one organization** (`docs/editions.md`); not enforced until Phase 11A

## Tests (the plan's list)

- [x] resize, dock, pop out, drag out, and reset each restore correctly after a restart —
      `workspace.e2e.mjs` (real app), `Workspace.test.tsx`, `layout.test.ts`,
      `workspace_windows` unit tests
- [x] a popped-out window can call only its own commands —
      `a_popped_out_panel_can_call_nothing_itself`, `the_workspace_commands_are_an_organization_windows_alone`
      (IPC), and a pop-out of a second organization's window in
      `two_organizations_in_two_windows_never_cross`
- [x] editing and saving a file records the owner's action; a file outside the known folders cannot
      be opened — `owner_files` unit tests, `the_file_commands_check_what_they_are_given` (IPC),
      `workspace.e2e.mjs`
- [x] a working copy being written by a worker opens read-only; Stop the worker makes it writable —
      `a_working_copy_a_worker_is_writing_is_read_only_for_the_owner_until_it_stops` (Rust),
      `Files.test.tsx`, `workspace.e2e.mjs`
- [x] a file open in the editor shows a worker's changes as they land, without the owner reopening
      it — `a_workers_change_says_which_working_copy_and_the_file_view_marks_it` (Rust),
      `Files.test.tsx`, `workspace.e2e.mjs`
- [x] two organizations in two windows: work, approvals, and secrets never cross between them —
      `two_organizations_in_two_windows_never_cross` (IPC), `organizations.e2e.mjs` (real app),
      `events.test.ts` (a page hears its own window only)
- [x] switching organizations keeps each one's backups separate —
      `each_organizations_backups_are_its_own` (Rust), `organizations.e2e.mjs`
- [x] more: a copy is the setup without work or secrets
      (`a_copy_of_an_organization_is_its_setup_without_work_projects_or_secrets`); your Workforce
      and tile are shared (`your_workforce_and_tile_are_shared_by_every_organization`);
      archive, bring back, and delete for good (`an_organization_is_archived_brought_back_and_deleted_for_good`);
      the uninstaller forgets every organization's secrets
      (`deleting_my_data_forgets_every_organizations_secrets_under_its_own_name`)

## Review (four areas, each finding checked by a second reviewer)

- [x] **Files and the editor:** 10 findings; 9 confirmed and fixed with a test, 1 (a comma in a
      file's name) fixed as a rule with a test; mixed line endings recorded as a limit
- [x] **Copying an organization and your Workforce:** 9 findings, all confirmed and fixed with tests
- [x] **Panels and pop-outs:** 6 findings, all confirmed and fixed with tests
- [x] **Keeping organizations apart:** 11 findings, all confirmed and fixed with tests
- [x] Each new test fails on the code before its fix (checked)

## Pre-push checks

- [x] `pnpm check`
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [x] `cargo test --workspace --locked`
- [x] `pnpm bindings`, then no diff in `packages/types/src/generated`

## Limits (recorded in the ADRs)

- The details panel on the Organization page stays with that page (ADR-092).
- The AI tools page still says "at the bottom" for its sign-in terminal wherever the Terminal
  panel is (that page is Phase 16's; ADR-090).
- Files cannot be made, renamed, or deleted in the file view (ADR-093); unsaved editor changes are
  lost when Plenipo quits.
- The first organization cannot be archived or deleted: it holds your Workforce, your tile, and
  the PC's choices (ADR-094).
- An AI tool's update does not wait for other organizations' workers already using it, and signing
  in holds the first organization's work only (the AI tools page's own rules; ADR-094).
- Restoring any organization's backup restarts all of Plenipo (ADR-094 §10).
- A file with mixed line endings is saved with its main one everywhere; a file whose name or
  folder has a comma opens in Plenipo only (ADR-093).
- Settings Plenipo could not read are checked, shown, and reset in the first organization only
  (ADR-094).

## Still to try on a real Windows PC

See the acceptance report's list: pop-outs on WebView2, dragging a tab off the window, File
Explorer drops on an objective, **Show in folder**, a second organization's window, and the
uninstaller's secrets.
