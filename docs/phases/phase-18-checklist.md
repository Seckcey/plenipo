# Phase 18 — Implementation Checklist

**Status:** design approved by the owner (2026-09-28); being built. To be delivered as
**v1.11.0**. Builds on v1.10.0 (Phase 17).

Source: `ROLLOUT_PLAN.md`, Phase 18 — The Organization Canvas (third in the order of work,
ADR-039), and the four records written for it:

- [ADR-053 (the organization canvas: arrange, rewire, the trash can, and a live view)](../adr/ADR-053-the-organization-canvas.md)
- [ADR-054 (move or lend an agent to another team)](../adr/ADR-054-move-or-lend.md)
- [ADR-055 (Watch: seeing a worker write code as it happens)](../adr/ADR-055-watch-a-worker-write-code.md)
- [ADR-056 (the owner's tile: your picture, status, mood, and message)](../adr/ADR-056-the-owners-tile.md)

Dates are Pacific time.

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): a box on the canvas is a **tile**, an
agent is a position on the chart, a "diff" is **new** and **changed** lines, and a "stream" is a
change **being written — not saved yet**.

**Goal (plan):** "Make the canvas the easiest way to run the organization: arrange it, rewire it,
lend and move agents, archive with a drag, and see at a glance where work, data, and compute are.
Let the owner watch a worker write code as it happens."

## Owner decisions (2026-09-28)

**The design is approved, and ADR-053 to ADR-056 are accepted, as recommended** ("The rest of
everything sounds perfect"). One choice waited for an explanation: whether a refused edit's
record keeps none of the refused text (item 6, last part; ADR-055 §13). After it, the owner
answered "yes, that sounds fine": the record keeps the file, the size, and why, and no text.

The recommendations the owner accepted:

1. **Arranging** (ADR-053): a tile moves with its team (Alt moves one tile); dropping on an empty
   spot in Select places the tile; moving tiles is not listed in the Activity trail.
2. **The trash can** (ADR-053): dropping a lead asks to archive its project or department, with
   Undo; the Archived drawer uses Phase 17's commands.
3. **Filters** (ADR-053): hide what does not match, keeping the leads above a match, faded.
4. **Lending** (ADR-054): on-call agents only (every built-in worker role); "one objective" means
   the borrowing team's whole objective; while lent, the borrowing team decides its permission
   limit, working copy, AI tools, and department rule; Send home lets it finish the task it is on.
5. **The fix found while mapping** (ADR-054 §9): Guard reads the project from each task's own
   record, so a moved full-time agent gets its new project's permission limit from its next
   objective.
6. **Watch** (ADR-055): one tab per agent; show changes waiting for your approval; keep the lines
   in memory while Plenipo runs and only the record in the Ledger; a refused edit's record keeps
   no text.
7. **The owner's tile** (ADR-056): Do not disturb holds Windows notices; the profile is kept in
   the Ledger.
8. **Version 1.11.0** (the owner's instruction when starting the phase). **Numbers:** ADR-053 to
   ADR-056 (`main` ends at ADR-052); Ledger layout 11 (`main` is at 10).
9. **Out of scope** (plan and owner): other people's profiles and showing yours to anyone
   (Phase 24); dragging panels and windows (Phase 21); paid keys (Phase 16); the AI tools page
   (Phase 19); Connections (Phase 20); any invented language.

## Design (2026-09-28)

Written before building, from a map of the code at `c5d1a71` (`main`, v1.10.0).

### 1. Saved places, the automatic layout, and Tidy up (ADR-053 §1–§6)

- **Ledger layout 11** (`0011_canvas_and_loans`): `canvas_places (tile_id TEXT PRIMARY KEY, x REAL,
y REAL, updated_at INTEGER)` for "owner", "organization", and positions; `CHECK` that x and y
  are finite and within ±100,000. No events (ADR-053 §4). Added to the export list. Deleting a
  position for good removes its row (same transaction).
- **Commands:** `place_tiles(places: [{tileId, x, y}])` (at most 500, each tile known, upsert);
  `tidy_up()` → the places it removed (for Undo; Undo calls `place_tiles` with them). The
  organization snapshot carries `places`, so the canvas loads them with the organization.
- **Layout** (`org/layout.ts`): `layoutOrganization(snapshot, collapsed, places)` runs today's
  automatic layout, then walks the tree top-down: a tile with a saved place goes there; a tile
  without one keeps its automatic offset from its lead, applied to where its lead actually is.
  Workers follow their agent. Links become one path per child: today's bus-and-branch look when
  the child is right of its lead, a curve otherwise. Bounds, hit-testing, the minimap, and
  oversight lines use the placed tiles.
- **Dragging:** the canvas's gesture machine gains three modes (Select, Move the view, Arrange),
  a **place** drop (empty spot, or anything in Arrange), a **trash** drop, and a **line end**
  drag. A move shifts the tile and its shown team (Alt: the tile alone) and saves every moved
  tile's place in one `place_tiles` call. Alt + arrow keys move the selected tile by 20 pixels.
  Drops never use HTML5 drag-and-drop (ADR-009 §12).

### 2. Rewire by dragging lines (ADR-053 §7–§8)

- **Handles:** the selected or pointed-at agent's lines show handles (buttons with labels, for
  the keyboard too): the lead's end of its "reports to" line, and both ends of its oversight
  lines.
- **"Reports to"** drops call `move_position` (the same rules in `org/rules.ts`, the same
  events).
- **Oversight** drops call the new `retarget_oversight(oversightId, overseerId?, targetId?)`:
  Ledger `retarget_oversight` ends the old row and inserts the new one in one transaction, with
  `insert_oversight`'s checks (on-call overseer, full-time target, not on its team, the title,
  the AI tool) and the events `org.oversight_ended` then `org.oversight_assigned`.

### 3. The trash can and the Archived drawer (ADR-053 §9–§11)

- **Trash** is a toolbar button and a drop target. An agent that leads no one → `archive_position`
  and a note with **Undo** (`bring_back_position`). A Supervisor → a question, then
  `archive_project`, Undo `bring_back_project`. A Manager → a question, then `archive_department`,
  Undo `bring_back_department`. A VP → the Ledger's own refusal, in plain words.
- **Notes** (today's toasts) gain an optional action button, for Undo.
- **The drawer:** `Directory`'s `Archived` list is exported and shown in a canvas panel, wired to
  the same actions as the List view (Bring back, Delete for good with its question and Workforce
  offer, Save to my Workforce). No new command.

### 4. Toolbar, filters, legend, guide (ADR-053 §12–§16, §21)

- **Toolbar** across the top of the canvas (the zoom column moves into it); `role="toolbar"` with
  arrow-key movement; new glyphs (select, hand, arrange, tidy, filter, legend, where, trash,
  help, watch, lent).
- **Filters** (`org/filters.ts`, pure): department, project, status, AI tool, AI company, rank,
  specialty, and today's search. `visibleTiles(snapshot, filters)` → the matches plus their
  leads (faded); the layout hides the rest. Kept in the session, like the view.
- **Legend** (`org/symbols.ts`): one list of every mark (key, picture, words, where it applies).
  Tiles, lines, chips, badges, and "where" marks take their class and `data-symbol` from it; the
  legend panel shows the list; shown or hidden is remembered (`localStorage`).
- **Guide:** a six-step tour on first visit (`localStorage` remembers it was seen), **?** opens a
  short help panel with "Take the tour again".

### 5. The live view (ADR-053 §17–§20)

- **`get_live_view()`** (new, `crates/workforce/src/live.rs` fed by the broker and Liaison) →
  `LiveView { workers: [{ positionId, agentId, taskId, thinksIn: "Anthropic's cloud",
runsOn: "thisPc" | { server, production } | null, touching: [{ kind: folder | server |
website | screen, name }] }], handoffs: [{ from, to, state, at }] }`:
  - **thinks in:** the company of the worker's AI tool (every model today runs in its company's
    cloud);
  - **runs on:** the control status (browser, screen, a server by name) and a program running now
    (`capability.used` with an execution not yet ended), else nothing;
  - **touching:** the latest Guard call of its current task in the last two minutes: a file's
    folder inside its working copy, a server's name, a website's host;
  - **hand-offs:** Liaison's open hand-offs of the last few minutes, from the asking agent's
    position to the one that took it, and answers.
- **Refresh:** after `liaison.*`, `guard.*`, `capability.used`, `ssh.*`, `control.*`, and
  `task.state_changed` events (debounced), and on the control status.
- **Canvas:** a "where" line on working tiles; hand-off markers moving along the path between the
  two tiles; with reduce motion (`prefers-reduced-motion`), still arrows at the middle.

### 6. Move or lend (ADR-054)

- **Ledger layout 11** also adds `loans (id, position_id, from_lead_id, to_lead_id, to_project_id,
to_department_id, until ('objective' | 'returned'), objective_task_id, state ('active' |
'ended'), going_home, started_at, ended_at, end_reason, metadata)`, one active loan per
  position, never deleted.
- **Ledger operations** (one transaction each): `lend_position` (the checks in ADR-054 §2–§3),
  `send_home` (now, or `going_home` if it is working), `loan_joined_objective` (the first task
  from the borrowing team), and ending loans when the objective ends (the task state machine,
  like workers retiring, ADR-009 §4), when a task of a going-home agent ends, and when the
  borrowing lead, project, or department is archived or deleted. Events `org.agent_lent`,
  `org.agent_returned`. `move_position`, `archive_position`, deleting for good, and lending again
  refuse a lent agent.
- **Workforce:** `OrgView` learns loans: `team(lead)` adds agents lent to it and drops those lent
  away; a lent agent's working lead, project, and department are the borrowing team's
  (`directory.rs` placement, the Router's department layer, learning's project). A hand-off from
  the home team is refused with the reason. The snapshot marks each lent agent (`lentTo`,
  `until`, `goingHome`).
- **Guard's scope from the task (the fix):** the broker takes the Workforce record from the
  step's task (the task Workforce and Liaison write for each objective and hand-off, with
  `projectId` and now `departmentId`), falling back to the conversation's; Guard uses the
  record's department when it names one. `plan_objective`'s project check accepts the task's
  own team.
- **Canvas:** the drop menu offers Move here, Lend for one objective, Lend until I send it home,
  and the oversight choices; a lent badge and a dashed lent line; Send home on the badge and in
  the Team tab.
- **Commands:** `lend_agent(positionId, toLeadId, until)`, `send_home(positionId)`.

### 7. Watch (ADR-055)

- **Broker:** `files::write` returns the old text (when the file existed, was text, and within
  the read limit) and `files::edit` its before and after; `act()` builds a `FileChange` (path,
  created or changed, before and after, line counts), filters secrets from both, adds the file and
  counts to `capability.used`, and hands the change to a **Watch hub** (in memory, 64 MB in all,
  oldest out), which emits `plenipo://watch` to the main window only. Refusals (`deny`, approval
  refused or expired, apply failures) are handed to the hub with the file and reason only; the
  refusal record for a file write keeps the file and size, no text.
- **Letter by letter:** the Claude Code parser reads `content_block_start` (a Plenipo
  `write_file`/`edit_file` tool use) and its `input_json_delta` pieces; the ACP driver reads
  `tool_call_update` `in_progress` text for a file tool (Kimi, and Grok if it sends it). A small
  incremental reader pulls `path`, `content`, `oldText`, `newText` from the JSON so far. The
  runtime hands each piece (throttled, at most every 100 ms) to the broker's new
  `ToolProvider::preview_write`, which checks the path against the step's grant (ADR-055 §11),
  filters secrets over the whole text, caps it at 256 KB, and hands it to the hub. Never stored.
- **Matching:** the hub pairs a preview with the next change the broker applies for the same
  conversation and file; unpaired previews turn "not saved" when their task ends.
- **Comparison:** `similar` (line diff) marks new, changed, and removed lines.
- **Commands:** `get_watch(positionId)` (its objective's files and states, from the hub, or the
  Ledger after a restart), `get_watch_change(changeId)` (one change's file and marks, or a
  summary). Read-only.
- **Screen:** the bottom panel's tabs gain `{ kind: "code", positionId }`; `CodeWatchView`: the
  file list, the file with marks, Follow along, Pin this file, and Stop (`cancel_agent_turn` for
  the worker's conversation). No input element.
- **Fake AI tool** (`plenipo-fake-agent`, for tests): a marker that makes the Claude persona
  stream a Plenipo `write_file` or `edit_file` as `content_block_start` and `input_json_delta`
  pieces, with pauses, before calling it; the Kimi persona's own write already streams.

### 8. The owner's tile (ADR-056)

- **Setting** `owner` in the Ledger: `{ status, mood, message, picture }` (the picture as PNG
  bytes in base64); `owner.profile_changed` records which parts changed, the status, and the mood.
- **Commands:** `get_owner_profile()`, `set_owner_profile({ status, mood, message, picture })`
  (picture: unchanged, a new PNG, or removed); the PNG is decoded with the `png` library already in
  Plenipo and refused unless it is at most 256 × 256 and 256 KB.
- **Screen:** the owner tile (picture or glyph, status light and word, mood face and word,
  message), a top-bar button with a panel to change them, and the picture chosen with a file
  input, cut square and shrunk in the window.
- **Do not disturb:** the notice sender skips Windows pop-ups while it is on; the bell still
  counts.

### 9. New desktop commands — the main window's alone

`place_tiles`, `tidy_up`, `retarget_oversight`, `get_live_view`, `lend_agent`, `send_home`,
`get_watch`, `get_watch_change`, `get_owner_profile`, `set_owner_profile`: 10 in all. Each is
added to `build.rs` and `capabilities/default.json` only (not the sign window's
`indicator.json`), and an IPC test calls each from the main window and checks that another
window, the sign window, and a web page are refused; another checks each refusal's reason for
bad input.

### 10. Words on screen

New pairs for the word list: **tile** (for "node"); **Arrange** / **Tidy up** (for "layout",
"auto-layout"); **Move the view** (for "pan", "hand tool"); **Move here** / **Lend for one
objective** / **Lend until I send it home** / **Send home** / **lent** (for "reassign",
"loan", "borrow", "secondment"); **Where** / **thinks in … cloud** / **This PC** / **touching**
(for "compute location", "data locality", "host"); **Watch** (for "live diff", "code stream");
**being written — not saved yet** / **saved** / **refused** / **not saved** (for "streaming",
"partial tool input", "pending write"); **new** / **changed** lines (for "diff", "hunks");
**Follow along** / **Pin this file** (for "auto-scroll", "lock"); **Your picture** / status /
mood / message (for "avatar", "presence", "profile").

## Deliverables (plan)

- [ ] **Arrange freely** — drag tiles anywhere; positions are saved; Tidy up re-runs the
      automatic layout.
- [ ] **Rewire by dragging lines** — "reports to" and oversight lines, with today's checks.
- [ ] **Move or lend** — Move here, or Lend for one objective or until sent home; a lent line and
      badge; home by itself when done.
- [ ] **Trash can** — drop archives, with Undo; the Archived drawer brings back or deletes for
      good (Phase 17's commands).
- [ ] **Toolbar** — select, move the view, arrange, Tidy up, zoom, fit, filters, legend, trash,
      and add department, project, or role.
- [ ] **Filters** — department, project, status, AI tool, AI company, rank, specialty, and search.
- [ ] **Legend** — every symbol, line, color, and badge; can be hidden; remembered.
- [ ] **Live view** — who is working and hand-offs moving along the lines; where the compute is;
      where the data is.
- [ ] **A guide to the canvas** — a first-time tour and a "?".
- [ ] **The owner's tile** — picture, status light, mood, message, on the canvas and in the top
      bar; local only.
- [ ] **Watch a worker write code, live** — the Watch tab and button; the file with new and
      changed lines; the files touched in this objective; being written, saved, refused; follow
      or pin; Stop; read-only.

## Technical implementation (plan)

- [ ] The canvas stays custom-built, with pointer events.
- [ ] Saved positions per organization, in the Ledger, as coordinates per tile; new tiles placed by
      the automatic layout until moved.
- [ ] Lending is a Workforce record; while lent, the agent takes objectives from the borrowing team
      under the borrowing project's permission limit; returning is recorded.
- [ ] The live view reads what Plenipo already records; motion respects reduce motion; nothing
      shown by color alone.
- [ ] Rewiring by line uses the same rules as the drop menu and records the same events.
- [ ] Watching code reads the file changes Plenipo carries out (`write_file`, `edit_file`,
      `fs/write_text_file`); no new permission.
- [ ] Letter by letter from AI tools that stream a tool call; each other AI tool checked (against
      its recorded real program here, and on the owner's PC in the walk-through).
- [ ] The Ledger records each saved change; the preview is shown, not stored; large and binary
      files show a summary.
- [ ] The Watch tab never writes to a working copy.

## Tests (plan)

- [ ] A moved tile stays where it was put after a restart; Tidy up restores the automatic layout.
- [ ] Dragging a line end to a valid agent rewires it; to an invalid one, it is refused with the
      reason.
- [ ] Lend: the agent takes one objective from the other team under that project's permission
      limit, then goes home; the Ledger records both.
- [ ] Trash: drop archives, Undo restores; the drawer brings back and deletes for good.
- [ ] Each filter narrows the canvas; the legend lists every symbol that can appear.
- [ ] The live view shows the right place (this PC, a server, an AI company) for a worker in each
      case.
- [ ] Reduce motion turns the moving hand-offs into still markers.
- [ ] The owner's picture, status, mood, and message are saved and shown.
- [ ] Watch: each `write_file`, `edit_file`, and ACP file write by a fake worker appears in the tab
      in order, with the right file and lines.
- [ ] Watch: a streamed change shows as "being written", then "saved"; a change Guard refuses shows
      as "refused" and never as saved.
- [ ] Watch: the tab cannot write to the working copy; Stop stops the worker.
- [ ] Watch: a large or binary file shows a summary.
- [ ] End-to-end tests in the real app, with screenshots in `evidence/phase-18/`.

## Owner's rules for this phase

- [ ] Plain words on screen (the word list gains the new pairs); ADRs named, not just numbered.
- [ ] No secrets asked for in chat; nothing secret committed.
- [ ] Anything touching files, programs, the network, the browser, or the screen goes through
      Guard and the capability broker (Watch reads what the broker carries out; the owner's
      picture is chosen by the owner, never by a path Plenipo opens).
- [ ] New desktop commands are the main window's alone; the sign window and web pages are refused
      (IPC tests).
- [ ] Watch shows only what Guard already allows that worker to touch; secrets and blocked files
      are never shown; nothing is written to disk that Guard would refuse.
- [ ] Delete for good is refused while anything has unfinished work, keeps a short record in the
      Ledger, and asks first (Phase 17's commands, from the drawer).
- [ ] Logs and diagnostics files never hold secrets or anything typed in the terminal (Watch keeps
      no file contents in the Ledger, logs, or diagnostics).
- [ ] No model names in commits, branch names, or pull requests.
- [ ] Version 1.11.0 everywhere, with the Phase 18 row in `docs/development/versioning.md`.
- [ ] Release notes (`docs/releases/v1.11.0.md`), the plan's Phase 18 status line and its state
      in the order of work, this checklist, and the acceptance report with screenshots in
      `evidence/phase-18/`, in Pacific time.
- [ ] A review across several areas, with a second reviewer checking each finding, before the
      final push.
- [ ] Before each push: `pnpm check`, `cargo fmt --all -- --check`,
      `cargo clippy --workspace --all-targets --locked -- -D warnings`,
      `cargo test --workspace --locked`, `pnpm bindings` with no diff (documentation-only pushes:
      `pnpm docs:check`).

## Left for the owner (on Windows)

To be listed in the acceptance report: the walk-through on a real PC with real AI tools
(arranging, rewiring two lines, lending a Security Auditor for one objective and seeing it come
back, the trash can and the drawer, a filter, reading where each worker runs and what it
touches, and Watch with a Senior Developer on Claude Code); which AI tools show code letter by
letter on their real programs (Claude Code and Kimi expected; Grok and Codex checked); and the
upgrade from 1.10.0 (a backup, then Ledger layout 11).
