# ADR-059: Plenipo keeps the AI tools up to date, between tasks

- **Status:** Accepted (by the owner, 2026-09-28, with every choice as recommended)
- **Date:** 2026-09-28
- **Phase:** 19
- **Carries out:** ADR-039 (the owner's notes) §2.7, "Plenipo updates the AI tools"
- **Amends:** ADR-007 (running Claude Code and Codex) §5 — each AI tool's own updater is off
  during tasks, now Codex's too, and Plenipo runs the tool's official update command between
  tasks; ADR-023 (on/off switches) — one new switch

> **On screen** (ADR-010, plain words and rank names): **Version** ("installed" and "checked by
> Plenipo"), **a new version**, **Update**, **Waiting: 1 task is using Grok**, **Updating…**,
> **Updated to 1.0.43**, **The update didn't finish — your old version still works**, and the
> switch **Update AI tools by themselves**. This record keeps the code's words.

## In short

Once a day, Plenipo looks for a new version of each AI tool. When there is one, the tool's card
says so and Windows shows a notice. Press **Update** and Plenipo runs that tool's own official
update command — but only when no task is using the tool; if one is, it waits. Turn on **Update
AI tools by themselves** and Plenipo does it without asking, the same way. After every update
Plenipo checks the tool again: its version, its sign-in, that it still answers the way Plenipo
reads it, and its list of models. If an update fails, your old version keeps working, and the
card says so. Accepting this record means building it as written below.

## Context

Phase 19 of `ROLLOUT_PLAN.md`: "**Version:** installed, and the version Plenipo last checked; a
notice when they differ"; "**Update:** available, updating, updated"; "Plenipo checks each AI tool
for a new version once a day, and updates it only when no task is using it — automatically, or
only when the owner says so (a switch; the default is to ask)"; "Updates use each AI tool's own
official update command or installer. The tool's own self-updater stays off during tasks (ADR-007
§5). After an update: a version check, a sign-in check, a quick check that the tool still answers
in the form Plenipo reads (without running a task), and a model refresh." Tests: "an update never
starts while a task is using that tool; it waits", "a failed update leaves the old version
working and says so", "after an update, the version, sign-in, and models are re-checked".
Out of scope: "letting AI tools update themselves during tasks".

What the code has today (read at `edcd522`, v1.11.0):

- **Versions:** each tool's version is read with `--version` when Plenipo starts and on
  **Re-check**; the version each adapter was checked against (`checked_version()`: Claude Code
  2.1.283, Codex 0.157.1, Grok 1.0.41, Kimi 0.34.0, Ollama 0.34.4) is in the code but never
  shown.
- **Self-updaters:** Claude Code's is off in every task and check (`DISABLE_AUTOUPDATER=1`), and
  Grok's (`GROK_DISABLE_AUTOUPDATER=1`). Codex's startup update check is not switched off. Kimi
  and Ollama have no switch Plenipo sets.
- **Plenipo's own updates** (ADR-038): a thread checks a few minutes after start, then once a
  day; it reaches only GitHub's release addresses, each checked by Guard's gate for Plenipo's
  own requests (`crates/guard/src/outbound.rs`, one purpose today: `Updates`); it installs only
  when you say so, and asks first while work is running.
- **Busy:** Plenipo knows which conversations have a task running, and on which AI tool, but has
  no "is this AI tool in use" question, and no way to make new tasks wait.
- **No Windows service** (ADR-037): Plenipo runs in the tray as you, so it updates only while it
  is running.

The official update and version commands, and where a new version is announced (the Phase 19
checklist has the sources and the owner's checks):

| AI tool     | New version? (once a day)                                                | Update                                                              | How sure it is official                                                            |
| ----------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Claude Code | Anthropic's release list on npm, `@anthropic-ai/claude-code` (read-only) | `claude update`                                                     | Update: yes (Anthropic's setup page). List: yes; its numbers match (2.1.283 both)  |
| Codex       | OpenAI's release list on npm, `@openai/codex` (read-only)                | `codex update`                                                      | Update: yes (OpenAI's command reference), "when the installed release supports it" |
| Grok        | `grok update --check --json` (the tool's own check)                      | `grok update`                                                       | Yes — recorded from Grok 1.0.41                                                    |
| Kimi        | none that matches (npm's list says 2.1.1; your PC reported 0.34.0)       | `kimi upgrade --yes` (checks and installs in one step)              | Update: yes (Kimi Code's command reference)                                        |
| Ollama      | Ollama's release list on GitHub, `ollama/ollama` (read-only)             | none: Ollama's own app downloads updates and asks you to restart it | List: yes. No update command exists                                                |

## Decision

### 1. Two version numbers on each card

- **Installed:** what the tool's own `--version` says, as today.
- **Checked by Plenipo:** the version Plenipo was tested with (`checked_version()`), now shown.
- **When they differ,** the card says so in one line: "This version is newer than the one Plenipo
  was checked with (2.1.283). It should work; if something looks wrong, check for a new version
  of Plenipo." (Older: "…older than … — update it to get every model.")

### 2. Looking for a new version, once a day

- A background job, like Plenipo's own update check: a few minutes after Plenipo starts, then
  once a day. The last time it looked is kept in the Ledger, so a restart does not look again
  sooner. **Check for new versions** on the AI tools page looks now.
- **Where it looks,** per tool (the table above): the tool's own check where it has one (Grok),
  otherwise the AI company's own published release list, read-only, through **Guard's gate for
  Plenipo's own requests** with a new purpose, **AI tool versions**, that allows only these
  addresses, only `https`, every redirect checked:
  - `https://registry.npmjs.org/@anthropic-ai/claude-code/latest`
  - `https://registry.npmjs.org/@openai/codex/latest`
  - `https://api.github.com/repos/ollama/ollama/releases/latest`

  Plenipo reads only the version number. It sends nothing about you or your work, and never
  calls an AI company's unpublished addresses (ADR-039 §2.8).

- **Kimi** has no list that matches its program, so its card says "Press Update to check for a
  new version", and **Update** runs `kimi upgrade --yes`, which installs only if there is one.
  With automatic updates on, Plenipo runs it once a day while Kimi is free.
- **Ollama** is updated by its own app (see §7).
- A new version is recorded once (`ai_tool.update_available`, with the installed and new
  version) and shown on the card; a Windows notice says "A new version of Grok is ready
  (1.0.43)" — once per version, in the **Plenipo** kind of notices.

### 3. Updating: only between tasks, with the tool's own command

- **Only the tool's official update command** from the table, written in its adapter (a new
  `update_command()` in the `RuntimeAdapter` contract), run as an approved program (ADR-005)
  through the supervisor: no shell, standard input closed, the tool's own cleared environment,
  a 10-minute time limit, plus the switch that stops it asking questions
  (`CODEX_NON_INTERACTIVE=1` for Codex; `--yes` for Kimi). Nothing is typed into it.
- **Guard and the capability broker decide first**, as for every program Plenipo runs: the tool
  and its command must be on the fixed list, and **no task may be using the tool**. A refusal is
  recorded with its reason.
- **Never while a task is using it.** If one is, the card says "Waiting: 1 task is using Grok",
  and the update starts when the last one ends. **Cancel** stops the waiting. Waiting ends after
  a day; the next daily look starts again.
- **Updating counts as work going on:** closing Plenipo while an update runs asks first, as it
  does for tasks (ADR-037).
- What the update prints stays in memory, in **Runs** on the AI tools page, like any program
  Plenipo runs (ADR-005 §7); it is never written to the Ledger, the log files, or the
  diagnostics file. If it fails, only its first line, with secrets hidden, is kept as the
  reason.

### 4. Tasks wait during an update

- While a tool is updating, Plenipo **does not start new tasks on it**. A task that would start
  waits ("Waiting: Grok is being updated") and starts on the new version when the update and its
  checks are done — or on the old version, if the update failed and the old one still works.
- This hold also covers a running sign-in or sign-out program (ADR-058 §5).

### 5. After an update, Plenipo checks the tool again

In this order, and the card shows each step:

1. **Version:** `--version` again; the card shows "Updated to 1.0.43".
2. **Sign-in:** the tool's own status command, as before every task.
3. **It still answers the way Plenipo reads it, without running a task:** the version and
   sign-in answers parse as before; for the tools that talk over ACP (Grok, Kimi), an
   `initialize` exchange with no conversation and no prompt; for Codex, its app server's
   `initialize` (ADR-060); for Ollama, the service's version and model list. No model is asked
   anything, so no usage is spent.
4. **Models:** the tool's list of models again (ADR-060).

The Ledger records `ai_tool.updated` with the old and new version, or `ai_tool.update_failed`
with the reason.

### 6. A failed update leaves the old version working

- **The tools' own updaters keep the old version until the new one is in place:** Claude Code
  keeps every version in its own folder and switches when one is ready; Grok and Kimi download
  and check the new program before swapping it in; Codex's standalone installer keeps a package
  cache. Plenipo does not touch the tools' folders.
- **If the update command fails** (an error, or the time limit), Plenipo checks the tool (§5).
  When the old version still answers: "The update didn't finish. Your old version (1.0.41) still
  works." Tasks go on with it.
- **If the tool does not answer after the update** — the command failed half-way, or the new
  version does not answer the way Plenipo reads it — Plenipo **puts the old version back with
  the tool's own command** where it has one: `claude install <old version>` (Claude Code) and
  `grok update --version <old version>` (Grok). Then it checks again (§5).
- **Where there is no such command** (Codex, Kimi), Plenipo stops giving that tool tasks, the
  Router uses your other choices (as for a signed-out tool), and the card says exactly what is
  wrong and the tool's own reinstall command. The Ledger records it.

### 7. Ollama updates itself, with your click

Ollama has no update command. Its own app, in the tray, downloads new versions and asks you to
restart it. Plenipo shows Ollama's installed version, the newest version on Ollama's release
list, and "Update it from Ollama's icon in the tray". Plenipo does not download or run Ollama's
installer.

### 8. The switch: ask first, or update by themselves

- **Update AI tools by themselves** — off by default. It is on the AI tools page and in Settings
  → Switches (ADR-023); both change the same setting, kept in the Ledger. Turning it on or off is
  recorded (`ai_tools.auto_update_switched`).
- **Off (the default):** Plenipo only tells you (the card and a Windows notice). Nothing changes
  until you press **Update**.
- **On:** when Plenipo finds a new version, it updates the tool as soon as no task is using it,
  exactly as in §3–§6, and a Windows notice says "Grok was updated to 1.0.43" (or that it
  didn't finish). Plenipo updates only while it is running (it lives in the tray, ADR-037).

### 9. Each tool's own updater stays off during tasks

As ADR-007 §5, now for every tool that has a switch: `DISABLE_AUTOUPDATER=1` (Claude Code) and
`GROK_DISABLE_AUTOUPDATER=1` (Grok), as today, and `-c check_for_update_on_startup=false` for
Codex's tasks and checks (Codex's documented setting). Kimi and Ollama have no such switch;
whether Kimi updates itself in the middle of a task is on the owner's list to check.

### 10. Where updates cannot go on their own

When a tool says it cannot update itself — Claude Code installed with WinGet, or Codex installed
with npm — the card shows the tool's own words and the official command for that kind of install
(`winget upgrade Anthropic.ClaudeCode`; `npm install -g @openai/codex`), with a button that
opens a terminal on this PC where **you** type it. Plenipo does not run npm or WinGet itself.

## Consequences

- AI tools stay current without leaving Plenipo, and never change under a running task.
- Plenipo reaches two more addresses on the internet (npm's and GitHub's release lists), for
  version numbers only, through Guard's gate; each can be refused and the refusal is recorded.
- A tool can be out of date for up to a day, or longer if tasks keep it busy.
- An update the tool cannot do by itself (npm, WinGet) still needs you, once.
- New tasks can wait a few minutes while a tool updates.
- Tests: the fake AI tool gains an update command (new version, failure, a new version that
  does not answer, slow); an update waits for a running task and starts when it ends; a task
  that starts during an update waits and then runs; a failed update leaves the old version and
  says so; the put-back command runs when the new version does not answer; the version,
  sign-in, and models are checked after every update; the switch's default is off; Guard refuses
  any other address.

## Alternatives considered

- **Let each AI tool update itself.** Refused (ADR-039 §2.7): a tool could replace itself in the
  middle of a task, and its output could change under Plenipo's reader.
- **Update in a terminal tab you watch.** Not needed: the card shows each step, and the program's
  words are in Runs. A tab would also invite typing into it.
- **Keep a copy of each tool's program folder and put it back on failure.** Refused: it writes
  into the AI tools' own folders (Ollama's is gigabytes), and each tool's updater already keeps
  the old version until the new one is ready.
- **Plenipo runs npm or WinGet itself.** Refused for now: npm runs a package's own scripts, which
  ADR-034 (approved programs run as the owner) treats with care, and WinGet updates are rare.
- **Download and run Ollama's installer.** Possible (Ollama documents it), but Ollama's own app
  already does this with your click, and its installer has no documented quiet mode.
- **Check for new versions only when you press a button.** Simpler, but the plan asks for a
  daily check, and automatic updates need one.
- **Read Kimi's version from npm.** Refused: npm's numbers (2.1.1) do not match the program on
  your PC (0.34.0), so Plenipo could say "new version" when there is none.

## As built (v1.12.0)

Built as written, with these details:

- **Where each tool's newest version comes from:** Grok's own `grok update --check --json` (its
  `latestVersion`); npm's list for Claude Code and Codex; GitHub's for Ollama; none for Kimi
  (its **Update** runs `kimi upgrade --yes`, which installs only when there is a newer one). The
  lists are read through Guard's gate with the new purpose **AI tool versions**, which allows
  exactly the three addresses above, only `https`, with no query, and at most 512 KB. Copies
  built for the tests can read a stand-in on this computer instead
  (`PLENIPO_AI_TOOL_RELEASES`, set when Plenipo is built; the released app has none).
- **The update commands:** `claude update`, `codex update` (with `CODEX_NON_INTERACTIVE=1`),
  `grok update`, and `kimi upgrade --yes`; Ollama has none. The put-back commands are
  `claude install <version>` and `grok update --version <version>`. Each runs through
  `programs::run` as an approved program, so it shows in **Runs**, with a 10-minute time limit.
- **Waiting** checks every 2 seconds whether the tool is free, for at most a day; **Cancel**
  stops it. Closing Plenipo while an update waits or runs asks first.
- **A task that would start during an update** waits until the update and its checks are done,
  however long that takes; each step has its own time limit, so the wait ends. (A sign-in tab
  holds tasks for at most ten minutes, ADR-058.) **Stop** works on a task while it waits. A limit
  of the design: a waiting task counts toward the tasks Plenipo runs at once, so while one AI tool
  updates (at most about 20 minutes, with a put-back), fewer tasks can start on the others.
- **One at a time:** an update waits while the tool's sign-in tab is open and while one of
  Plenipo's own short checks is running it; those checks skip a tool that is updating (the
  update checks it itself). A Cancel is either in time — nothing runs — or refused, because the
  update already runs; Update can be pressed again at once.
- **Nothing to update:** Update is refused for a tool that is not installed, and an update whose
  command never started changes nothing (the tool is never marked "not given tasks" for it).
- **The first look** is 4 minutes after Plenipo starts, then Plenipo looks each hour whether a
  day has passed since the last look (kept in the Ledger).
- **"Up to date" or by hand:** when the update command succeeds but the version does not change
  while a newer one is known, the card shows the tool's own command to type (Claude Code with
  WinGet, Codex with npm) and **Open a terminal**. It is recorded (`ai_tool.update_by_hand`);
  when it happened by itself, a Windows notice says what to type, and Plenipo does not try that
  version again by itself.
- **What is kept of an update's output:** its last line that says anything (usually the error),
  with secrets hidden, at most 200 characters. The secret filter now also knows xAI's keys and
  npm's tokens.
- **Given no tasks:** when a new version does not answer and there is no put-back command, the
  AI tool is marked "not given tasks for now" (kept in the Ledger); every place that picks an
  AI tool says so, and **Check again** gives it tasks again once it answers.
- **Notices:** `ai_tool.update_available` (once per version), `ai_tool.updated`, and
  `ai_tool.update_failed` come as Windows notices of the **Plenipo** kind; a new version is not
  announced when the switch is on, since Plenipo updates it by itself — except Ollama's, which
  Plenipo cannot update.
- **Codex's own update check is off** in its tasks and checks
  (`-c check_for_update_on_startup=false`).
