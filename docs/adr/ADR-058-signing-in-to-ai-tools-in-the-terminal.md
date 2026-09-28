# ADR-058: Signing in to an AI tool in a terminal tab

- **Status:** Proposed
- **Date:** 2026-09-28
- **Phase:** 19
- **Carries out:** ADR-039 (the owner's notes) §2.6, "signing in to an AI tool from Plenipo"
- **Amends:** ADR-031 (the terminal panel) §2 and §6 — the panel gains sign-in tabs, which run
  one AI tool's own sign-in program from a fixed list instead of a shell

> **On screen** (ADR-010, plain words and rank names): **Sign in**, **Reconnect**, **Sign out**,
> and the tab's name, "**Sign in · Codex**". This record keeps the code's words (login command,
> PTY, place).

## In short

Each AI tool's card gets **Sign in**, **Reconnect**, and **Sign out** buttons. Pressing one opens
a tab in the terminal panel that runs that AI tool's own sign-in program — for example
`codex login` — and nothing else. You finish signing in yourself, in that tab and in your
browser, exactly as you would in PowerShell. Plenipo never sees, keeps, or passes your sign-in,
never types into the tab, and never opens the files where the AI tool keeps it. When the
program ends, Plenipo checks the tool again by itself and the card shows the new state.
Accepting this record means building it as written below.

## Context

Phase 19 of `ROLLOUT_PLAN.md`: "**Sign in / Reconnect / Sign out:** opens a terminal tab that runs
the AI tool's own command; Plenipo re-checks when the tab closes", and "the tab's starting command
comes from a fixed list per AI tool (for example `claude auth login`, `codex login`, `grok login`,
`kimi login`, `ollama signin`). The owner completes the login. Plenipo never reads, stores, or
passes the credential (ADR-007 §4), and never types into the tab (ADR-014 §7; ADR-039 §2.6)."
Its first test: "Sign in opens a terminal tab running exactly that tool's login command, and
nothing else can be started that way."

What the code has today (read at `edcd522`, v1.11.0):

- **Sign-in is typed instructions.** Each AI tool's `login_hint()` is a sentence ("Open a
  terminal, run: codex login — …. Then choose Re-check."), shown on the AI tools page, in Workers,
  and in Settings → AI tools.
- **The terminal panel** (ADR-031) opens a shell on this PC or on a server. Its commands name a
  **place** (`{kind: "thisPc"}` or `{kind: "server", serverId}`), never a program or a path, and
  a place with any other shape is refused (`TerminalPlace`'s strict reader). The shell runs
  through `portable-pty` (`crates/capabilities/src/terminal.rs`, `start_local`), which can start
  any program with arguments — today it is only ever given a shell. The shell gets Plenipo's
  own environment. Typing reaches it only from the tab's screen (`xterm.onData` →
  `write_terminal`). What is typed and shown is never recorded; the Ledger records only that a
  terminal opened and closed (`terminal.opened`, `terminal.closed`).
- **Each AI tool's program is found** by its adapter's rules (`discovery.rs`: PATH and the
  tool's usual folders, real `.exe` files only on Windows) and added to the supervisor's list of
  allowed programs (ADR-005). Its tasks and checks get a cleared environment: Windows' own
  variables, plus the few the adapter names (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GROK_HOME`,
  proxy settings), plus switches that keep it safe (`DISABLE_AUTOUPDATER=1`,
  `GROK_DISABLE_API_KEY_AUTH=1`). API-key variables are never passed.
- **When a terminal's program ends**, the tab shows why and stays open; nothing else is told.

The official sign-in and sign-out commands, checked on each maker's own pages or on the real
program (the Phase 19 checklist has the sources):

| AI tool     | Sign in and Reconnect | Sign out             | Sure it is official?                                                                 |
| ----------- | --------------------- | -------------------- | ------------------------------------------------------------------------------------ |
| Claude Code | `claude auth login`   | `claude auth logout` | Yes — Anthropic's command-line reference                                             |
| Codex       | `codex login`         | `codex logout`       | Yes — OpenAI's command reference                                                     |
| Grok        | `grok login`          | `grok logout`        | Yes — `grok --help` of Grok 1.0.41, recorded in `evidence/ai-tools-grok/help/`       |
| Kimi        | `kimi login`          | none                 | Yes — Kimi Code's command reference and its recorded help list no sign-out command   |
| Ollama      | `ollama signin`       | `ollama signout`     | Yes — `ollama --help` and a recorded sign-out and sign-in on your PC (Ollama 0.34.4) |

Flags that would change what is billed are never used: `claude auth login --console` (API
billing), `codex login --with-api-key`, and `--with-access-token`.

## Decision

### 1. The buttons

- Each AI tool's card shows **Sign in** while the tool is signed out, and **Reconnect** and
  **Sign out** while it is signed in (with a subscription, an API key, or anything else).
  Reconnect runs the same command as Sign in; the AI tool asks whether to sign in again or with
  another account.
- A tool with no sign-out command (Kimi) has no Sign out button. Its card says so in one line.
- The buttons are disabled, with the reason, while the tool is not installed.

### 2. The tab runs one program from a fixed list, and nothing else

- **A new place:** `{ kind: "aiTool", runtimeId, action: "signIn" | "signOut" }`. `runtimeId`
  must be one of the AI tools Plenipo knows (`builtin_adapters()`), and `action` one of the two
  words. Anything else — another tool, another action, a program, a path, arguments, extra
  fields — is refused by the same strict reader as today's places. No command takes a program,
  arguments, an environment, or a folder.
- **The command comes from the adapter,** from a new `account_command(action)` in the
  `RuntimeAdapter` contract that returns fixed arguments, or none (Kimi's sign-out). It is
  written in the adapter, next to the tool's other commands, and covered by the contract suite
  (every adapter's sign-in command is its official one; none contains a billing flag).
- **The program is found by the tool's own rules** (the same `locate` and `resolve` its tasks
  use), must be allowed by the supervisor (ADR-005), and is started directly in the terminal —
  **no shell in between**, so no other command can ride along. For Ollama, the tab runs the real
  `ollama` program, not Plenipo's Ollama helper.
- **The same environment as the tool's tasks:** a cleared environment with Windows' own
  variables and the adapter's names and switches, so the sign-in lands where tasks read it, and
  an API key set on the PC never reaches the sign-in. It adds what a terminal needs
  (`TERM`, `COLORTERM`) and, on Linux, what opens the browser (`DISPLAY`, `WAYLAND_DISPLAY`,
  `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`).
- **It starts in your home folder,** as your own Windows user, never as administrator (as
  ADR-031's terminal on this PC).

### 3. You sign in; Plenipo never sees it

- **You type** in the tab, and finish in your browser when the tool opens it. Typing reaches the
  program only from the tab's screen, as in every terminal tab (ADR-031 §6).
- **Plenipo never types into the tab.** No code path writes to a sign-in tab except your own
  typing: nothing is sent when it opens, and nothing when the program asks a question.
- **Plenipo never reads, stores, or passes the sign-in.** What the program shows goes only to
  your screen, as in any terminal tab: not kept, not logged, not parsed. Plenipo never opens the
  files where an AI tool keeps its sign-in (`~/.claude`, `~/.codex`, `~/.grok`, `~/.kimi-code`,
  `~/.ollama`), then or later (ADR-007 §4; ADR-039 §2.8).
- ADR-014 §7's rule against Plenipo driving an AI tool's interactive screen stays: **you** drive
  it (ADR-039 §2.6).

### 4. When the program ends, Plenipo checks again

- When the sign-in program ends (it finished, you closed the tab, or it failed), Plenipo checks
  that AI tool again **by itself** — its sign-in check, as before every task — and the card
  shows the new state. This does not depend on the window: a reloaded window still gets the
  result.
- If the state changed, the Ledger records `ai_tool.sign_in_changed` with the tool, the old and
  new state, and the kind of sign-in ("Claude subscription (max)") — never an account name or
  email, as ADR-007 §4 requires.
- The tab stays open and shows how it ended, as today; you close it.

### 5. Tasks and the sign-in tab

- **Sign out and Reconnect wait for tasks.** While a task is using that AI tool, the buttons say
  "Waiting: 1 task is using Codex" and the tab opens when it finishes, or you press **Cancel**.
  Signing out mid-task would break the task.
- **New tasks wait while the program runs.** While a sign-in or sign-out program is running,
  Plenipo does not start new tasks on that AI tool; they wait, and start once the check after it
  is done. (The same hold as during an update, ADR-059 §4.)
- Stop all (ADR-020) does not close your sign-in tabs, as it does not close your terminals.

### 6. Guard and the capability broker

- The tab is opened by the capability broker, like every terminal (`broker/terminals.rs`).
  Before it starts, **Guard** checks the request against the fixed list: a known AI tool, a
  known action, a command the adapter has, no task using the tool for Sign out and Reconnect,
  and no worker using the screen (today's rule for terminals). A refusal is recorded with its
  reason.
- Guard still does not check what you type (ADR-031 §3).

### 7. What is recorded

- `terminal.opened` and `terminal.closed`, as for every terminal, with `place: "aiTool"`, the
  tool, and the action, how long it ran, and the program's exit code. The Activity trail reads
  "You opened Codex's sign-in" and "Codex's sign-in closed after 40 s".
- `ai_tool.sign_in_changed` (§4).
- Nothing the program shows or you type, anywhere: not in the Ledger, the log files, or the
  diagnostics file.

### 8. Not in this record

- Signing in for a Connection (Phase 20) or a paid AI key (Phase 16).
- Opening an AI tool's own full screen (for example `kimi`, to type `/logout`): that screen
  runs the AI tool's own tools outside Guard.

## Consequences

- You can sign an AI tool out and back in without leaving Plenipo, and the card is right the
  moment you finish.
- The typed hints ("Open a terminal, run: …") stay as a second way, for Workers and Settings.
- One more kind of terminal tab; the terminal's rule — it names a place, never a program — still
  holds, because the place names a tool and an action from a fixed list.
- The sign-in tab gets a cleared environment, unlike today's shell (which keeps Plenipo's), so a
  proxy or special folder that only the shell has will not reach it; the adapter's own list
  covers what the tools need.
- Tests: a place with another tool, action, program, path, or extra field is refused; the fake
  AI tool records exactly its official sign-in arguments and that nothing was written to it
  before a key was pressed; the card shows the new state after the program ends; Sign out waits
  for a running task; the IPC tests refuse the sign window and web pages.

## Alternatives considered

- **Open a PowerShell tab and type the command for you.** Refused: Plenipo would be typing into
  your terminal (ADR-039 §2.6).
- **Open a PowerShell tab and ask you to type the command.** Works, but it is today's hint with
  an extra step, and the card could not tell when you were done.
- **Sign in inside Plenipo** (a web view, or the AI tool's sign-in link in Plenipo's browser).
  Refused: Plenipo would see your password or the sign-in page (ADR-007 §4).
- **Read the AI tool's saved sign-in to tell which account is signed in.** Refused (ADR-039
  §2.8); the tool's own status command is enough.
- **Use ACP's "sign in with a terminal" method** (Kimi's `initialize` offers `--login`). It is
  the same program; the documented `kimi login` is clearer.
