# Phase 18 — Acceptance Report

|              |                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 18 — The Organization Canvas                                                                                                                                                                                                                                                                                                                                                                                                                        |
| **Branch**   | `claude/phase-18` ([PR #88](https://github.com/Seckcey/plenipo/pull/88))                                                                                                                                                                                                                                                                                                                                                                            |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings` (no diff), and the canvas end-to-end tests against the release build (7 of 7; section 3). GitHub CI on PR #88: Rust, Frontend, Docs, Website, E2E on Linux (the whole end-to-end suite), and Windows (filled in from the final run).                                                                                                                                       |
| **Date**     | 2026-09-28 (Pacific time)                                                                                                                                                                                                                                                                                                                                                                                                                           |
| **Result**   | Every acceptance criterion and every Phase 18 test in the plan pass; every deliverable is built. Version **1.11.0**. Decisions: ADR-053 to ADR-056, accepted by the owner as recommended, with the owner's choice that a refused change's record keeps no text; all built, with the differences each records as built. The walk-through with real AI tools on Windows is the owner's (section 8), and one question waits for the owner (section 6). |

Screenshots (from the end-to-end run in the real app, `tests/e2e/specs/canvas.e2e.mjs`):

- **The canvas:** [the first-time tour](evidence/phase-18/canvas-tour.png) ·
  [a tile placed by hand](evidence/phase-18/canvas-placed.png) ·
  [after Tidy up](evidence/phase-18/canvas-tidy-up.png) ·
  [a line's ends](evidence/phase-18/canvas-line-ends.png)
- **Move or lend:** [the Security Auditor lent to Marketing](evidence/phase-18/canvas-lent.png) ·
  [its Team tab](evidence/phase-18/canvas-lent-team-tab.png)
- **The trash can:** [archived, with Undo](evidence/phase-18/canvas-trash-undo.png) ·
  [the Archived drawer](evidence/phase-18/canvas-archived-drawer.png)
- **Filters and the legend:** [one department](evidence/phase-18/canvas-filters.png) ·
  [the legend](evidence/phase-18/canvas-legend.png)
- **The live view:** [where each worker thinks, runs, and what it touches](evidence/phase-18/canvas-live-where.png)
- **Watch:** [a change being written](evidence/phase-18/canvas-watch-writing.png) ·
  [saved, with its new lines](evidence/phase-18/canvas-watch-saved.png) ·
  [refused, without its text](evidence/phase-18/canvas-watch-refused.png)
- **Your tile:** [the panel](evidence/phase-18/canvas-owner-panel.png) ·
  [on the canvas](evidence/phase-18/canvas-owner-tile.png)

Test totals: **1,113 Rust** · **673 frontend** (304 design system + 369 app) · **89
end-to-end** tests against the real release binary. Here, the canvas group (7) passes against the release build; the whole suite runs on GitHub, where the groups
that need what this machine lacks (the browser group needs Chrome; the server and terminal
groups need `ssh-keygen`) also run.

On screen the plan's words become plain ones ([word list](../design/vocabulary.md)): a box on
the canvas is a **tile**, "layout" is **Arrange** and **Tidy up**, "pan" is **Move the view**,
"reassign" and "loan" are **Move here**, **Lend**, and **Send home**, "compute location" and
"data locality" are **Where**, **thinks in … cloud**, **runs on**, and **touching**, a "diff" is
**new** and **changed** lines, a "stream" is a change **being written — not saved yet**, and
the owner's "avatar" and "presence" are **Your picture** and **status**. Quotes from the plan
keep the plan's words.

CI has no AI tool accounts. The end-to-end tests use Plenipo's stand-in AI tool
(`plenipo-fake-agent`), which can stream a file write piece by piece as Claude Code does, and a
throwaway home folder.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                                                 | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | The owner rearranges the organization by dragging.                                                                                          | **Pass** | E2E "places a tile by hand, keeps it after a restart, and Tidy up puts it back (with Undo)" ([placed](evidence/phase-18/canvas-placed.png), [Tidy up](evidence/phase-18/canvas-tidy-up.png)); Ledger `a_moved_tile_stays_where_it_was_put_after_a_restart_and_tidy_up_forgets_it`; `OrganizationCanvas.test.tsx` "saves where a tile is dropped, keeps it after a restart, and Tidy up (with Undo) puts it back", "moves the selected tile with Alt and the arrow keys, and arranges without menus"; `canvas.test.ts` (a tile moves with its team, or alone with Alt). |
| 2   | The owner rewires two reporting lines by their ends.                                                                                        | **Pass** | E2E "rewires a line by its end" (Code Reviewer's line dragged to Campaign Supervisor, [line ends](evidence/phase-18/canvas-line-ends.png)); `OrganizationCanvas.test.tsx` "drags the selected agent's line end to a new lead, and refuses one the rules refuse" and "moves an oversight line's team end in one step" (both kinds of line); Ledger `an_oversight_line_is_moved_by_its_end_in_one_step`; IPC `places_lending_and_the_owners_tile_through_ipc`.                                                                                                           |
| 3   | The owner lends a Security Auditor to another department for one objective and sees it come back.                                           | **Pass** | E2E "lends the Security Auditor to Marketing for one objective, and it comes home by itself" ([lent](evidence/phase-18/canvas-lent.png)); broker `a_lent_auditor_works_one_objective_under_the_other_projects_limit_then_goes_home`; Workforce `a_lent_agent_works_for_one_objective_of_the_other_team_then_comes_home`; Ledger `lending_follows_the_rules`, `sent_home_now_or_when_its_task_ends`.                                                                                                                                                                    |
| 4   | The owner archives an agent with the trash can and brings it back from the drawer.                                                          | **Pass** | E2E "rewires a line by its end, and archives with the trash can (Undo, and the drawer)" ([Undo](evidence/phase-18/canvas-trash-undo.png), [drawer](evidence/phase-18/canvas-archived-drawer.png)); `OrganizationCanvas.test.tsx` "archives an agent dropped on it, with Undo; a Supervisor asks about its project first", "refuses a lead with a team, with the reason, and opens the Archived drawer".                                                                                                                                                                |
| 5   | The owner filters the canvas to one department.                                                                                             | **Pass** | E2E "filters narrow the canvas, and the legend explains every mark" ([filters](evidence/phase-18/canvas-filters.png)); `canvas.test.ts` "each filter narrows the canvas, keeping the leads above a match, faded" (all seven filters and the search).                                                                                                                                                                                                                                                                                                                   |
| 6   | From the canvas alone, the owner can say which workers run on this PC, on a server, or in an AI company's cloud, and what each is touching. | **Pass** | E2E "watch a worker write code" (the **Where** switch: "Thinks in Anthropic's cloud", [screenshot](evidence/phase-18/canvas-live-where.png)); broker `the_live_view_says_where_the_work_runs_in_each_case`, `the_live_view_shows_what_a_worker_touches_and_the_handoffs`; `canvas.test.ts` "says where each worker thinks, runs, and what it touches, in each case" (this PC, Plenipo's browser, a production server, a folder, a website).                                                                                                                            |
| 7   | While a Senior Developer on Claude Code works, the owner opens Watch and sees the code appear as it is written, then saved, file by file.   | **Pass** | E2E "watch a worker write code: being written, saved with its new lines, and refused" ([being written](evidence/phase-18/canvas-watch-writing.png), [saved](evidence/phase-18/canvas-watch-saved.png), [refused](evidence/phase-18/canvas-watch-refused.png)), with the stand-in AI tool streaming as Claude Code does; broker `a_streamed_change_shows_being_written_then_saved_and_a_refused_one_never_saved`, `watch_shows_every_file_change_as_it_lands_with_its_lines`. With the real Claude Code, on the owner's PC (section 8).                                 |

## 2. Deliverables → evidence

| Plan deliverable                    | Built as                                                                                                                                                                                                                                                                                                                                                                                                 | Tests                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **arrange freely**                  | ADR-053 §1–§6: drag a tile anywhere (its team follows; Alt moves it alone); spots are kept in the Ledger (layout 11) and not listed in the Activity trail; tiles never moved follow their lead; **Tidy up** forgets every spot, with Undo.                                                                                                                                                               | Ledger `a_moved_tile_stays_where_it_was_put_after_a_restart_and_tidy_up_forgets_it`, `only_tiles_on_the_canvas_are_placed_and_only_on_it`, `a_tile_deleted_for_good_leaves_the_canvas`; `canvas.test.ts`; `layout.test.ts`; `OrganizationCanvas.test.tsx`; E2E.                                                                                                                                                                                                                                                                                       |
| **rewire by dragging lines**        | ADR-053 §7–§8: the selected agent's line ends are handles (buttons, for the keyboard too); "reports to" uses `move_position`'s rules; an oversight line's end moves in one step (`retarget_oversight`), with the same checks and events.                                                                                                                                                                 | Ledger `an_oversight_line_is_moved_by_its_end_in_one_step`; `OrganizationCanvas.test.tsx` (both kinds, refused with the reason); E2E.                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **move or lend**                    | ADR-054: **Move here**, **Lend for one objective**, **Lend until I send it home** (on-call agents only), a lent badge and line, **Send home**, and **Lend to another team** in the Team tab without a mouse. While lent, the borrowing team decides its permission limit, working copy, AI tools, and rules; it comes home by itself. Guard reads each task's own project (the fix found while mapping). | Ledger `lending_follows_the_rules`, `sent_home_now_or_when_its_task_ends`, `a_job_that_joins_an_objective_already_ended_still_brings_it_home`, `archiving_the_team_it_helps_sends_it_home_and_archiving_its_own_takes_it_along`; Workforce `a_lent_agent_works_for_one_objective_of_the_other_team_then_comes_home`, `a_member_lent_away_does_not_hide_the_useful_reason`; broker `a_lent_auditor_…`, `a_moved_full_time_agent_works_under_its_new_projects_limit_from_its_next_objective`; `Inspector.test.tsx`; `OrganizationCanvas.test.tsx`; E2E. |
| **trash can and Archived drawer**   | ADR-053 §9–§11: drop an agent to archive it (Undo); a Supervisor or Manager asks first about its project or department; a VP gets the Ledger's refusal in words; the drawer is Phase 17's Archived list with its actions.                                                                                                                                                                                | `OrganizationCanvas.test.tsx` (archive with Undo, the questions, the refusal, the drawer, never under the details panel); E2E.                                                                                                                                                                                                                                                                                                                                                                                                                        |
| **toolbar**                         | ADR-053 §12: Select, Move the view, Arrange, Tidy up, zoom, Fit, Filters, Legend, Where, the trash can, Add (department, project, or role), and **?**; a toolbar for the keyboard (arrow keys); keys V, H, A, and the space bar.                                                                                                                                                                         | `OrganizationCanvas.test.tsx`; E2E (every test uses it).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| **filters**                         | ADR-053 §13: department, project, status, AI tool, AI company, rank, specialty, and the search; leads above a match stay, faded; "Showing N of M".                                                                                                                                                                                                                                                       | `canvas.test.ts`; `OrganizationCanvas.test.tsx`; E2E.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| **legend**                          | ADR-053 §15: one list of every mark, drawn from the same list the canvas uses (`data-symbol` on each mark), hidden to begin with and remembered.                                                                                                                                                                                                                                                         | `canvas.test.ts` "has one line per mark"; `OrganizationCanvas.test.tsx` "the legend lists every mark a full canvas shows"; E2E ([legend](evidence/phase-18/canvas-legend.png)).                                                                                                                                                                                                                                                                                                                                                                       |
| **live view**                       | ADR-053 §17–§20: working tiles; a "where" line (thinks in, runs on, touching) under the **Where** switch; hand-offs moving along the lines, or still arrows with reduce motion.                                                                                                                                                                                                                          | broker live-view tests; `canvas.test.ts`; `OrganizationCanvas.test.tsx` "hand-offs move along the line, or stand still with reduce motion", "reads the live view again when Guard or Liaison records something"; E2E.                                                                                                                                                                                                                                                                                                                                 |
| **a guide to the canvas**           | ADR-053 §21: a six-step tour the first time, **?** for the guide and the tour again.                                                                                                                                                                                                                                                                                                                     | `OrganizationCanvas.test.tsx` "shows a first-time tour of six steps, and ? starts it again"; E2E ([tour](evidence/phase-18/canvas-tour.png)).                                                                                                                                                                                                                                                                                                                                                                                                         |
| **the owner's tile**                | ADR-056: your picture (chosen with Windows' own box, shrunk in the window, kept in the Ledger as a small PNG), status light with its word, mood, and message, on the canvas and in the top bar; local only. **Do not disturb** holds Windows pop-ups and shows them as one when it ends.                                                                                                                 | Workforce `the_owners_picture_status_mood_and_message_are_saved_and_read_back`, `only_a_small_real_picture_and_a_short_line_are_kept`; desktop `do_not_disturb_holds_the_pop_ups`; `owner.test.tsx`; `picture.test.ts`; E2E ([panel](evidence/phase-18/canvas-owner-panel.png), [tile](evidence/phase-18/canvas-owner-tile.png)).                                                                                                                                                                                                                     |
| **watch a worker write code, live** | ADR-055: **Watch** beside a working tile and **Watch a worker** in the terminal panel's New menu; one tab per agent; the files of its latest objective; each file's new and changed lines; being written, saved, waiting for your approval, refused, not saved; Follow along and Pin this file; Stop; read-only.                                                                                         | Hub tests (16) in `watch.rs`; broker Watch tests; runtime `a_file_change_being_written_becomes_previews`, `kimis_recorded_write_streams_as_a_change_being_written`; `CodeWatchView.test.tsx`; `code.test.ts`; E2E.                                                                                                                                                                                                                                                                                                                                    |

## 3. Plan tests → evidence

| Test (plan)                                                                                                                             | Evidence                                                                                                                                                                                                                                                                |
| --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A moved tile stays where it was put after a restart; Tidy up restores the automatic layout.                                             | Ledger `a_moved_tile_stays_where_it_was_put_after_a_restart_and_tidy_up_forgets_it`; `OrganizationCanvas.test.tsx`; E2E (Plenipo restarted between the drop and the check).                                                                                             |
| Dragging a line end to a valid agent rewires it; to an invalid one, it is refused with the reason.                                      | `OrganizationCanvas.test.tsx` "drags the selected agent's line end to a new lead, and refuses one the rules refuse", "moves an oversight line's team end in one step"; Ledger `an_oversight_line_is_moved_by_its_end_in_one_step` (and its refusals); E2E.              |
| Lend: the agent takes one objective from the other team under that project's permission limit, then goes home; the Ledger records both. | broker `a_lent_auditor_works_one_objective_under_the_other_projects_limit_then_goes_home` (Guard uses the borrowing project's limit); Workforce `a_lent_agent_works_for_one_objective_of_the_other_team_then_comes_home` (`org.agent_lent`, `org.agent_returned`); E2E. |
| Trash: drop archives, Undo restores; the drawer brings back and deletes for good.                                                       | `OrganizationCanvas.test.tsx`; E2E (archive, Undo, archive again, Bring back from the drawer). Delete for good from the drawer uses Phase 17's command and question (`OwnerControl.test.tsx`).                                                                          |
| Each filter narrows the canvas; the legend lists every symbol that can appear.                                                          | `canvas.test.ts` (every filter); `OrganizationCanvas.test.tsx` "the legend lists every mark a full canvas shows" (every `data-symbol` on a full canvas is in the legend); E2E.                                                                                          |
| The live view shows the right place (this PC, a server, an AI company) for a worker in each case.                                       | broker `the_live_view_says_where_the_work_runs_in_each_case`; `canvas.test.ts` "says where each worker thinks, runs, and what it touches, in each case"; E2E.                                                                                                           |
| Reduce motion turns the moving hand-offs into still markers.                                                                            | `OrganizationCanvas.test.tsx` "hand-offs move along the line, or stand still with reduce motion".                                                                                                                                                                       |
| The owner's picture, status, mood, and message are saved and shown.                                                                     | Workforce `the_owners_picture_status_mood_and_message_are_saved_and_read_back`; IPC `places_lending_and_the_owners_tile_through_ipc`; `owner.test.tsx`; `OrganizationCanvas.test.tsx` "shows your picture, status, mood, and message on the canvas"; E2E.               |
| Watch: each `write_file`, `edit_file`, and ACP file write by a fake worker appears in the tab in order, with the right file and lines.  | broker `watch_shows_every_file_change_as_it_lands_with_its_lines` (a write, an edit, a refused edit, a large file, and a Kimi write, in order, each with its lines; after a restart, only the agent's own files); `CodeWatchView.test.tsx`.                             |
| Watch: a streamed change shows as "being written", then "saved"; a change Guard refuses shows as "refused" and never as saved.          | broker `a_streamed_change_shows_being_written_then_saved_and_a_refused_one_never_saved` (only complete lines while being written; the secret never shown); hub `a_change_being_written_turns_saved_or_refused_and_never_both`; E2E.                                     |
| Watch: the tab cannot write to the working copy; Stop stops the worker.                                                                 | IPC (Watch's commands take nothing to write); `CodeWatchView.test.tsx` "is read-only: there is nowhere to type", "Stop stops the task of the file shown"; E2E (nothing in the tab can be typed into).                                                                   |
| Watch: a large or binary file shows a summary.                                                                                          | hub `a_large_or_non_text_file_shows_a_summary_not_its_contents`, `a_very_different_large_file_is_summed_up_quickly`; broker (a 300 KB file); `CodeWatchView.test.tsx`.                                                                                                  |
| End-to-end tests in the real app, with screenshots in `evidence/phase-18/`.                                                             | `tests/e2e/specs/canvas.e2e.mjs` (7 tests, 17 screenshots above).                                                                                                                                                                                                       |

## 4. The owner's decisions and rules → evidence

| Decision or rule                                                                                                                                        | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A refused change's record keeps the file, the size, and why, and no text (ADR-055 §13).                                                                 | broker `watch_shows_every_file_change_as_it_lands_with_its_lines` (`guard.denied` detail is ".env (an edit of 16 characters)"); E2E (the refused `.env` shows no text, and nothing was written).                                                                                                                                                                                                                                   |
| Lending: on-call agents only; the borrowing team decides (ADR-054).                                                                                     | Ledger `lending_follows_the_rules`; broker `a_lent_auditor_…`.                                                                                                                                                                                                                                                                                                                                                                     |
| Do not disturb holds Windows notices (ADR-056).                                                                                                         | desktop `do_not_disturb_holds_the_pop_ups` (held, then shown as one when it ends).                                                                                                                                                                                                                                                                                                                                                 |
| New desktop commands are the main window's alone; the sign window and web pages are refused.                                                            | IPC `the_canvas_lending_watch_and_the_owners_tile_are_the_main_windows_alone` (all 12 commands, refused from another window, the sign window, and a web page), `the_phase_18_commands_check_what_they_are_given` (each bad input refused, for its reason), `watch_updates_go_to_the_main_window_only` (Watch's updates only through the main window's own channel, never an event). The sign window may now only listen to events. |
| Watch shows only what Guard already allows that worker to touch; secrets and blocked files are never shown; nothing is written that Guard would refuse. | broker `a_streamed_change_…` (a blocked file shows "A change Plenipo will not show", then refused; the secret appears nowhere); E2E (`.env` refused and not on disk). Previews check Guard's current settings, not only the step's grant.                                                                                                                                                                                          |
| Delete for good is refused while anything has unfinished work, keeps a short record, and asks first.                                                    | Phase 17's command and question, from the drawer (`OwnerControl.test.tsx`, Phase 17's Ledger tests).                                                                                                                                                                                                                                                                                                                               |
| Files, programs, the network, the browser, and the screen go through Guard and the broker.                                                              | Watch only reads what the broker carried out; the picture is chosen by the owner in Windows' own box and arrives as a small PNG, never a path Plenipo opens (IPC: `set_owner_profile` has no path field).                                                                                                                                                                                                                          |
| Logs and diagnostics files never hold secrets or anything typed in the terminal.                                                                        | Watch keeps file text in memory only; the Ledger gets the file and line counts (broker test checks the record has no text). See section 6 for what edits kept before this phase.                                                                                                                                                                                                                                                   |
| No secrets asked for in chat; none committed. No model names in commits or pull requests.                                                               | This branch's history and PR #88.                                                                                                                                                                                                                                                                                                                                                                                                  |
| Plain words on screen; ADRs named.                                                                                                                      | [Word list](../design/vocabulary.md) gains the Phase 18 pairs; `OrganizationCanvas.test.tsx` "names the filters' choices and the guide's buttons plainly".                                                                                                                                                                                                                                                                         |

## 5. Deviations from the plan

Each is recorded in its ADR's "As built" section.

- **Line ends show for the selected agent**, not also while pointing at one (ADR-053).
- **The legend starts hidden** and is remembered once shown (ADR-053).
- **Where is a switch in the toolbar**, off to begin with; hand-offs move whether it is on or not
  (ADR-053).
- **Stop in Watch** stops the task that changed the file on screen ("Stop Task 2" when the list
  holds several), not every worker of the agent (ADR-055).
- **While a change is being written, Watch shows only its complete lines**, so half a secret is
  never on screen (ADR-055).
- **Watch reaches the window through the main window's own channel**, not an event (ADR-055):
  in Tauri, a page listening to every event hears events meant for another window.
- **Numbers:** ADR-053 to ADR-056; Ledger layout 11; 12 new desktop commands (the 10 in the
  design, and `subscribe_watch` and `unsubscribe_watch`).

## 6. A question for the owner

Since Phase 7, an **edit** keeps up to 200 characters of its old and new text (secrets hidden) in
two Ledger records: the approval's, so the approval card can show what would change (kept when
you refuse it), and the Activity trail's record of an edit that was carried out. Phase 18 adds
nothing to either, and a refusal by Guard keeps no text, as you chose. Keep this as it is, or
keep only the file and the size in both (the approval card would show the text from memory
while Plenipo runs)? Recorded as a design limit in ADR-055 (Watch), "As built".

## 7. Defects found and fixed during Phase 18

A review across five areas (the Ledger and lending, the canvas, Watch, the Watch tab and your
tile, and security), each finding checked by a second reviewer before it was fixed. Every
confirmed finding is fixed with a test, or recorded where it is a limit of the design.

**The Ledger and lending**

- A lent agent could stay lent for good if a job joined its objective just after the objective
  ended. The loan now ends when that job ends
  (`a_job_that_joins_an_objective_already_ended_still_brings_it_home`).
- Send home, pressed at the same moment as a hand-off, could drop the whole hand-off. The job now
  goes ahead, and the agent comes home when it ends.
- While lent, a new title or AI tool was checked against the home team, not the team the agent
  helps. It is now checked against the team it helps.
- A lead with two agents of one title plus one lent away told its worker to do the work itself.
  It now says to name one by its title (`a_member_lent_away_does_not_hide_the_useful_reason`).
- Undo of Tidy up failed if a tile had been deleted meanwhile, or if there were more than 500
  spots. Unknown tiles are now skipped, spots go in steps of 500, and a refused save goes back
  to the tiles before it.
- A refusal message had stray spaces in it.

**The canvas**

- Drops went through the toolbar, panels, tour, and minimap, and could land on the trash can
  under the details panel. Now nothing drops through them.
- A drop landed offset after the view scrolled or zoomed during the drag. It now lands under the
  pointer.
- Holding Space with a tile focused pressed the tile instead of moving the view. Space now moves
  the view, and Enter presses the tile.
- There was no way to lend without a mouse. The Team tab now has **Lend to another team**.
- The drag hint over the trash can said the wrong thing. It now says what releasing does.
- Smaller fixes:
  - A report's line could float below the bus.
  - Line ends could be dragged in Move the view.
  - Escape that cancelled a drag also closed the details.
  - A search could select a tile the filters hide.
  - A second finger left a tile half-arranged.
  - The view drifted while an agent was held over the trash can.
  - A "where" line gave React a duplicate key.
  - Some filter and legend words were wrong.

All of these have tests in `OrganizationCanvas.test.tsx`, `layout.test.ts`, and
`canvas.test.ts`. Each test fails without its fix.

**Watch**

- Comparing a very large file's lines could take minutes on the thread that runs workers' tools.
  The comparison now stops after 0.3 s and shows a summary instead, and it no longer runs on
  that thread (`a_very_different_large_file_is_summed_up_quickly`).
- A change waiting for your approval stayed "waiting" when the worker's step ended. It now says
  the step ended before you answered (`a_change_waiting_for_an_answer_ends_with_its_step`).
- Two changes to one file in one message could be mixed up, and a late preview could undo a
  saved change. Changes to one file are now settled in order
  (`two_changes_to_one_file_are_settled_in_order_and_a_late_preview_is_dropped`).
- A preview could show half of a secret the filter could not recognize yet. Previews now show
  only complete lines, and only while Guard's current settings allow the file. A path named
  twice in one call is never previewed.
- After a restart, an agent's list could show other agents' files, and a new file hid the
  earlier ones. The list now shows the agent's own files and merges in the earlier ones (broker
  Watch test).
- Smaller fixes:
  - Memory use is now counted fully.
  - A grown block's extra lines are marked new.
  - Long paths pair up again.
  - Previews are read at a steady pace.
  - Files are read only up to the Watch limit.

**The Watch tab and your tile**

- Stop acted on the newest change, not the file on screen. It now stops the file's own task.
- A saved change whose lines Plenipo no longer keeps looked empty. It now says so.
- Two objectives at once made the list reset over and over.
- A failed read was never tried again. It now can be.
- Agents that share a title are now named by their team.
- Saving before your details were read could wipe them. Save now waits, and says when they
  could not be read.
- A slow save could be cancelled halfway.
- Tab could leave the panel.
- A huge picture was decoded at full size. It is now refused over 50 megapixels, and only the
  middle square is decoded. The first version of this fix measured the picture through a
  `blob:` address, which the window's content policy refuses; the end-to-end test caught it,
  and the picture is now measured from a `data:` address, which the policy allows (a test
  checks it). The policy itself is unchanged.

These have tests in `CodeWatchView.test.tsx`, `code.test.ts`, `owner.test.tsx`, and
`picture.test.ts`.

**Security and notices**

- Watch's updates were an event sent to the main window. In Tauri, a page listening to every
  event also hears it. They now travel through a channel only the main window can open, and the
  sign window may only listen to events (`watch_updates_go_to_the_main_window_only`).
- A preview checked only the step's grant, not Guard's current settings. It now checks both,
  and shows nothing when the settings can't be read.
- **Do not disturb** dropped the notices it held, and its hint said they would wait. They now
  come as one when it ends (`do_not_disturb_holds_the_pop_ups`).
- A race in stopping a server command: Stop could arrive before the command could hear it. Stop
  now reaches the command from the moment its start is recorded. An older SSH test hit this
  once under load.

**Recorded, not changed**

- The text an edit keeps in the Ledger: a design limit from Phase 7, and the owner's question
  (section 6).
- One reviewer finding was already fixed when it was checked (Undo of Tidy up).
- One was a deliberate difference: line ends show for the selected agent only.

## 8. Left for the owner (on Windows)

- **The walk-through** on a real PC with real AI tools:
  - Arrange the canvas and rewire two lines.
  - Lend a Security Auditor to another department for one objective, and see it come back.
  - Archive with the trash can, then bring it back from the drawer.
  - Filter to one department.
  - Read where each worker runs and what it touches.
  - Open Watch while a Senior Developer on Claude Code writes a feature.
- **Which AI tools show code letter by letter** on their real programs. Claude Code and Kimi are
  expected to. Grok and Codex are to be checked.
- **The upgrade from 1.10.0**: a backup is taken, then Ledger layout 11 is applied.
- **Your answer to section 6.**
