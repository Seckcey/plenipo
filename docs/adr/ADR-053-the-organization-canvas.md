# ADR-053: The organization canvas — arrange, rewire, the trash can, and a live view

- **Status:** Accepted (by the owner, 2026-09-28, as recommended)
- **Date:** 2026-09-28
- **Phase:** 18
- **Carries out:** ADR-039 (the owner's notes) §2.1 (the trash can and the Archived drawer) and
  §3 (Phase 18: the canvas)
- **Amends:** ADR-009 (the organization and its canvas) §12 — tiles can be placed by hand, lines
  can be dragged by their ends, and the canvas gains a toolbar, filters, a full legend, and a
  live view

> **On screen** (ADR-010, plain words and rank names): a box on the canvas is a **tile**.
> **Arrange**, **Tidy up**, **Move the view**, **Filters**, **Legend**, **Trash**, the
> **Archived** drawer, **Where** (where each worker's work runs and what it is touching), and
> **Take the tour**. An agent is a position on the chart. This record keeps the plan's words,
> which are also the code's.

## In short

You can put every tile where you want it, and Plenipo remembers. **Tidy up** puts everything
back in neat rows. You can change who reports to whom by dragging the end of a line. Dropping an
agent on the trash can archives it, with Undo, and the Archived drawer on the canvas brings
things back or deletes them for good (Phase 17's commands, not new ones). A toolbar, filters, a
legend that explains every mark, and a short tour make the canvas easy to learn. While work
runs, the canvas shows who is working, hand-offs moving along the lines, where each worker's
work runs (the AI company's cloud, this PC, or a server by name), and what it is touching (a
folder, a server, or a website). Accepting this record means building it as written below.

## Context

Phase 18 of `ROLLOUT_PLAN.md` asks for: "arrange freely: drag tiles anywhere; positions are
saved; Tidy up re-runs the automatic layout"; "rewire by dragging lines: grab the end of a
'reports to' or oversight line and drop it on another agent; the same checks as today's drop
menu apply"; "trash can: dropping an agent on it archives it, with Undo; an Archived drawer on
the canvas brings items back or deletes them for good (from Phase 17)"; a toolbar, filters, a
legend, a live view, and "a guide to the canvas". Its technical notes: the canvas stays
custom-built with pointer events (ADR-009 §12); "saved positions are per organization, in the
Ledger, as coordinates per tile; new tiles are placed by the automatic layout until moved"; the
live view "reads what Plenipo already records … It invents nothing. Motion respects the
system's 'reduce motion' setting, and nothing is shown by color alone"; "rewiring by line uses
the same rules as today's drop menu (`org/rules.ts`) and records the same events".

What the code does today (v1.10.0, read at `c5d1a71`):

- **Layout** (`apps/desktop/src/org/layout.ts`): every tile's place is worked out from the
  organization each time, in columns from the left (you, the organization, then each level).
  Nothing about places is saved; only the view (where you looked, how close) is kept, and only
  until Plenipo closes.
- **Dragging** (`components/org/TopologyCanvas.tsx`): dragging an agent onto another opens a
  menu — report there, or review, test, or check that team. Dropping on empty space does nothing.
  The lines are drawn under the tiles and cannot be grabbed.
- **Legend:** three items (Reports to, Live worker, Oversight), always shown. **Search** fades
  what does not match; there are no filters on the canvas (the List view has two).
- **Controls:** fit, zoom in and out, the zoom level, and a switch for oversight lines. Adding a
  department, a project, or a role is in the hire palette on the left.
- **Archived items** are in the List view's Archived tab (Phase 17), with Bring back and Delete
  for good. Archiving happens from an agent's Manage tab, after a question.
- **Live:** a working tile's status dot pulses and its lines show moving dashes. Nothing shows
  hand-offs, where the work runs, or what it touches, although Plenipo records all of it:
  Liaison's hand-offs (`liaison.*`), Guard's grants (each with its working copy), each call
  (`capability.used`), server connections (`ssh.*`), and who is using the browser, the screen,
  or a server now (the control status behind the sign).
- **Where models run:** every AI model Plenipo uses today runs in its AI company's cloud,
  including Ollama's (ADR-017: cloud models only). The AI tools' programs run on this PC.

## Decision

### Arrange freely

1. **Three ways to use the pointer**, in the toolbar:
   - **Select** (the usual one, key V): click a tile to see its details. Drag an agent onto
     another agent for the menu (Move here, Lend, or oversee its team — ADR-054), onto the trash
     can to archive it, or onto an empty spot to place it there.
   - **Move the view** (key H, or hold the space bar): dragging anywhere moves the view; nothing
     else moves.
   - **Arrange** (key A): dragging a tile only places it. No menus open, so tiles can be set side
     by side without asking anything.
2. **A tile moves with its team.** Dragging a lead places it and every tile under it, keeping
   their shape. Holding **Alt** moves only that one tile. Live workers always follow their agent.
   With a tile selected, **Alt + arrow keys** move it 20 pixels (the keyboard way to arrange).
3. **Places are saved in the Ledger, per organization.** Each tile you moved keeps its spot
   (`canvas_places`: the tile — you, the organization, or an agent — and its spot on the canvas).
   A tile you never moved keeps its place from the automatic layout, **next to its lead**: if
   you moved a Manager, a new hire on its team appears beside that Manager, not where the
   Manager used to be.
4. **Places are how you look at your organization, not something that happened in it,** so
   moving a tile is not listed in the Activity trail. They are in the Ledger's backups and
   exports, and go with the organization (Phase 21 gives each organization its own Ledger).
5. **Tidy up** clears every saved spot, so the automatic layout comes back, with **Undo** for a
   few seconds.
6. **Lines follow the tiles.** Where a tile sits to the right of its lead, the line looks as it
   does today; anywhere else, it curves from the lead to the tile.

### Rewire by dragging lines

7. **Line ends have handles.** When you select or point at an agent, the lead's end of its
   "reports to" line, and both ends of its oversight lines, show a round handle.
8. **Drop the handle on another agent:**
   - the lead's end of a "reports to" line: the agent now reports there — the same move, the
     same checks (`org/rules.ts`), and the same events (`org.position_moved`, and
     `org.project_reassigned` when a Supervisor moves between departments) as "Move here";
   - the team's end of an oversight line: the same reviewer, QA evaluator, or security auditor
     now oversees that team instead;
   - the overseer's end: another on-call agent takes over the assignment.
     An oversight change ends the old assignment and makes the new one **in one step**
     (`org.oversight_ended` and `org.oversight_assigned`, the events the drop menu records), so it
     never half-happens. A drop the rules refuse shows why, and changes nothing.

### The trash can and the Archived drawer

9. **Drop an agent on the trash can to archive it,** with the same checks as Archive (it leads
   no one, and nothing it has is unfinished). A note says "Archived Security Auditor" with
   **Undo**, which brings it back (Phase 17's Bring back). A full-time agent brought back starts
   a new conversation (ADR-043 §5), and the Undo says so.
10. **Dropping a lead on the trash can asks first:** a Supervisor asks "Archive the Website
    project and its team?"; a Manager asks "Archive the Development department and everything in
    it?". Both have Undo afterwards. A VP leads departments, so the trash can explains what to
    archive first.
11. **The Archived drawer** opens from the trash can: a panel on the canvas listing archived
    departments, projects, and agents — the List view's Archived tab, the same rows and
    buttons — with **Bring back** and **Delete for good**. Delete for good asks first, is refused
    while anything has unfinished work, offers to save experienced agents to your Workforce, and
    leaves a short record in the Ledger (ADR-043, ADR-045). **Phase 17's commands do all of this;
    nothing is built again.**

### The toolbar

12. **One toolbar along the top of the canvas:** Select, Move the view, Arrange · Tidy up ·
    zoom out, the zoom level, zoom in, Fit · Filters, Legend, Where, Oversight lines · **Add**
    (a department, a project, a new role, or hire an agent) · the trash can · **?** (the guide).
    Every button has a word or a tooltip and a key where it has one. The minimap stays.

### Filters

13. **Filters:** department, project, status, AI tool, AI company, rank, and specialty, plus
    today's search. Each filter narrows what the canvas shows: tiles that do not match are
    hidden, and the leads above a match stay, faded, so its lines still make sense. A note says
    "Showing 5 of 23 agents" with **Clear filters**. Filters last until Plenipo closes.
14. **AI company** is the company of the agent's AI tool (Anthropic for Claude Code, OpenAI for
    Codex, xAI for Grok, Moonshot AI for Kimi, Ollama for Ollama). Each model's own maker comes
    with Phase 16 (ADR-036); then this filter uses the maker.

### The legend

15. **The legend explains every mark that can appear:** each kind of tile, each status (a dot
    **and** a word), each line (reports to, live worker, review, QA, security, lent, a hand-off),
    each chip (department, project, AI tool), each badge (lent, fixed AI tool, specialty, new
    worker), each "where" mark (the AI company's cloud, this PC, a server, a folder, a website),
    and each drop state. It can be hidden, and Plenipo remembers whether it was shown.
16. **One list drives both:** the canvas draws its marks from the same list the legend shows, and
    a test checks that every mark on a canvas that has every kind of thing has its legend line.

### The live view

17. **Who is working:** as today (the status dot and word, moving dashes on its line), plus a
    **hand-off moving along the line** from the agent that asked to the agent that took it, and
    back when it answers — from Liaison's own records.
18. **Where the work runs,** under each working tile, with a small picture and words:
    - **the AI company's cloud** ("Anthropic's cloud") while the model is thinking — today every
      model Plenipo uses runs there;
    - **This PC** while it runs a program, uses Plenipo's browser, or uses the screen, mouse,
      and keyboard here;
    - **a server by name** ("Shop · PRODUCTION") while it is connected to a server.
19. **What it is touching now:** the folder it last read or wrote in its working copy
    ("Website · src/pages"), the server it is using, or the website it has open ("shop.example.com"),
    from Guard's calls in its current task. Nothing is guessed; a worker that has touched nothing
    yet shows only where it runs.
20. **Reduce motion:** when Windows' "reduce motion" (animation effects off) is set, hand-offs
    show as still arrows at the middle of the line, and nothing moves. Nothing is shown by color
    alone: every mark has a picture and a word.

### A guide to the canvas

21. **A short tour the first time** you open the canvas (six steps: arranging, lines, move or
    lend, the trash can, filters and the legend, Watch), which you can skip. **?** in the toolbar
    explains the canvas in a few lines and can start the tour again.

### Commands and checks

22. **New desktop commands, the main window's alone** (like every command since Phase 7):
    `place_tiles` (save the spots of the tiles you moved), `tidy_up` (clear them, returning them
    for Undo), `retarget_oversight` (move an oversight line's end), and `get_live_view` (who is
    working, hand-offs in flight, where each worker's work runs and what it touches). Each is
    refused from the sign window and from any web page, with IPC tests. The trash can, the
    drawer, and "reports to" lines use the commands that exist.

### In the Ledger

23. **Ledger layout 11** adds `canvas_places` (with ADR-054's `loans`), after a backup, as for
    every layout change. Deleting an agent for good removes its spot.

## Your choices (recommended first)

- **A tile moves with its team** (Alt for one tile). _Or:_ a tile always moves alone — simpler,
  but moving a Manager leaves its team behind with long lines.
- **Dropping on an empty spot in Select places the tile.** _Or:_ only in Arrange — fewer
  surprises, but one more click every time.
- **Places are not in the Activity trail.** _Or:_ list "You moved 3 tiles" — honest, but it would
  flood the trail while you arrange.
- **Dropping a lead on the trash can asks to archive its project or department.** _Or:_ refuse,
  and point to the Manage tab.
- **Filters hide what does not match** (leads above a match stay, faded). _Or:_ fade instead of
  hide, like search.

## Consequences

- The canvas becomes the easiest place to run the organization; the List view and each agent's
  details still do everything the canvas does, without a mouse.
- A hand-arranged canvas can drift out of shape as the organization grows; Tidy up (with Undo)
  fixes it in one click.
- The live view adds one light command that Plenipo refreshes when Guard, Liaison, or a server
  records something; it reads records Plenipo already keeps and stores nothing new.
- The legend and the canvas cannot drift apart, because they share one list and a test.

## Alternatives considered

- **A graph library.** Rejected again (ADR-009 §12): the canvas is a few hundred tested lines,
  and HTML5 drag-and-drop is intercepted by the Windows webview.
- **Keep places on this PC only (like the view's zoom).** Rejected: the plan puts them in the
  Ledger, per organization, so they survive a restart, a restore, and Phase 21's organizations.
- **Rewire only through the drop menu.** Rejected: the plan asks for dragging lines by their
  ends, which says exactly what you mean without a menu.
- **A separate "delete" can.** Rejected by the owner (ADR-039 §2.1): the trash can archives, and
  deleting for good happens from the drawer, after a question.
- **Guess where data is from the AI tool's own output.** Rejected: only Guard's own records say
  for certain which folder, server, or website a worker touched.

## As built (v1.11.0)

Built as written, with these small differences, each to keep the canvas calm and the other
screens working:

- **Line ends show for the selected agent** (§7), not also while pointing at one: moving the
  pointer from a tile to its line end would otherwise hide the handle on the way. Select an agent,
  then drag a round end; or press Enter on it to choose from a list.
- **The legend starts hidden** (§15) and Plenipo remembers once you show it, so it never covers
  part of the canvas unasked. The first-time tour points to it.
- **Where is a switch in the toolbar** (§18), off until you turn it on; while it is on, the rows
  get more room so each working tile's "where" line fits under it. Hand-offs moving along the
  lines (§17) show whether it is on or not.
- **Badges** (§15): lent, fixed AI tool ("Fixed"), specialty, and experienced (a star, with its
  word for screen readers); the plan's "new worker" is the live worker tile.
- **Fitting leaves the toolbar's strip clear**, so no tile sits under the toolbar after Fit to
  screen.
- The keyboard: V, H, and A choose the pointer's mode; the space bar moves the view while held;
  Alt and the arrow keys move the selected tile. The **?** button opens the guide (there is no
  "?" key).
