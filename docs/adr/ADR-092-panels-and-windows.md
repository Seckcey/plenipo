# ADR-092: Panels and windows — resize, dock, pop out, drag out, and Reset layout

- **Status:** Accepted (2026-09-30). It carries out the owner's answers in
  [ADR-091 (what the check found, and the owner's answers)](ADR-091-phase-21-owners-answers.md);
  the owner reviews it with Phase 21's pull request.
- **Date:** 2026-09-30
- **Phase:** 21
- **Amends:** [ADR-031 (the terminal panel)](ADR-031-terminal-panel.md) §1 (the panel can sit on
  the left, and in its own window); [ADR-033 (pages, notices, and Settings)](ADR-033-pages-notices-settings.md)
  §8 (a terminal belongs to the window that shows it);
  [ADR-055 (Watch)](ADR-055-watch-a-worker-write-code.md) §14 (who hears Watch)
- **Keeps:** [ADR-009 (the organization engine and canvas)](ADR-009-workforce.md) §12: drags use
  pointer events, never HTML5 drag-and-drop

> **On screen** (ADR-010, plain words and rank names): **panel**, **Terminal**, **Files**, **Move
> to the left**, **Move to the right**, **Move to the bottom**, **Pop out**, **Put back**, **Hide**,
> **Reset layout**. Not "dock zone", "pane", "webview", or "window label".

## In short

Plenipo's side and bottom panels (the terminal and the new file view) can be resized, moved to the
left, right, or bottom, and popped out into their own window, by a menu or by dragging the panel's
tab. Drag a tab outside Plenipo's window and the panel opens in its own window right there. A
popped-out panel is the same panel: its terminals keep running and its Watch tabs keep following.
Plenipo remembers the layout, and **Reset layout** puts it back the way it started. **Accepting this
record means** building it as written below.

## Context

Phase 21 of `ROLLOUT_PLAN.md`: "every side and bottom panel can be resized, docked (left, right,
bottom), moved by dragging its tab, and popped out into its own window; dragging a panel outside
Plenipo's window pops it out there; layouts are saved; Reset layout". Its technical notes: "each
window type has its own permission file listing only the commands its panels need (ADR-033's rule);
IPC tests per window type. A popped-out panel is the same panel, not a copy"; "dragging out is done
with pointer events and the window's edges (no HTML5 drag-and-drop, ADR-009 §12): a pop-out window
opens where the panel was dropped". Its tests: "resize, dock, pop out, drag out, and reset each
restore correctly after a restart"; "a popped-out window can call only its own commands".

What the code has (ADR-091, items 1 to 4): one window per organization, updates only to the window
named `main`, the terminal panel at the bottom or on the right (resized from its top or left edge),
and the Organization page's details panel.

Tauri (the app's window framework, 2.11) lets a page open a second window with `window.open` when
Plenipo's own code allows it, and that window stays joined to the page that opened it: the page can
draw into it, as it draws into itself. Both engines Plenipo runs on do this (WebView2 on Windows,
WebKitGTK on Linux). The terminal's screen part (xterm.js 6) and the editor (CodeMirror 6) both
support being moved into such a window.

## Decision

### The panels

1. **Two panels:** **Terminal** (the panel ADR-031 made: your terminals, the server watch tabs, and
   the Watch tabs for code) and **Files** (the file view, ADR-093). Each is in one of three docks —
   **left**, **right**, or **bottom** — or in its own window.
2. **A dock** holds one or more panels, each with a tab at the dock's top. It shows one at a time,
   and can be hidden. A dock with no panels takes no room. The bottom dock sits under the page,
   between the left and right docks.
3. **The Organization page's details panel stays with that page.** It shows the tile you picked, so
   it belongs to the page, not to the window. It still resizes, as before. (A dockable details panel
   would need the canvas rebuilt around it; it is left for later.)

### Resize, dock, and move

4. **Resize** by dragging the dock's inner edge, or with the arrow keys, Home, and End on that edge.
   A dock is at least 120 pixels, and the page keeps at least 180.
5. **Dock** from the panel's menu (**Move to the left**, **Move to the right**, **Move to the
   bottom**), or by **dragging the panel's tab** onto another dock. While a tab is dragged, the docks
   it can go to are marked.
6. **Drags use pointer events and the window's edges** (ADR-009 §12). Every drag has a menu or
   keyboard way to do the same.

### Pop out

7. **Pop out** from the panel's menu, or **drag the panel's tab outside Plenipo's window**: when the
   pointer lets go past the window's edge, the panel opens in its own window there.
8. **The same panel, not a copy.** The organization's window draws its popped-out panels itself (the
   new window is opened with `window.open`, which Plenipo allows only right after the page asks for
   that panel). The panel's parts move into the new window as they are: terminals keep running with
   what they showed, Watch keeps following, and the file view keeps what is open.
9. **Put back** (in the pop-out's menu), closing the pop-out window, or dragging its tab back into
   Plenipo's window puts the panel back in a dock (the one it came from, or where it was dropped).
10. **A pop-out follows its organization's window.** It hides with it (to the tray) and shows with
    it; it closes when Plenipo quits. If the organization's window reloads, its pop-outs close, and
    open again once the page is back.

### Saved, and Reset layout

11. **The layout is kept on this computer,** for each organization's window: where each panel is,
    which dock is open, which panel shows in it, and each dock's size. Plenipo keeps each pop-out
    window's place and size itself (`windows.json` in its data folder, no names or paths), and puts
    it back on the same screen if that screen is still connected, or beside Plenipo if not. After a
    restart, the layout comes back the same.
12. **Reset layout** (in each panel's menu, and in Settings → Personalization) puts every panel back
    where it started: the terminal at the bottom and Files on the left, both hidden, at their first
    sizes; it closes the pop-outs and forgets their places.
13. **The keyboard:** **Ctrl+`** shows or hides the terminal (as now); **Ctrl+Shift+E** shows or
    hides Files, as in Visual Studio Code. A popped-out panel's keys work in its own window too.
14. **The terminal panel's old place** (`plenipo.terminal`: open, bottom or right, and its size)
    becomes the terminal's place in the new layout, so nothing moves on the first start.

### Each window's own permission file

15. **Three kinds of window, each with its own permission file** (the plan's rule, written in
    ADR-091 §9):
    - **An organization's window** (`main`, and `org-…` for a second organization, ADR-094):
      `capabilities/default.json`, every command, each acting on that window's organization only.
    - **A popped-out panel** (`main:terminal`, `main:files`, and so on):
      `capabilities/popout.json`, **no commands**. Its organization's window draws it and makes
      every call; the pop-out itself can call nothing.
    - **The sign** (`control-indicator`, Phase 10): `capabilities/indicator.json`, its three
      commands, as now.
16. **IPC tests for each kind of window** prove it: an organization's window reaches its commands;
    a pop-out reaches none; the sign reaches its three; web pages reach none.
17. **Pop-out windows open only when Plenipo asked.** Plenipo refuses every other `window.open` (a
    web page, a link, a script), as it does today.

### Who hears what

18. **Live updates go to the organization's own window** (and so reach its pop-outs, which it
    draws), never to another organization's window, the sign, or a web page. This widens ADR-055
    §14 from "the main window" to "the organization's own window".
19. **A terminal belongs to the window that shows it** (amends ADR-033 §8): its organization's
    window, which draws its pop-outs. Reloading that window closes its terminals, as before; popping
    the panel out or putting it back does not.

## Consequences

- The owner can put the terminal on a second screen, keep the file view on the left, and get it all
  back after a restart.
- A pop-out window has no permissions of its own, so opening one adds nothing a page could use.
- While Plenipo's window is minimized, its pop-outs still work: the page's timers may slow, but
  terminal output, Watch, and the editor draw in the pop-out's own window.
- Parts that used the page's `window` or `document` directly now use the window their panel is in
  (menus, the resize edge, sizes, the terminal, the editor).
- Linux (for development and tests) needs WebKitGTK's "let pages open windows" setting, which
  Plenipo turns on for the organization's window only; Windows needs nothing more.

## As built

Built on 2026-09-30 (v1.16.0) as decided. Where the build adds to the decision:

- **Code:** `apps/desktop/src/workspace/` (the layout, the provider that moves each panel's parts
  between docks and pop-outs, the docks, and Reset layout in Settings → Personalization);
  `workspace_windows.rs` and `workspace_commands.rs` in the app (the pop-out windows, their places
  in `windows.json`, and `prepare_pop_out`, `focus_pop_out`, `close_pop_out`, `reset_pop_outs`).
- **§15, a pop-out's label** is `popout-<panel>--<window>--<number>` (not `main:terminal`): each
  new pop-out gets a number of its own, so it never waits for an old one's label to be free, and
  its place is kept by panel and window, whatever the number.
- **§9, Put back** closes the pop-out through Plenipo as well as from the page: a window its page
  closed could stay behind unseen on Linux.
- **§10, closing:** only the owner closing a pop-out (its window's close button) puts the panel
  back. A pop-out Plenipo closes itself (Quit, a reload, Reset layout) stays in the kept layout, so
  it opens again after a restart.
- **§8, the terminal in a pop-out:** xterm.js opens again in the new window's page and keeps its
  running shell and scrollback; on Linux, WebKitGTK is told to let the page open the windows
  Plenipo allows.
- **Found in review and fixed:** pop-out windows open one at a time (two restored at once each
  get their own window); a panel put back and popped out again comes back when its window is
  closed; a window whose page never loads is closed, never left empty; Ctrl+` and Ctrl+Shift+E
  work inside a pop-out (§13); a pop-out opened while Plenipo starts in the tray stays hidden with
  it, and an AI tool's sign-in tab opening by itself never brings a hidden terminal forward (§10).
- **Checked:** `workspace.e2e.mjs` (dock, resize, pop out and put back, no window unless asked,
  the same layout after a restart, Reset layout), `Workspace.test.tsx`, `layout.test.ts`,
  `TerminalPanel.test.tsx`, and the per-window IPC tests.

## Alternatives considered

- **A pop-out as a separate page with its own permissions**, which then asks Plenipo for its
  terminals again. Rejected: it would be a copy (a terminal would lose what it showed unless Plenipo
  kept every terminal's output), and each pop-out would need commands of its own.
- **Only a "Pop out" button, no dragging out.** Rejected: the plan asks for both.
- **HTML5 drag-and-drop.** Rejected (ADR-009 §12): the Windows webview intercepts it.
- **One layout for every organization.** Not chosen: a client's window may want its own; each
  organization's window keeps its own layout on this computer.
