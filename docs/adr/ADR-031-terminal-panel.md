# ADR-031: The terminal panel

- **Status:** Proposed
- **Date:** 2026-09-27
- **Phase:** 12
- **Number:** first written as ADR-030; renumbered when ADR-029 (workers try a CAPTCHA three
  times) reached main first.
- **Amends:** ADR-025 (servers over SSH, through Guard), sections 4 and 11

## Context

The owner asked for a terminal inside Plenipo (owner direction, 2026-09-27, in `ROLLOUT_PLAN.md`
Phase 12, "Terminal panel"): a panel at the bottom or side of the window, like the terminal in
a code editor, that they can show, hide, and resize. It has two kinds of tabs:

- **Your terminal:** the owner types freely, on this PC or on a server from Settings → Servers.
- **Watch tabs:** one per worker using a server, showing each command the worker runs and its
  output as it arrives, with **Stop** and **Disconnect** right there.

ADR-025 left this out: "No terminal for the owner inside Plenipo" (section 11), and "Never asked
of a server: … a terminal, a shell" (section 4). The owner approved the watch tab design. This
record says how the panel works, where it changes ADR-025, and what stays the same.

ADR-025 itself is still **Proposed**. Accepting this record does not accept ADR-025; the owner
can accept both.

## Decision

### 1. The panel

- A panel at the **bottom** of the window by default; the owner can move it to the **right
  side**. It opens and closes from a **Terminal** button in the top bar, and with **Ctrl+`**
  (the key left of 1), as in Visual Studio Code.
- The owner drags its edge to resize it. Plenipo remembers, on this computer, whether it was
  open, its size, and its side, and brings it back the same after a restart.
- **Several tabs at once**, each with a close button. Tabs are built from the Phase 12A design
  system (ADR-030): the same tabs, status marks, and buttons as every other screen.
- **Production servers are red** in their tabs (a red mark and the word PRODUCTION), as in
  Settings → Servers, the approval cards, and the sign.

### 2. Your terminal on this PC

- **New terminal → This PC** opens Windows PowerShell in the owner's home folder, as the owner's
  own Windows user, never as administrator. Settings can pick another shell that is installed:
  PowerShell 7 or Command Prompt.
- Plenipo runs it through Windows' own terminal support (ConPTY, via the `portable-pty` Rust
  library), so colors, arrow keys, and programs like `vim` work.
- The screen part is `xterm.js` (`@xterm/xterm`), the terminal used inside Visual Studio Code.
  Its colors come from the design tokens.

### 3. Your terminal on a server

- **New terminal → a server** lists the servers in Settings → Servers.
- **The pinned server ID is checked first** (ADR-025 section 3). A changed ID is refused before
  Plenipo signs in or sends anything, with the same words as everywhere else ("This server's ID
  changed"), and the tab says so.
- Plenipo signs in with the server's stored sign-in from the Vault. **The key or password is
  never shown**, never sent to the screen, and dropped after use (ADR-025 section 4).
- **Amends ADR-025 section 4:** for the owner's terminal only, Plenipo asks the server for a
  terminal and a shell (`pty-req` and `shell`). Workers still never get either. Agent
  forwarding, X11, and environment variables are still never asked for, by anyone.
- **The owner is in charge:** Guard does not check what the owner types, and there are no
  approval cards for it.
- It uses Plenipo's built-in SSH (ADR-026). Nothing to install.

### 4. What is recorded

- The Ledger records that a terminal opened and closed: where (this PC, or which server), when,
  and how long (`terminal.opened`, `terminal.closed`). It shows in the Activity trail as "You
  opened a terminal on Shop".
- **What the owner types and sees is not recorded.** It can hold passwords typed at a `sudo`
  prompt. (A transcript could be added later as a choice, off by default.)

### 5. Watch tabs

- A tab opens by itself when a worker connects to a server, named for the worker and the server
  ("Operations Engineer · Shop"). It stays open, and readable, after the worker disconnects,
  until the owner closes it.
- It shows each command Guard let through and its output as it arrives, from the same events as
  the Activity trail (`ssh.command_started`, `ssh.output`, `ssh.command_finished`), with secrets
  hidden. Refused commands show as refused.
- **Stop** stops the command running now (TERM, then KILL, as in ADR-025). **Disconnect** ends
  the worker's server work (ADR-025 section 11). Both are recorded, as today.
- It is **read-only**: there is no place to type.

### 6. Workers never type into the owner's terminal

- Workers keep using `ssh_run`, one command at a time through Guard, so every command is still
  checked, asked about when it must be, and recorded. The watch tab only shows what Guard already
  let through.
- The terminal's commands (open, type, resize, close) exist only in the owner's window. They are
  not tools: no AI tool is ever offered them, and the tool relay refuses any tool name it does not
  know. They are not allowed in the sign window, or from any web page.
- Stop all (ADR-020) still stops every worker. It does not close the owner's own terminals.

### 7. The switch

- The owner's terminal on a server works only while **Settings → Switches → Remote computers
  (SSH)** is on, like everything else that reaches a server. The terminal on this PC does not
  depend on it.

### 8. Not in this record

- Workers typing into the owner's terminal, or running shell lines (pipes, `&&`) through
  `ssh_run` (out of scope in the plan).
- Windows servers (Phase 15): the owner's terminal reaches Linux and Unix servers, as Phase 11
  does.
- Copying files to or from a server (SFTP).

## Consequences

- The owner can check a server, or fix something by hand, without leaving Plenipo, and can watch
  a worker's server work as it happens.
- Two new libraries: `portable-pty` (Rust, MIT) and `@xterm/xterm` with its fit add-on
  (JavaScript, MIT).
- A terminal on a production server is a powerful place; the red marks are the only guard,
  because the owner is in charge.
- Tests (Phase 12): the panel opens, hides, resizes, and comes back the same after a restart;
  the owner's terminal on this PC and on the synthetic SSH server, with a changed server ID
  refused; a worker's watch tab shows its commands and output live, and Stop and Disconnect
  there end its work; a worker cannot send keystrokes to the owner's terminal.

## Alternatives considered

- **Record what the owner types:** it can hold passwords. Not by default.
- **Guard checks the owner's commands too:** the owner is in charge, and it would make the
  terminal hard to use. Not chosen.
- **Let workers use a shared terminal:** every command would stop going through Guard one at a
  time. Out of scope in the plan.
- **Open Windows Terminal instead of a panel:** it would not know the pinned server ID, the
  Vault sign-ins, or the workers' tabs.
