# Phase 12 — Implementation Checklist

**Status:** not started. Built on v1.7.0 (Phase 12A, the design system).

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
