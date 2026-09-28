# Phase 19 — Implementation Checklist

**Status:** design approved by the owner (2026-09-28); being built. Builds on v1.11.0 (Phase 18);
releases as **v1.12.0**.

Source: `ROLLOUT_PLAN.md`, Phase 19 — The AI Tools Page: Sign-in, Usage, and Updates (fourth in
the order of work, ADR-039), and the three records written for it:

- [ADR-058 (signing in to an AI tool in a terminal tab)](../adr/ADR-058-signing-in-to-ai-tools-in-the-terminal.md)
- [ADR-059 (Plenipo keeps the AI tools up to date, between tasks)](../adr/ADR-059-plenipo-updates-the-ai-tools.md)
- [ADR-060 (usage, "plan left", and new models — only from what the AI tools officially report)](../adr/ADR-060-usage-plan-left-and-new-models.md)

**Numbers:** ADR-058 to ADR-060. `main` already has ADR-057 (web addresses in the record keep the
page and the names of its fields, accepted 2026-09-28), so the next free number is 058. No new
Ledger layout (it stays at 11): the new totals are read from the tables Plenipo already has, and
the new choices and reports are kept in the Ledger's settings.

Dates are Pacific time.

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): "login" is **Sign in**, a "CLI version" is
the tool's **version**, "rate limit" is **usage limit**, "quota" is **left of your plan**, and
"tokens" are **tokens (pieces of words)**, **read** and **written**.

**Goal (plan):** "Everything about an AI tool in one place: sign in, reconnect, sign out, see its
usage, see how it is paid for, keep it up to date, and see its new models."

## In short, for the owner

The AI tools page becomes the one place for each AI tool. Each tool gets a card with:

- **Sign in / Reconnect / Sign out.** The button opens a tab in the terminal panel that runs the
  tool's own sign-in program, like `codex login`. You sign in yourself. Plenipo never sees it.
  When you finish, the card updates by itself.
- **Usage.** How many tokens (pieces of words) each model read and wrote today, this week, and
  last week. The usage limit and when it resets. And how much of your plan is left — only for
  Claude Code and Codex, because only they report it officially.
- **How it is paid for.** "Subscription". The paid-key switch is there but locked until Phase 16.
- **Version and Update.** The version you have, the version Plenipo was tested with, and a new
  version when there is one. Press **Update**, or turn on **Update AI tools by themselves**.
  Plenipo never updates a tool while a task is using it.
- **Models.** The tool's models. New ones say **new — not checked yet**, and you can still pick
  them.

Usage limits leave Settings → AI models; Settings links to the page instead.

## Owner decisions (2026-09-28)

**The design is approved, and ADR-058 to ADR-060 are accepted, with all eleven choices as
recommended.** Asked what each recommendation was, the owner answered "That all sounds
reasonable". Earlier the owner asked that new records start at ADR-058 ("when you start adding
ADRs, start with ADR 058"), which they do.

The recommendations the owner accepted are marked **Recommended** below.

## Choices for you

Each choice has my recommendation first. Say "as recommended" to take them all, or name the ones
you want different.

1. **Tasks while you sign out or reconnect** (ADR-058 §5).
   - **Recommended:** Sign out and Reconnect wait until no task is using that AI tool, and new
     tasks wait while the sign-in program runs. Nothing breaks halfway.
   - Other: open the tab right away. A task using that tool may fail with "not signed in".
2. **Kimi has no sign-out command** (ADR-058 §1).
   - **Recommended:** no Sign out button for Kimi; its card says Kimi has no sign-out command
     yet. Sign in and Reconnect work.
   - Other: a button that opens Kimi's own full screen in a tab, where you type `/logout`. Not
     recommended: that screen runs Kimi's own tools (files, commands) outside Guard, and I could
     not confirm `/logout` exists.
3. **How Plenipo learns a new version is out, once a day** (ADR-059 §2).
   - **Recommended:** Grok's own check (`grok update --check --json`); for Claude Code, Codex,
     and Ollama, the version number on the AI company's own published release list (npm for
     Claude Code and Codex, GitHub for Ollama), read-only, through Guard's gate, sending nothing
     about you. Kimi: no matching list, so **Update** checks and installs in one step.
   - Other: no daily look for Claude Code, Codex, and Ollama; you press **Update** to find out.
     Automatic updates would then only work for Grok and Kimi.
4. **When automatic updates happen** (ADR-059 §8; only if you turn the switch on).
   - **Recommended:** as soon as a new version is found and no task is using the tool.
   - Other: only at night (1 AM to 5 AM, your PC's time), if Plenipo is running then.
5. **Ollama** (ADR-059 §7).
   - **Recommended:** Plenipo shows Ollama's version and the newest one, and tells you to update
     from Ollama's tray icon. Ollama's own app already downloads updates and asks you to restart
     it.
   - Other: Plenipo downloads Ollama's official installer from ollama.com, checks it is signed
     by Ollama, and runs it between tasks. More moving parts, and the installer has no
     documented quiet mode.
6. **If an update breaks the tool** (ADR-059 §6).
   - **Recommended:** put the old version back with the tool's own command where it has one
     (Claude Code: `claude install <old version>`; Grok: `grok update --version <old version>`).
     For Codex and Kimi, stop giving that tool tasks and show its reinstall command. The tools'
     own updaters already keep the old version until the new one is ready.
   - Other: Plenipo copies each tool's program folder before updating and puts it back on
     failure. Works for all five, but Plenipo would write into the tools' own folders, and
     Ollama's folder is gigabytes.
7. **Codex's "plan left" and its models** (ADR-060 §3, §5).
   - **Recommended:** Codex's official **app server** (OpenAI's documented way for apps to read
     limits and models): a short check, no task, nothing kept but the numbers and the plan's
     name — never your email.
   - Other: no "plan left" for Codex, and its models only with Plenipo's own updates.
8. **Claude Code's models** (ADR-060 §5).
   - **Recommended:** with Plenipo's own updates, as the plan says (Claude Code has no list
     command).
   - Other: ask Claude Code at start-up through the Agent SDK's start-up message. Its types are
     documented, but the message itself is the SDK's own wiring, so it could change without
     notice.
9. **Codex or Claude Code installed in a way that can't update itself** (npm, WinGet)
   (ADR-059 §10).
   - **Recommended:** the card shows the official command for that install
     (`npm install -g @openai/codex`, `winget upgrade Anthropic.ClaudeCode`) and a button that
     opens a terminal where you type it.
   - Other: Plenipo runs npm or WinGet itself. Not recommended: npm runs a package's own
     scripts (ADR-034, approved programs run as the owner).
10. **The switch's place** (ADR-059 §8).
    - **Recommended:** on the AI tools page and in Settings → Switches (the same setting).
    - Other: only on the AI tools page.
11. **"This week"** (ADR-060 §1).
    - **Recommended:** Monday to Sunday, in your PC's time; plus today, last week, and the last
      14 days.
    - Other: the last 7 days, rolling.

Already decided, not a choice: version **1.12.0** (your instruction); the paid-key switch shows,
locked (the plan); Codex's own update check switched off during tasks (ADR-007 §5).

## The commands for each AI tool, and how sure I am

"Yes" means the maker documents it, or it was recorded from the real program. Sources: Anthropic's
[command-line reference](https://code.claude.com/docs/en/cli-reference) and
[setup page](https://code.claude.com/docs/en/setup) (updates, `DISABLE_AUTOUPDATER`), the
[Agent SDK reference](https://code.claude.com/docs/en/agent-sdk/typescript) (`SDKRateLimitEvent`);
OpenAI's [command reference](https://learn.chatgpt.com/docs/developer-commands?surface=cli),
[app server page](https://learn.chatgpt.com/docs/app-server),
[install page](https://learn.chatgpt.com/docs/codex/cli), and
[settings reference](https://learn.chatgpt.com/docs/config-file/config-reference)
(`check_for_update_on_startup`); Grok 1.0.41's own help, recorded in
[`evidence/ai-tools-grok/help/`](evidence/ai-tools-grok/help/); Kimi Code's
[command reference](https://www.kimi.com/code/docs/en/kimi-code-cli/reference/kimi-command.html)
and its help recorded on your PC (`crates/runtime/tests/fixtures/kimi-0.34.0/help/`); Ollama's
[Windows page](https://docs.ollama.com/windows), its help and sign-in recorded on your PC
(`crates/runtime/tests/fixtures/ollama-0.34.4/`), and its API (`/api/tags`).

| AI tool     | Sign in and Reconnect         | Sign out                       | Version (in use today)       | New version?                                                                                                         | Update                                                                                      | Models                                                  | Left of your plan                                                                        |
| ----------- | ----------------------------- | ------------------------------ | ---------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Claude Code | `claude auth login` — **yes** | `claude auth logout` — **yes** | `claude --version` — **yes** | npm `@anthropic-ai/claude-code` — **yes**; its number matches (2.1.283 both)                                         | `claude update` — **yes**; WinGet installs: `winget upgrade Anthropic.ClaudeCode` — **yes** | none: with Plenipo's updates (the plan)                 | `rate_limit_event` in its task stream — **yes** (documented; the share used is optional) |
| Codex       | `codex login` — **yes**       | `codex logout` — **yes**       | `codex --version` — **yes**  | npm `@openai/codex` — **yes** (an official install and update path)                                                  | `codex update` — **yes it exists**; **not sure** it updates an npm install (your check)     | app server `model/list` — **yes**                       | app server `account/rateLimits/read` — **yes**                                           |
| Grok        | `grok login` — **yes**        | `grok logout` — **yes**        | `grok --version` — **yes**   | `grok update --check --json` — **yes** (recorded)                                                                    | `grok update` — **yes**; back: `grok update --version <v>` — **yes**                        | `grok models` and ACP `initialize` — **yes** (recorded) | none                                                                                     |
| Kimi        | `kimi login` — **yes**        | **none exists**                | `kimi --version` — **yes**   | **none that matches** (npm says 2.1.1; your PC reported 0.34.0)                                                      | `kimi upgrade --yes` — **yes**                                                              | ACP `session/new` — **yes** (recorded)                  | none                                                                                     |
| Ollama      | `ollama signin` — **yes**     | `ollama signout` — **yes**     | `ollama --version` — **yes** | GitHub `ollama/ollama` latest release — **yes** (Ollama's own releases; not reachable from my build machine to test) | **no command exists**: Ollama's tray app (choice 5)                                         | `/api/tags` — **yes** (documented, recorded)            | none                                                                                     |

Things I could not confirm from here, for your check on Windows (listed again at the end):
whether `codex update` updates an npm install; whether `GROK_DISABLE_AUTOUPDATER=1` also stops a
`grok update` that Plenipo asks for (it should not); whether Kimi updates itself during a task;
and that Claude Code on your PC sends `utilization` in its `rate_limit_event`.

## Design (2026-09-28)

Written before building, from a map of the code at `edcd522` (`main`, v1.11.0).

### 1. The AI tools page and each card

- **The page** (`views/RuntimesView.tsx`, strip item "AI tools") keeps its heading. At the top:
  **Check for new versions**, **Check again** (today's Re-check), the switch **Update AI tools
  by themselves**, and when it last looked. Then one card per AI tool. Below, **Approved
  programs** and **Runs** stay as they are (an update's run shows there).
- **Each card** (`components/AgentRuntimeCards.tsx`, rebuilt from the design system's panels,
  tabs, pills, and tables): the tool's name, its AI company, and a status pill, then three tabs:
  - **Overview:** sign-in (state and the kind of sign-in) with **Sign in** / **Reconnect** /
    **Sign out**; version ("installed 1.0.41 · checked by Plenipo 1.0.41") and the notice when
    they differ; update state with **Update** / **Cancel**; how it is paid for, with the locked
    paid-key switch; the usage limit and **Try again now**; left of your plan (or "doesn't
    report it"); and this week's tokens in one line.
  - **Usage:** today, this week, and last week by model (read, reused, written, tasks), and the
    last 14 days.
  - **Models:** each model with its effort levels; **new — not checked yet** and **not offered
    by this version** marks; when the list was last asked for.
- **Going straight to a card:** `go({ view: "runtimes", id: "codex" })` scrolls to Codex's card
  (the place keeps the tool's ID). Workers' "Open AI tools" and Settings' links use it.
- **Live:** the page re-reads on `plenipo://agents` updates and on Ledger `ai_tool.*` and
  `agent.result` events, as the models page does now.

### 2. Sign in, Reconnect, Sign out (ADR-058)

- **Adapter contract** (`crates/runtime/src/agent/adapter.rs`): `account_command(action) ->
Option<&'static [&'static str]>`: Claude Code `auth login` / `auth logout`; Codex `login` /
  `logout`; Grok `login` / `logout`; Kimi `login` / none; Ollama `signin` / `signout` (on the
  real `ollama`). The contract suite checks each is present where expected and carries no
  billing flag (`--console`, `--with-api-key`, `--with-access-token`, `--api-key`).
- **A new terminal place** (`crates/capabilities/src/dto.rs`): `TerminalPlace::AiTool {
runtime_id, action }`, read by the same strict reader (`deny_unknown_fields`; `runtime_id`
  must be a known tool; `action` `signIn` or `signOut`). Generated TypeScript type updated.
- **Broker** (`broker/terminals.rs`): `open_ai_tool(runtime_id, action)` asks **Guard**
  (`Guard::check_ai_tool_action`, a fixed table: known tool, known action, the adapter has the
  command, the tool not in use for Sign out and Reconnect, no worker on the screen), locates the
  program by the tool's rules, has the supervisor allow it, and starts it with `start_local`
  directly (no shell). `start_local` gains a cleared-environment option: Windows' own variables,
  the adapter's `runtime_env`, `TERM`, `COLORTERM`, and on Linux what opens a browser. Title
  "Sign in · Codex" / "Sign out · Codex"; working folder: your home folder; refuses
  administrator, as the terminal on this PC does.
- **Tasks:** the desktop layer asks the AI tool service to **hold** the tool while the program
  runs (§5.4) and to check it again when it ends (`AgentRuntime::check(runtime_id)`, a one-tool
  re-check), then records `ai_tool.sign_in_changed` if the state changed. Sign out and Reconnect
  wait while a task uses the tool: the button says "Waiting: 1 task is using Codex", with
  **Cancel**; the tab opens when it is free.
- **Screen:** the card's buttons call `useTerminal().openAiTool(runtimeId, action)`, which adds
  an owner tab with the new place; `OwnerTerminal` is unchanged except for the tab's title and
  an "it ended" callback so the card shows "Checking…". No input element other than the
  terminal itself; nothing is written to it except your keys.

### 3. Usage (ADR-060 §1–§3)

- **Ledger:** `token_usage(runtime_id, day_starts: &[i64]) -> Vec<UsageRow { day, model, tasks,
read, reused, written }>` over `executions` (`runtime`, `started_at`, `model`,
  `usage_metadata`), using the model the tool reported (from the step's `agent.result`) when
  there is one. Read-only; at most 60 days.
- **Limit and reset:** from the Router's snapshot (`ToolInfo.usageLimit`), now also shown as a
  clock time ("resets at 3:10 PM").
- **Left of your plan:**
  - **Claude Code:** the parser reads `rate_limit_event` → `AgentEvent::PlanReport { used,
resets_at, status }` (live only); the service keeps the latest per tool.
  - **Codex:** a new `plan_check` for adapters that have one (Codex only): `codex app-server`
    through the supervisor, with Codex's cleared environment and
    `-c check_for_update_on_startup=false`; JSON-RPC `initialize` → `initialized` →
    `account/read` → `account/rateLimits/read` → `model/list`; then standard input closes. 20
    seconds at most. `account/read` keeps only the kind of sign-in (`chatgpt` or `apiKey`) and
    the plan's name; an API-key answer is shown as "signed in with an API key", as today.
  - Kept in the Ledger setting `ai_tools` (the latest report per tool, with its time), without an
    Activity entry.

### 4. How it is paid for (ADR-060 §4)

- The card's Overview shows "How it is paid for: Subscription (Claude subscription (max))" and a
  disabled switch **Paid AI key (pay per use)** with "Comes with spending caps in a later
  version."
- **Command** `set_ai_tool_payment(runtimeId, method)`: `subscription` is accepted (nothing
  changes); `paidKey` is refused with that sentence. Phase 16 fills it in.

### 5. Version and updates (ADR-059)

1. **Versions:** `AgentRuntimeInfo` gains `checkedVersion` (from `checked_version()`), shown
   beside the installed one, with the notice when they differ.
2. **Adapter contract:** `update_check() -> UpdateCheck` (`Command(&[..])` for Grok's
   `update --check --json`; `Published(address)` for Claude Code, Codex, and Ollama; `None` for
   Kimi); `update_command() -> Option<&[..]>` (`update` for Claude Code, Codex, and Grok;
   `upgrade --yes` for Kimi; none for Ollama); `update_env()` (`CODEX_NON_INTERACTIVE=1` for
   Codex); `put_back_command(version)` (`install <v>` for Claude Code; `update --version <v>` for
   Grok); and `own_update_instructions()` (WinGet and npm words for choice 9). Covered by the
   contract suite.
3. **Guard's gate for Plenipo's own requests** (`crates/guard/src/outbound.rs`): a new purpose,
   `AiToolVersions` ("checking the AI tools for new versions"), allowing only the three
   addresses in ADR-059 §2, `https` only, every redirect checked; refusals recorded as today
   (`guard.request_refused`). The network code reuses `crates/capabilities/src/updates.rs`'s
   client. Tests use a test server on this computer the same way Phase 13's do (only in copies
   built for it).
4. **Busy and hold** (`crates/runtime/src/agent/service.rs`): `AgentRuntime::in_use(runtime_id)
-> Vec<TaskId>` (from the conversations with a running task) and `hold(runtime_id) ->
HoldGuard`. While a hold is on, a task that would start on that tool waits (`Claim::Wait`,
   shown "Waiting: Grok is being updated") and starts when the hold ends.
5. **The update job** (new `crates/runtime/src/agent/upkeep.rs`, wired by a new
   `apps/desktop/src-tauri/src/ai_tools_host.rs`): the daily look (a thread like Plenipo's own
   update check: a few minutes after start, then each day; the last look kept in the `ai_tools`
   setting); `update(runtime_id, by)`: wait until not in use (or Cancel, or a day), hold, record
   `ai_tool.update_started`, run the update command through the supervisor (approved program,
   no shell, standard input closed, 10 minutes), then the checks in ADR-059 §5, then the put-back
   or the stop in §6, then release the hold. Events: `ai_tool.update_available`,
   `ai_tool.update_started`, `ai_tool.updated`, `ai_tool.update_failed`, `ai_tool.put_back`,
   `ai_tools.auto_update_switched`. `work_going` counts a running update.
6. **Self-updaters off during tasks:** Codex's task and check arguments gain
   `-c check_for_update_on_startup=false`.
7. **Notices** (`crates/ledger/src/notices.rs`, the **Plenipo** kind): "A new version of Grok is
   ready (1.0.43)" (once per version, when the switch is off), "Grok was updated to 1.0.43", and
   "Grok's update didn't finish. The old version (1.0.41) still works."
8. **The switch:** kept in the `ai_tools` setting (`autoUpdate`, off by default); shown on the AI
   tools page and in Settings → Switches (`SwitchSettings.tsx`), both calling
   `set_ai_tools_auto_update`.

### 6. Models (ADR-060 §5)

- **Adapter contract:** `model_list() -> ModelSource` (`AppServer` for Codex; `Command(["models"])`
  plus ACP `initialize` for Grok; `AcpSessionNew` for Kimi; `Helper(["models"])` for Ollama, a new
  `--plenipo-ollama models` that reads `/api/tags`; `None` for Claude Code) and a parser for each.
  Grok's list is read from the `grok models` output the sign-in check already has.
- **Kept:** the latest list per tool in the `ai_tools` setting, with its time; a change recorded
  once as `ai_tool.models_changed { runtime, added, removed }`.
- **Router and menus:** `ToolInfo` gains `reportedModels: [{ name, label, effortLevels, checked:
false }]` and `unlisted: [name]`; `routing/format.ts` `modelGroups()` lists new models in the
  tool's group as "name — new, not checked yet"; choosing one works like a typed model name
  (`validate_model`, and Kimi's `kimi-code/` rule).

### 7. Settings

- **Settings → AI models** (`components/models/ModelSettings.tsx`): the "AI tools" table leaves;
  a line and a link take its place ("Usage limits, sign-in, and updates are on the AI tools
  page"); "What a usage limit does" stays. `ModelSettings` gets `go`.
- **Settings → AI tools** (`settings/InfoSettings.tsx`): each row links to its card.
- **Settings → Switches:** the new switch.

### 8. Guard and the capability broker — the owner's rule, for this phase

- **Programs:** every AI tool program Plenipo starts in this phase — a sign-in or sign-out tab, an
  update, a put-back, Grok's update check, Codex's app server, Kimi's `session/new`, Ollama's
  model list — is on a fixed list in its adapter, found by the tool's own rules, allowed by the
  supervisor (ADR-005), and started with the tool's own cleared environment. The sign-in tab,
  updates, and put-backs are decided first by **Guard** through the **capability broker**
  (`Guard::check_ai_tool_action`), which records refusals. The status checks (version, sign-in,
  models, plan left) are the same kind of check Plenipo already runs before every task.
- **Network:** the three release lists go through Guard's gate for Plenipo's own requests (§5.3).
  No other address is reached by Plenipo in this phase.
- **Files:** none. Plenipo does not read or write the AI tools' folders or sign-in files.
- **Browser and screen:** none. The AI tool opens your browser itself during sign-in, as it does
  in PowerShell.

### 9. What is recorded, and what never is

- **Recorded:** `terminal.opened` / `terminal.closed` for sign-in tabs (tool, action, how long,
  exit code); `ai_tool.sign_in_changed` (state and kind of sign-in only); the update events
  (versions, who started it, the first line of a failure with secrets hidden);
  `ai_tool.models_changed`; `ai_tools.auto_update_switched`; Guard's refusals. The Activity trail
  gets plain sentences for each.
- **Never recorded anywhere** (Ledger, log files, diagnostics file): what a sign-in tab shows or
  what you type in it; an update's full output (it stays in memory, in Runs); account names or
  emails; anything from the AI tools' own files.
- **Diagnostics file:** its AI tools list gains the installed and newest version and the update
  state.

### 10. New desktop commands — the main window's alone

`get_ai_tools`, `check_ai_tool`, `check_ai_tool_versions`, `get_ai_tool_usage`, `update_ai_tool`,
`cancel_ai_tool_update`, `set_ai_tools_auto_update`, `set_ai_tool_payment`: 8 in all, plus the new
`aiTool` place for `open_terminal`. Each is added to `build.rs` and `capabilities/default.json`
only (not the sign window's `indicator.json`). IPC tests: each is called from the main window,
and refused from another window, the sign window, and a web page; each refuses bad input with
its reason (an unknown tool, a bad action, `paidKey`, too many days); and `open_terminal` refuses
`aiTool` places naming another tool, another action, a program, a path, arguments, or extra
fields.

### 11. The stand-in AI tool, and tests

- **`plenipo-fake-agent`** gains, for each persona: its sign-in and sign-out programs (print a
  code, wait for a line you type, then write the fake sign-in state and exit; record every byte
  they received, so a test can show nothing was sent before a key); a version kept in its state
  folder; its update command (new version, fails, new version that no longer answers, slow);
  Grok's `update --check --json` and `update --version`; Claude Code's `install <v>`; a Codex
  app server (`initialize`, `account/read`, `account/rateLimits/read`, `model/list`); a Claude
  Code `rate_limit_event` with `utilization`; a new model in `grok models`, Grok's and Kimi's ACP
  answers, and the Ollama helper's model list.
- **Tests:** the plan's eight (below), each at the level that proves it (Rust units, the contract
  suite, the broker and runtime integration tests, the desktop IPC tests, Vitest for the page),
  and **end-to-end tests in the real app** (`tests/e2e/specs/ai-tools.e2e.mjs`) with screenshots
  in `docs/phases/evidence/phase-19/`.

### 12. Words on screen

New pairs for the word list: **Sign in / Reconnect / Sign out** and **sign-in tab** (for "login",
"logout", "re-auth", "login shell"); **Usage**, **tokens read / reused / written**, **today / this
week / last week** (for "token usage", "input/output/cached tokens", "rolling window"); **left of
your plan**, **reported by … at …** (for "quota", "rate-limit utilization", "remaining
allowance"); **resets at** (for "reset timestamp"); **How it is paid for: Subscription / Paid AI
key (pay per use)** (for "billing mode", "BYOK", "metered"); **installed / checked by Plenipo**
(for "CLI version", "tested version", "compatibility"); **a new version / Update / Updating… /
Updated to … / The update didn't finish — your old version still works / put back** (for
"upgrade", "self-update", "rollback"); **Update AI tools by themselves** (for "auto-update");
**new — not checked yet / not offered by this version** (for "discovered model", "unverified
model", "deprecated").

## Deliverables (plan)

- [ ] **On each AI tool's card: Sign in / Reconnect / Sign out** — opens a terminal tab that runs
      the AI tool's own command; Plenipo re-checks when the tab closes.
- [ ] **Usage** — totals of tokens by day and week, by model; the current usage limit and its
      reset time; and "plan left" only where the tool reports it officially.
- [ ] **How it is paid for** — "Subscription" today; the switch to a paid key is shown and works
      when Phase 16's spending caps exist.
- [ ] **Version** — installed, and the version Plenipo last checked; a notice when they differ.
- [ ] **Update** — available, updating, updated.
- [ ] **Models** — the tool's models, with new ones marked "new — not checked yet".
- [ ] The usage limits move from Settings → AI models to this page (Settings links to it).
- [ ] **Updates** — Plenipo checks each AI tool for a new version once a day, and updates it only
      when no task is using it — automatically, or only when the owner says so (a switch; the
      default is to ask).

## Technical implementation (plan)

- [ ] Sign-in in the terminal (ADR-031's panel): the tab's starting command comes from a fixed
      list per AI tool; the owner completes the login; Plenipo never reads, stores, or passes the
      credential, and never types into the tab.
- [ ] Updates use each AI tool's own official update command or installer; the tool's own
      self-updater stays off during tasks; after an update, a version check, a sign-in check, a
      quick check that the tool still answers in the form Plenipo reads (without running a task),
      and a model refresh.
- [ ] New models come from the AI tool's own list where it has one; a tool without a list gets
      its models with Plenipo's own updates; new models are offered as "new — not checked yet".
- [ ] Usage adds up the token counts already saved per turn; "plan left" only from an official
      command or protocol; Plenipo never reads an AI tool's saved sign-in or calls its
      unpublished web addresses.
- [ ] A new decision record for sign-in in the terminal and for Plenipo-run updates (ADR-058,
      ADR-059; and ADR-060 for usage and models).

## Tests (plan)

- [ ] Sign in opens a terminal tab running exactly that tool's login command, and nothing else
      can be started that way.
- [ ] After the tab closes, the card re-checks and shows the new sign-in state.
- [ ] An update never starts while a task is using that tool; it waits.
- [ ] A failed update leaves the old version working and says so.
- [ ] After an update, the version, sign-in, and models are re-checked.
- [ ] A model the tool reports but Plenipo has not checked shows as "new — not checked yet" and
      can be chosen.
- [ ] Usage totals match the saved turns; the limit and reset time show on the card.
- [ ] The payment switch cannot be turned to a paid key before Phase 16.
- [ ] End-to-end tests in the real app, with screenshots in `evidence/phase-19/`.

## Owner's rules for this phase

- [ ] Plain words on screen (the word list gains the new pairs); ADRs named, not just numbered.
- [ ] No passwords, keys, tokens, or secrets asked for in chat; nothing secret committed.
- [ ] Anything touching files, programs, the network, the browser, or the screen goes through
      Guard and the capability broker (§8).
- [ ] New desktop commands are the main window's alone; the sign window and web pages are refused
      (IPC tests).
- [ ] Signing in happens only in a terminal tab that runs the AI tool's own login command, from a
      fixed list; the owner types the sign-in; Plenipo never reads, stores, or passes it, never
      types into that tab, and never reads an AI tool's saved sign-in files.
- [ ] Plenipo never calls an AI tool's unpublished web addresses; "plan left" only when the tool
      reports it through an official command or protocol.
- [ ] Updates use only each tool's own official update command or installer, never while a task
      is using that tool, and ask first unless automatic updates are on; a failed update leaves
      the old version working.
- [ ] The paid-key switch cannot be turned on before Phase 16.
- [ ] Logs and diagnostics files never hold secrets or anything typed in the terminal.
- [ ] No model names in commits, branch names, or pull requests.
- [ ] Version 1.12.0 everywhere, with the Phase 19 row in `docs/development/versioning.md`.
- [ ] Release notes (`docs/releases/v1.12.0.md`), the plan's Phase 19 status line and its state in
      the order of work, this checklist, and the acceptance report with screenshots in
      `evidence/phase-19/`, in Pacific time.
- [ ] A review across several areas, with a second reviewer checking each finding, before the
      final push; each confirmed finding fixed with a test, or recorded as a design limit.
- [ ] Before each push: `pnpm check`, `cargo fmt --all -- --check`,
      `cargo clippy --workspace --all-targets --locked -- -D warnings`,
      `cargo test --workspace --locked`, `pnpm bindings` with no diff (documentation-only pushes:
      `pnpm docs:check`).
- [ ] Every GitHub check green, Windows included.

## Left for the owner (on Windows)

To be listed in the acceptance report: signing Codex out and back in from its card; this week's
usage for Claude Code by model; updating Grok with one click (and, with the switch on, by itself)
while no task uses it, and seeing a model that arrived with the update marked as new; and these
checks on the real programs:

- `codex update` on your Codex install (npm): does it update, or print npm's command?
- `grok update` with `GROK_DISABLE_AUTOUPDATER=1` set: it should still update when asked.
- Whether Kimi updates itself during a task (and `kimi upgrade --help`).
- Claude Code's `rate_limit_event` on your plan: does it carry `utilization`?
- Codex's app server on your Codex version: `account/rateLimits/read` and `model/list` answer.
- The upgrade from 1.11.0 (a backup first; no Ledger layout change).
