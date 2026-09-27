# Phase 12 — Implementation Checklist

**Status:** in progress (2026-09-27). Built on v1.7.0 (Phase 12A, the design system).

Source: `ROLLOUT_PLAN.md`, Phase 12 — Product UX, Notifications, Settings, and Operator
Experience, and [ADR-031 (the terminal panel)](../adr/ADR-031-terminal-panel.md).

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md).

**Goal (plan):** "Turn the proven engine into a desktop product that the owner can understand
and operate without watching raw terminal output."

"All screens in this phase are assembled from the Phase 12A design system and component
library. No new one-off styling." (ADR-030, one design system for every screen.)

## Owner decisions (2026-09-27)

- **The plan for this phase is approved:** a Home page; a page for each department, project,
  worker, and task; Windows pop-up notices when something needs the owner, with a place in
  Settings to choose which ones; Settings in one tidy place; the terminal panel.
- **ADR-031 (the terminal panel): accepted**, with its choices: the panel at the bottom (it can
  move to the right), opened with a **Terminal** button or **Ctrl+`**; Windows PowerShell by
default; only opening and closing a terminal is recorded, never what the owner types; Guard
does not check the owner's typing, but the server's pinned ID is still checked; the server
terminal needs the **Remote computers (SSH)** switch; **Stop all** does not close the owner's
terminals; two new libraries, `portable-pty`and`@xterm/xterm`.
- Version **1.8.0**.
- Free numbers on 2026-09-27 (check `main` and the open pull requests again before using one):
  migration **0009**, ADR **032**.
- Carried over from Phase 12A: the older pages take every color from the tokens but keep some of
  their own layout rules (their buttons, badges, and cards in `apps/desktop/src/styles.css`)
  until this phase rebuilds each page from the library (ADR-030 §8).
- Still open, not part of this phase: ADR-025 (servers over SSH, through Guard) is proposed, and
  the owner's Phase 11 check on Windows with a real server is still to do.

## Design (2026-09-27)

Written before building, from a map of the code. Numbers checked on 2026-09-27: version
**1.8.0**; no Ledger migration is needed (new queries use the existing tables and indexes, and the
owner's preferences use the existing `settings` table); **ADR-032** is taken by PR #36, so the next
free ADR is **ADR-033**.

### Brand (owner request, 2026-09-27)

- The owner's approved Plenipo + Pip brand kit is kept byte for byte in
  `docs/brand/pip-brand-kit` (its manifest lists a checksum for every file; git stores the folder
  as-is).
- The app icon (window, taskbar, tray, installer) is the kit's three-rail P on the kit's navy.
- `@plenipo/ui` gets `PlenipoMark` (the P), `PlenipoLogo` (the P, "lenipo", and Pip on the n,
  drawn from the kit's outlined wordmark, colored by four brand tokens so it follows the theme),
  and `Pip` (all 15 poses, 400 px copies of the kit's artwork). `EmptyState` and `ErrorState` can
  show Pip beside their words.
- Pip is on Home (his pose follows what is happening), beside every page's "nothing here yet",
  in the terminal panel, in Settings → About, and in the Gallery.

### The terminal (ADR-031)

- **Backend:** `crates/capabilities/src/terminal.rs` runs a shell in a pseudo terminal
  (`portable-pty`; ConPTY on Windows) and relays a server's shell channel;
  `broker/terminals.rs` opens, tracks, and records terminals, apart from every worker's grant (so
  Stop all never reaches them); `ssh.rs` gains `Connection::open_shell` (`pty-req` then `shell`,
  for the owner only). On Windows each shell lives in its own kill-on-close Job Object
  (`win32job`), and no terminal opens while Plenipo runs as administrator (`is_elevated`); both
  are safe wrappers, because the workspace forbids unsafe code. Off Windows (development, CI,
  and the end-to-end tests) the shell is `$SHELL`, else `/bin/sh`.
- **Commands (main window only):** `get_terminal_settings`, `set_terminal_shell`,
  `open_terminal` (output streams back on a Tauri `ipc::Channel`, base64), `write_terminal`,
  `resize_terminal`, `close_terminal`, and `stop_server_command` (the watch tab's Stop). A
  terminal is opened by naming a place (this PC, or a server's ID), never a program or a path.
- **Recorded:** `terminal.opened` and `terminal.closed` (where, when, how long, why), with no
  task. Never what is typed or shown.
- **Stop in a watch tab:** each worker's server command now has its own stop signal (the step's
  stop still reaches it), so the owner's Stop ends only the command running now (TERM, then
  KILL); it is recorded (`ssh.command_stop_requested`, and the command's `ssh.command_finished`
  says "you pressed Stop"). **Disconnect** is the existing `take_over_control("server:…")`.
- **Screen:** `apps/desktop/src/terminal/`: a provider (panel open, size, and side kept in
  `localStorage` under `plenipo.terminal`), the panel (library `Tabs` with close buttons, a
  `MenuButton` for **New terminal**, a `ResizeHandle`), the owner's terminal (`@xterm/xterm` and
  its fit add-on; colors from new `terminal-…` tokens with contrast pairs), and watch tabs built
  from the Ledger feed (`ssh.connected`, `ssh.command_started`, `ssh.output`,
  `ssh.command_finished`, `ssh.disconnected`, and refused `ssh_run` calls). **Ctrl+`** and the
  **Terminal** button in the top bar show and hide it.
- **Test server:** `plenipo-test-sshd --shell on` gives the owner's terminal a small shell (a
  prompt, echo, `echo`, `whoami`, `hostname`, `pwd`, `size`, `exit`); without it, a terminal and
  a shell are refused and counted, as before, so the worker checks stay true.

### Home, and a page for each department, project, worker, and task

- **Home** is the first section on the left strip and the page Plenipo opens on. Pip greets the
  owner (in Pacific time) and says in a line how things are. Then: **Waiting for you**
  (approvals and lessons, with Review), **What's stuck** (failed objectives, refused actions,
  handoffs with nobody to take them, positions that cannot work), **Departments** (a card each:
  health, its Manager, projects, workers, the 24-hour strip), **Current objectives**, **Who's
  working**, and **Just finished**.
- **Department, Project, Worker, and Task pages** open from Home, the Organization map, the
  Projects and Workers pages, the Activity trail, and each other (not from the strip). The top
  bar says where you are ("Department · Operations") with **Back**. Each is a detail page from the
  library: properties on the left, activity in the middle, a map or table below.
  - **Department:** its Manager, projects, current workers, queue, and activity history (strip,
    week, and the events).
  - **Project:** its Supervisor, folder and repository, the task tree of each objective, running
    workers, branches and pull requests, artifacts (screenshots), and recent decisions.
  - **Worker** (a position): role, AI tool and model (and why), current task, permissions in
    use, the conversation, and its event history.
  - **Task:** objective and acceptance criteria, the delegation tree, the activity stream (live),
    artifacts, approvals, and the final result.
- **New queries** (Ledger, Workforce, broker) behind a few new commands: `get_home` (objectives
  open and recently finished, with their answers, and what's stuck), `get_scope_events` (events
  of a department, project, or position, newest first, a page at a time), `get_project_record`
  (pull requests, artifacts, decisions), and `get_task_record` (approvals, artifacts, decisions of
  a task and the tasks under it). Decisions are: approvals answered or expired, refusals,
  handoffs refused, lessons kept or discarded, and why each worker got its AI tool.
- Where you are (page and item) is kept for this window and comes back after a restart.

### Windows pop-up notices

- `tauri-plugin-notification` (v2). Plenipo decides in Rust (`notices.rs`), from each committed
  Ledger event: **Waiting for your OK** (an approval), **A check to solve** (a CAPTCHA handed to
  you), **Problems** (an objective failed, a server ID changed, an AI tool signed out or at its
  usage limit), **Finished work** (an objective's result is ready), and **Lessons** (a worker
  learned something). Several at once become one notice; the same notice is not repeated within
  a minute.
- **Settings → Notifications** chooses each kind, and "only while Plenipo's window is not in
  front"; **Send a test notice** checks them. Kept in the Ledger's settings (`preferences`).
- A real notice needs the installed app's identity, so the decision is tested by unit and IPC
  tests, and the owner checks real notices on Windows.

### Settings in one tidy place

- A list of sections on the left, one section at a time (the last one comes back): **AI tools**
  (each tool and its sign-in state), **AI models** (role choices, first choice and backups in
  order), **Permissions** (permission sets and approval rules), **Organization** (departments and
  projects), **Servers** (unchanged), **Switches**, **Notifications**, **Terminal**,
  **Personalization**, **Local paths** (where Plenipo keeps its files, read-only), **Diagnostics**
  (a summary, and the raw details page), and **About Plenipo**.

### The older pages

- Each page's own buttons, badges, pills, tabs, and cards (`styles.css`) are replaced by the
  library's `Button`, `StatusPill`, `CountBadge`, `Tabs`, and panels; what each page does stays
  the same. `styles.css` keeps only page layout, from tokens.

## Deliverables (plan)

### Home / Company

- [ ] department health
- [ ] current objectives
- [ ] agents working
- [ ] blocked tasks
- [ ] approvals waiting
- [ ] recent completions

### Department View

- [ ] manager
- [ ] projects
- [ ] current workers
- [ ] queue
- [ ] performance/activity history

### Project View

- [ ] coordinator
- [ ] repository/workspace
- [ ] task tree
- [ ] running workers
- [ ] branches/PRs
- [ ] artifacts
- [ ] recent decisions

### Agent View

- [ ] role
- [ ] selected provider/model
- [ ] current task
- [ ] capabilities granted
- [ ] runtime/session
- [ ] event history

### Task View

- [ ] objective
- [ ] acceptance criteria
- [ ] delegation tree
- [ ] activity stream
- [ ] artifacts
- [ ] approvals
- [ ] final result

### Terminal panel (ADR-031)

- [ ] A panel at the bottom by default; the owner can move it to the right side
- [ ] Opens and closes from a **Terminal** button in the top bar and with **Ctrl+`**
- [ ] Drag its edge to resize; remembers whether it was open, its size, and its side, and comes
      back the same after a restart
- [ ] Several tabs at once, each with a close button, built from the Phase 12A library
- [ ] Production servers are red in their tabs (a red mark and the word PRODUCTION)
- [ ] **This PC:** Windows PowerShell in the owner's home folder, as the owner's own Windows
      user, never as administrator; Settings can pick PowerShell 7 or Command Prompt when
      installed; ConPTY through `portable-pty`; `@xterm/xterm` on screen, colors from the tokens
- [ ] **A server:** the servers from Settings → Servers; the pinned server ID is checked first,
      and a changed ID is refused before signing in or sending anything ("This server's ID
      changed"); the stored sign-in comes from the Vault, is never shown or sent to the screen,
      and is dropped after use; `pty-req` and `shell` for the owner only (never for workers);
      no agent forwarding, X11, or environment variables for anyone; built-in SSH (ADR-026)
- [ ] The server terminal works only while **Settings → Switches → Remote computers (SSH)** is
      on; the terminal on this PC does not depend on it
- [ ] Guard does not check what the owner types; no approval cards for it
- [ ] The Ledger records `terminal.opened` and `terminal.closed` (where, when, how long), shown
      in the Activity trail as "You opened a terminal on Shop"; what the owner types and sees is
      never recorded
- [ ] **Watch tabs:** one opens by itself when a worker connects to a server, named for the
      worker and the server ("Operations Engineer · Shop"); it stays open and readable after the
      worker disconnects, until the owner closes it
- [ ] A watch tab shows each command Guard let through and its output as it arrives (from
      `ssh.command_started`, `ssh.output`, `ssh.command_finished`), with secrets hidden;
      refused commands show as refused
- [ ] **Stop** (TERM, then KILL) and **Disconnect** in the watch tab, both recorded as today
- [ ] A watch tab is read-only: there is no place to type
- [ ] The terminal's commands (open, type, resize, close) exist only in the owner's main
      window: they are not tools, no AI tool is offered them, the tool relay refuses unknown
      tool names, and the sign window and web pages cannot call them
- [ ] **Stop all** still stops every worker and does not close the owner's terminals
- [ ] New libraries: `portable-pty` (Rust, MIT), `@xterm/xterm` and its fit add-on (MIT)

### Settings

- [ ] providers
- [ ] authentication state
- [ ] role/model policies
- [ ] fallback order
- [ ] capability profiles
- [ ] projects
- [ ] departments
- [ ] approval rules
- [ ] local paths
- [ ] notification preferences
- [ ] diagnostics
- [ ] Settings → Servers keeps working as before

### Notifications (the approved plan)

- [ ] Windows pop-up notices when something needs the owner
- [ ] A place in Settings to choose which notices the owner gets

### Existing screens

- [ ] Each older page is rebuilt from the library (ADR-030 §8); what each page does stays the
      same

## Tests (plan)

- [ ] keyboard navigation
- [ ] state restoration
- [ ] large task history
- [ ] large org tree
- [ ] disconnected providers
- [ ] empty states
- [ ] error states
- [ ] accessibility smoke tests
- [ ] terminal panel: open, hide, resize, and restore after a restart
- [ ] the owner's terminal on this PC and on a synthetic SSH server, with a changed server ID
      refused
- [ ] a worker's watch tab shows its commands and output live, and Stop and Disconnect there
      end its work
- [ ] a worker cannot send keystrokes to the owner's terminal

## Acceptance criteria (plan)

- [ ] The normal user experience does not require reading terminal output, editing JSON, or
      memorizing session IDs.
- [ ] Raw diagnostics remain available for troubleshooting.

## Out of scope (plan)

- cosmetic redesigns that delay functionality
- mobile application
- remote multi-user console
- workers typing into the owner's terminal, or running shell lines (pipes, `&&`) through it

## Before pushing

- `pnpm check`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `pnpm bindings`, then no diff in `packages/types/src/generated`
- `pnpm e2e` against the release build (see `docs/development/setup.md`)

## Done when

- [ ] Every box above is ticked, or has a note saying why not
- [ ] Acceptance report in `docs/phases/phase-12-acceptance-report.md`, with screenshots in
      `docs/phases/evidence/phase-12/`
- [ ] Release notes in `docs/releases/v1.8.0.md`; version 1.8.0 everywhere
      (`scripts/check-versions.mjs`)
- [ ] `ROLLOUT_PLAN.md` Phase 12 status line
