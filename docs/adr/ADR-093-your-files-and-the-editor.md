# ADR-093: Your files and the editor — the file view, one writer at a time, Watch in the editor, and files on an objective

- **Status:** Accepted (2026-09-30). It carries out the owner's answers in
  [ADR-091 (what the check found, and the owner's answers)](ADR-091-phase-21-owners-answers.md),
  answers 7 and 8; the owner reviews it with Phase 21's pull request.
- **Date:** 2026-09-30
- **Phase:** 21
- **Carries out:** [ADR-039 (the owner's notes)](ADR-039-owners-notes-order-of-work.md) §2.3 ("the
  owner opens and edits project files") and §2.12 ("from Phase 21, the same view runs inside the
  file view and editor")
- **Amends:** [ADR-016 (the Development department)](ADR-016-development-department.md) §3 (the
  owner is told who holds a working copy, and can stop it);
  [ADR-055 (Watch)](ADR-055-watch-a-worker-write-code.md) §8 (a change says which working copy it
  is in)
- **Keeps:** workers' file access goes through Guard, unchanged (ADR-013)

> **On screen** (ADR-010, plain words and rank names): **Files**, **Project folder**, **Working
> copies**, **Open in Plenipo**, **Open in another program**, **Show in folder**, **Save**,
> **Reload**, **Save anyway**, **Wait**, **Stop the worker**, **being written — not saved yet**,
> **new** and **changed** lines, **Files for this objective**. Not "worktree", "repo", "diff",
> "buffer", "lock", or "IPC".

## In short

A **Files** panel lists each project's folder and its working copies. Open any file in Plenipo:
code with colors, pictures shown, anything else described. Edit and save; each save is in the
Activity trail as yours. A working copy a worker is writing opens read-only, says which worker,
and offers **Wait** or **Stop the worker**. A file a worker is writing changes on your screen as
it lands. Programs and scripts open as text and are never started. Files dropped on an objective,
from anywhere, are copied for its workers. **Accepting this record means** building it as written
below.

## Context

Phase 21 of `ROLLOUT_PLAN.md` asks for "a file view in the side and bottom bars: each project's
folder and working copies as a tree"; "open and edit files in a built-in editor (text and code with
highlighting, pictures shown); save; open in another program; drag files onto an objective to
attach them"; "one writer at a time: a working copy a worker is writing opens read-only, names the
worker, and offers Wait or Stop the worker (ADR-016)"; and "watch in the editor: a file a worker is
writing changes live in the editor, the same way as Phase 18's Watch tab, and the file tree marks
the files a worker is changing now". Its technical note: "the owner's reads and edits go through a
Tauri command limited to the project folders and working copies Plenipo knows about. Each save is
recorded in the Activity trail as the owner's action. Workers are unaffected: their file access
still goes through Guard." The owner's answers (ADR-091, 7 and 8) add: always offer **Open in
Plenipo**; never start programs or scripts; blocked files are the owner's, hidden while a worker
uses the screen, mouse, and keyboard; the worker goes ahead if the owner is editing; files from
anywhere may be dropped on an objective.

What the code has (ADR-091, items 5 to 7): no command reads a project file for the owner; the
broker alone knows, in memory, which worker holds a working copy for writing; a project with no
working copy has no writer tracking; a Watch change does not say which working copy it is in; an
objective is text only. Guard's path checker (`crates/guard/src/paths.rs`) settles `..`, links,
junctions, device names, and letter case against a folder's real path.

## Decision

### The file view

1. **Files lists what Plenipo knows:** each active project, with its **Project folder** (when it has
   one) and its **Working copies** (named by branch: "plenipo/fix-login-a1b2", and "Senior Developer
   is writing here" when one is). Nothing else on the PC is listed or opened.
2. **Folders load as you open them,** sorted folders first. Git's own folder (`.git`) is never
   listed. A folder with more than 2,000 entries shows the first 2,000 and says how many more.
3. **Marks, each with a word as well as a sign:** a file a worker is changing now ("Senior Developer
   is changing this"), and a file workers may not touch ("Blocked for workers").
4. **Each file offers Open in Plenipo first** (and a double-click or Enter does it), then **Open in
   another program** (not for programs and scripts), **Show in folder**, and **Copy path**. Watch
   tabs and the Projects page offer **Open in Plenipo** too.

### The editor

5. **The editor is a page** (in the Projects section, "File · README.md"), with a tab for each open
   file. It is **CodeMirror 6** (a widely used code editor part, MIT license), with colors for the
   language found from the file's name; languages load as needed. Its colors come from the design
   tokens.
6. **What opens how:**
   - **Text and code** (UTF-8, with or without a byte-order mark) up to 5 MB, in the editor. Line
     endings (Windows or Unix) and the byte-order mark are kept when saving.
   - **Pictures** (PNG, JPEG, GIF, WebP, BMP, ICO, and SVG, shown as a picture) up to 20 MB.
   - **Programs and scripts** (`.exe`, `.bat`, `.cmd`, `.ps1`, `.vbs`, `.msi`, `.lnk`, and the rest
     of Windows' list of files that run) open as text when they are text, and are described when
     they are not. **Plenipo never starts them.**
   - **Anything else** (a Word file, a PDF, a large file) shows what it is and its size, with **Open
     in another program** and **Show in folder**.
7. **Save** (the button, or Ctrl+S) writes the file through Plenipo: a new copy beside it, then
   swapped in, so a crash never leaves half a file. It is recorded in the Activity trail as yours
   (`file.saved`: the project, the working copy if any, the file's path inside it, its size, and the
   lines added and removed; **never its contents**). "You saved README.md in Website".
8. **Plenipo refuses a save,** and says why, when:
   - the file is outside the project folders and working copies Plenipo knows, or inside `.git`;
   - a worker is writing in that working copy (or project folder) now (item 10);
   - a worker is using the screen, mouse, and keyboard, until you **Take over** (item 14);
   - the file changed on disk since you opened it: **Reload** (lose your changes) or **Save anyway**.
9. **Not in this phase:** new files, new folders, renaming, and deleting from the file view; a full
   code editor with extensions (the plan's "out of scope").

### One writer at a time

10. **A working copy a worker is writing opens read-only,** with the worker's name: "Senior
    Developer is writing in this working copy. You can read along; editing waits until it is done."
    The same holds for a project folder a worker writes in directly (a project that works in place,
    ADR-016 §2). Plenipo now tells the page who holds each one, and the Ledger's `guard.grant_*`
    events say when that changes.
11. **Wait** keeps the file read-only and unlocks it by itself when the worker is done ("Senior
    Developer is done. You can edit again."). **Stop the worker** stops that worker's task, as
    **Cancel task** does (after you confirm), and the file becomes writable once its step ends.
12. **If you are editing when a worker starts writing there,** the worker goes ahead: the file turns
    read-only, your unsaved changes stay in the editor, and you save once the worker is done (with
    **Save anyway** if it changed the same file).
13. **Checked where it is written:** Plenipo checks for a writer and writes the file while holding
    the same lock the broker holds when it hands a worker a working copy, so a worker's step can
    never start in the middle of your save.

### Blocked files, and a worker at the keyboard

14. **Blocked files (like `.env` and keys) are yours,** so you can open and edit them. **While a
    worker is using the screen, mouse, and keyboard,** the editor hides blocked files ("Hidden while
    a worker uses the screen") and takes no typing, and Plenipo refuses to save anything, until you
    **Take over**, as the terminal does (ADR-033 §8). What a worker types must never reach a file
    through the owner's editor.

### Watch in the editor

15. **A file open in the editor that a worker is writing changes as it lands,** the same way as
    Phase 18's Watch tab: **being written — not saved yet** while the AI writes it (for AI tools that
    send their changes as they write), then the saved file with **new** and **changed** lines marked.
    It needs no new permission: it shows what Guard already lets that worker touch (ADR-055 §11).
16. **The file view marks the files a worker is changing now.**
17. **Each Watch change now says where it is:** its working copy, or its project when the worker
    works in the project folder (amends ADR-055 §8). The Watch tab and the editor both use it.

### Open in another program, and Show in folder

18. **Open in another program** opens the file with the program Windows uses for it. It is never
    offered, and is refused, for programs and scripts. **Show in folder** opens File Explorer with
    the file picked. Both are the owner's own actions, like opening a link, and are not recorded.

### Files on an objective

19. **Drop files on an objective** (the box where you type it, on an agent's details or a project's
    page) **from anywhere:** from the Files panel, or from File Explorer.
20. **A file from File Explorer reaches Plenipo through Windows' own drop,** which Plenipo reads
    itself; the page only says which of the dropped files go on the objective. No page can name a
    file on the PC and have it attached.
21. **Files inside the objective's own project folder are named, not copied** ("Look at these files:
    `src/app.ts`"). **Every other file is copied** when you give the objective (up to 20 files, 25 MB
    each, 100 MB in all) into Plenipo's own folder for that objective. When the objective's first
    worker starts, Plenipo puts the copies in an `attachments` folder in its working copy (or its
    project folder, when it works in place), without replacing anything already there. The
    objective's text lists them. The originals are never changed.
22. **Recorded:** `objective.files_attached` (each file's name and size; never its contents or where
    it came from on the PC) and, when delivered, `objective.files_delivered`.

### The commands

23. **New commands, an organization's window's alone** (ADR-092 §15), each acting on that window's
    organization: `get_file_roots`, `list_folder`, `read_file`, `save_file`, `open_file_outside`,
    `show_in_folder`, and `get_changing_files`; `give_objective` gains the files to attach. A
    popped-out panel, the sign, and web pages reach none of them (IPC tests).
24. **A file is named by where it is and its path inside:** "the project folder of Website" or "the
    working copy on branch …", then `src/app.ts`. Plenipo finds the folder from the Ledger and checks
    the path with Guard's path checker (the folder's real path; `..`, links, junctions, device names,
    and letter case), so a name can never lead outside.

## Consequences

- The owner can read and fix a project's files without leaving Plenipo, and see who is writing
  where.
- Saving can collide with a worker only at the moment a step starts, and the shared lock settles it.
- Watch changes carry one more field (where they are), and `give_objective` one more argument.
- One new library family: CodeMirror 6 (`@codemirror/*`, MIT), in the page only.
- Dropped files take disk space in Plenipo's data folder until the objective is removed with its
  working copy.

## As built

Built on 2026-09-30 (v1.16.0) as decided. Where the build adds to the decision:

- **Code:** `crates/capabilities/src/broker/owner_files.rs` (the roots, folder lists, reads, saves
  with "changed on disk", and what is changing now), `attachments.rs` (files on an objective),
  `files_commands.rs` in the app (the commands, the tickets for File Explorer drops, and opening
  in another program or in File Explorer); `apps/desktop/src/files/` (Files, the editor page,
  CodeMirror's setup in `editorSetup.ts`, and files on an objective in `useObjectiveFiles.ts`).
- **§5, the editor page** keeps each open file's unsaved text while you move between pages; it is
  lost when Plenipo quits (a limit).
- **§7, Save** writes a new copy beside the file and swaps it in; where the swap cannot happen
  (a file another program holds open on Windows), it writes in place.
- **§20, File Explorer drops:** Plenipo keeps each drop's paths behind a ticket for that window
  (an hour at most, 64 drops), and the page only sends the ticket and each file's number back.
- **§21:** files are delivered into an `attachments` folder of the working copy when the
  objective's first worker starts, once.
- **Checked:** `owner_files` and `attachments` unit tests, `crates/capabilities/tests/development.rs`
  (one writer, Watch naming its working copy, files on an objective), `Files.test.tsx`, the IPC
  tests, and `workspace.e2e.mjs` (edit and save README.md, a file outside refused, a working copy
  a worker writes: read-only, live, and writable after Stop the worker).

## Alternatives considered

- **A side-by-side editor in the file view panel.** Not chosen: the editor needs the page's room;
  the panel stays a list.
- **Monaco** (Visual Studio Code's editor). Not chosen: much larger, and it needs web workers the
  app's content rules do not allow.
- **Locking a working copy while the owner edits it.** Rejected by the owner (ADR-091, 7): a worker
  could wait on an editor left open.
- **Giving workers a way to read files outside their working copy.** Rejected: dropped files are
  copied in, so Guard stays as it is.
