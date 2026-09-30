# Phase 21 — Acceptance Report

|              |                                                                                                                                                                                                                                                                                                             |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 21 — Workspace: Panels, Windows, Files, and More Than One Organization                                                                                                                                                                                                                                      |
| **Branch**   | `claude/eloquent-darwin-mo8yak`                                                                                                                                                                                                                                                                             |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff), and the end-to-end tests for the workspace and for two organizations against the release build (13 of 13 passed; section 3). GitHub CI on the pull request, Windows included. |
| **Date**     | 2026-09-30 (Pacific time)                                                                                                                                                                                                                                                                                   |
| **Result**   | Every deliverable built, as one release (the owner's answer): **v1.16.0**. Every test in the plan's list passes. A review of four areas, each finding checked by a second reviewer, is done (section 5). The walk-through on a real Windows PC is the owner's (section 6). Plenipo by 8 West Ventures, LLC. |

Screenshots (from the end-to-end runs in the real app):

- **Panels:** [Files on the left, the terminal on the right](evidence/phase-21/workspace-docked.png) ·
  [the terminal popped out, in its own window](evidence/phase-21/workspace-terminal-popped-out.png) ·
  [Plenipo while it is popped out](evidence/phase-21/workspace-main-while-popped-out.png) ·
  [the same layout after a restart](evidence/phase-21/workspace-after-restart.png)
- **Files and the editor:** [README.md edited, not saved yet](evidence/phase-21/workspace-editor-unsaved.png) ·
  [the save in the Activity trail](evidence/phase-21/workspace-saved-in-activity.png)
- **One writer at a time:** [a worker's change landing in the editor](evidence/phase-21/workspace-watch-in-editor.png) ·
  [Stop the worker](evidence/phase-21/workspace-stop-the-worker.png) ·
  [writable again](evidence/phase-21/workspace-writable-again.png)
- **Organizations:** [New organization](evidence/phase-21/organizations-new.png) ·
  [the client's organization in its own window](evidence/phase-21/organizations-client-window.png) ·
  [Settings → Organization, both listed](evidence/phase-21/organizations-settings.png)

## 1. Deliverables → result

| #   | Deliverable (ROLLOUT_PLAN.md)                                                                                 | Result   | Evidence                                                                                                                                                          |
| --- | ------------------------------------------------------------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Panels: resize, dock (left, right, bottom), move by dragging the tab, pop out, drag out; layouts saved; Reset | **Done** | ADR-092 (panels and windows); `workspace.e2e.mjs` tests 2 to 6; `Workspace.test.tsx`; `layout.test.ts`                                                            |
| 2   | A file view: each project's folder and working copies as a tree                                               | **Done** | ADR-093 (your files and the editor) §1–4; `Files.test.tsx`; `workspace.e2e.mjs` test 7                                                                            |
| 3   | Open and edit files in a built-in editor; save; open in another program; drag files onto an objective         | **Done** | ADR-093 §5–8, §18–22; `owner_files` and `attachments` tests; `files_on_an_objective_are_named_or_copied_and_reach_its_working_copy_once`; `workspace.e2e.mjs` 7–8 |
| 4   | One writer at a time: read-only, names the worker, Wait or Stop the worker                                    | **Done** | ADR-093 §10–13; `a_working_copy_a_worker_is_writing_is_read_only_for_the_owner_until_it_stops`; `workspace.e2e.mjs` test 9                                        |
| 5   | Watch in the editor: a worker's change lands live; the tree marks files being changed                         | **Done** | ADR-093 §15–17; `a_workers_change_says_which_working_copy_and_the_file_view_marks_it`; `workspace.e2e.mjs` test 9                                                 |
| 6   | More than one organization: create, rename, switch, open in a new window; each window belongs to one          | **Done** | ADR-094 (more than one organization); the IPC tests in section 3; `organizations.e2e.mjs`                                                                         |

## 2. The owner's answers → as built

| Answer (ADR-091)                                | As built                                                                                                                                                                |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. One pull request                             | One release, v1.16.0 (Phase 16's second wave merged first as v1.15.0).                                                                                                  |
| 2. Free keeps one organization, Pro unlimited   | `docs/editions.md` has the row; not enforced until Phase 11A.                                                                                                           |
| 3. Two organizations work at the same time      | Each open organization runs its own services; Stop all, Quit, and the tray cover every one.                                                                             |
| 4. What each keeps, what the PC shares          | Own Ledger, backups, working copies, screenshots, browser profile, Vault name; shared AI tools, Workforce, tile, and choices for the PC.                                |
| 5. New organization: template, copy, or scratch | **Use a template** shows "Templates are coming later."; **Copy** brings the setup only (roles, departments and empty positions, permissions, AI model choices, titles). |
| 6. Archive, then delete for good                | Both built, with the offer to save experienced workers; the first organization stays (it keeps the PC's shared record).                                                 |
| 7. Open in Plenipo first; never start a program | Every file offers **Open in Plenipo** first; programs and scripts open as text, with **Show in folder** only.                                                           |
| 8. Files from anywhere on an objective          | From Files, or from File Explorer through Windows' own drop and a ticket; copied, originals never changed.                                                              |

## 3. Tests → evidence

| Test (the plan's list)                                                                                  | Evidence                                                                                                                                                                                                                |
| ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| resize, dock, pop out, drag out, and reset each restore correctly after a restart                       | `workspace.e2e.mjs` (dock and resize; pop out and put back; the layout after a restart, pop-out included; Reset layout); `Workspace.test.tsx` (drag out by the window's edge); `layout.test.ts`                         |
| a popped-out window can call only its own commands                                                      | `a_popped_out_panel_can_call_nothing_itself`; `the_workspace_commands_are_an_organization_windows_alone`; a second organization's pop-out in `two_organizations_in_two_windows_never_cross`; `workspace.e2e.mjs` test 4 |
| editing and saving a file records the owner's action; a file outside the known folders cannot be opened | `owner_files` tests; `the_file_commands_check_what_they_are_given`; `workspace.e2e.mjs` tests 7 and 8                                                                                                                   |
| a working copy being written by a worker opens read-only; Stop the worker makes it writable             | `a_working_copy_a_worker_is_writing_is_read_only_for_the_owner_until_it_stops`; `Files.test.tsx`; `workspace.e2e.mjs` test 9                                                                                            |
| a file open in the editor shows a worker's changes as they land, without reopening it                   | `a_workers_change_says_which_working_copy_and_the_file_view_marks_it`; `Files.test.tsx`; `workspace.e2e.mjs` test 9                                                                                                     |
| two organizations in two windows: work, approvals, and secrets never cross                              | `two_organizations_in_two_windows_never_cross` (IPC); `organizations.e2e.mjs` tests 1 and 2; `events.test.ts`                                                                                                           |
| switching organizations keeps each one's backups separate                                               | `each_organizations_backups_are_its_own`; `organizations.e2e.mjs` test 3                                                                                                                                                |

More: `a_copy_of_an_organization_is_its_setup_without_work_projects_or_secrets`,
`your_workforce_and_tile_are_shared_by_every_organization`,
`an_organization_is_archived_brought_back_and_deleted_for_good`,
`a_new_organization_starts_from_scratch_a_copy_or_not_yet_a_template`,
`deleting_my_data_forgets_every_organizations_secrets_under_its_own_name`, and
`Organizations.test.tsx`.

**Found and fixed by the end-to-end runs:** a second pop-out could not open while the first one's
window was still closing (each pop-out now has a number of its own); quitting Plenipo forgot a
popped-out panel (only the owner closing it puts it back now).

## 4. Acceptance criterion

> The owner pops the terminal out to a second screen, docks the file view on the left, edits a
> README in a project folder while a worker writes in a different working copy, and opens a second
> organization for a client in its own window with none of the first organization's work,
> approvals, or secrets in it.

Shown in the real app on Linux, piece by piece: the terminal popped out and back
(`workspace.e2e.mjs` 3), Files docked on the left (2), README.md edited and saved (7), a worker
writing in a working copy (9), and a client's organization in its own window with none of the
first's work, approvals, or secrets (`organizations.e2e.mjs` 1–2). A second screen and Windows are
the owner's walk-through (section 6).

## 5. Review

REVIEW_PLACEHOLDER

## 6. For the owner to try on a real Windows PC

1. **Pop out the terminal** (its menu → **Pop out**), move it to a second screen, type in it, then
   **Put back**. Quit and start Plenipo: it comes back where you left it.
2. **Drag the Files tab** off Plenipo's window and let go: it opens in its own window there.
3. **Open README.md** from Files, change a line, **Save**, and see it in Activity.
4. **Drop a file from File Explorer** onto a project's objective box, give the objective, and see
   the file in the working copy's `attachments` folder.
5. **Show in folder** on a file: File Explorer opens with the file selected.
6. **Organizations → New organization**, name it, **Create and open**: a second window opens. Give
   each organization a secret (Settings → Permissions); each window lists its own only.
7. **Settings → Organization → Archive organization** on the second one: its window closes.
8. Uninstall with "delete my Plenipo data" ticked: both organizations' secrets leave Windows
   Credential Manager.
