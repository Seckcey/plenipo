# ADR-082: Antigravity as an AI tool — Google's Antigravity CLI, run with a settings folder of its own

- **Status:** Proposed (2026-09-29)
- **Date:** 2026-09-29
- **Phase:** 16, Wave 1 (built beside Phase 20 by
  [ADR-080 (building Phase 16's first wave alongside Phase 20)](ADR-080-phase-16-wave-1-alongside-phase-20.md))
- **Follows:** [ADR-014 (adding AI tools)](ADR-014-adding-ai-tools.md), whose bar Antigravity
  passed on the owner's PC ([evidence](../phases/evidence/phase-16/README.md)), and
  [ADR-081 (who made each model)](ADR-081-who-made-each-model.md) §10, which left Google's AI tool to
  its own record.
- **Adds to:** [ADR-007 (how Plenipo runs AI tools)](ADR-007-runtime-adapters.md) — two
  small parts of the adapter contract (§8), and
  [ADR-058 (signing in to an AI tool in a terminal tab)](ADR-058-signing-in-to-ai-tools-in-the-terminal.md) — a
  tool that signs in when started on its own.

> **On screen** (ADR-010, plain words and rank names): **Antigravity** (the AI tool), **Google**
> (its company), **Google sign-in**, and "Conversation only". Never "agy", "runtime", "provider",
> "harness", or "G1 credits".

## In short

Google's Gemini CLI no longer works with personal Google accounts
([the finding](../phases/ai-tools-gemini-finding.md)). Google's replacement is **Antigravity CLI**
(the `agy` program). It runs Google's Gemini models, and some of Anthropic's and OpenAI's, under
one Google sign-in. It passed the bar for new AI tools on the owner's PC. Plenipo adds it as its
sixth AI tool. Workers on it answer in text only: Plenipo gives Antigravity a settings folder of its
own that turns off its tools and its paid credits, and stops any task where that did not hold.
Accepting this record means adding Antigravity as written below.

## Context

- **The bar (ADR-014).** Step 0 on the owner's Windows PC, Antigravity CLI 1.2.13 (2026-09-29):
  - one task with its words on standard input (`-p=` with `--input-format stream-json`);
  - JSON lines out (`--output-format stream-json`: `init`, `step_update`, `result`);
  - a conversation continued by its ID (`--conversation <ID>`);
  - `agy models` lists fourteen models signed in, and "Please sign in" when not;
  - read-only flags (`--mode plan`, `--sandbox`); asked to write a file, it tried to run a
    program, was refused, and wrote nothing.
- **Its sign-in** is kept in Windows Credential Manager, not in the home folder: pointed at another
  home folder, `agy models` still listed the owner's models.
- **Pay-per-use risks.** A Gemini API key needs a key variable (`GEMINI_API_KEY`) **and** a setting
  (`"modelProvider": "gemini"`). Its settings also hold `useG1Credits`, which lets it spend paid "AI
  credits" when the plan's quota runs out. On the owner's account, AI credits are not enabled and
  the setting is off; ADR-036 §2 says nothing paid before spending caps.
- **Found on the build machine** (signed out, 1.2.13):
  - it reads its settings from `<home>/.gemini/antigravity-cli/settings.json`, and rewrites the
    file after reading it, dropping what it does not know;
  - permission rules it keeps: `command(*)`, `read_url(*)`, `read_file(*)`, `write_file(*)`,
    `mcp(*)` (not `url(*)` or `file(*)`);
  - `useG1Credits: false` is its default (a rewrite leaves it out), `true` is kept;
  - `AGY_CLI_DISABLE_AUTO_UPDATE=true` stops its background self-updates, and `agy update` still
    works with it set, with standard input closed and no sign-in;
  - a settings file it cannot read makes it fall back to its defaults (`request-review`
    permissions);
  - with a Gemini API key, `agy models` lists only Gemini's models; signed in to Google, it also
    lists Anthropic's and OpenAI's.

## Decision

### 1. The AI tool

- **Antigravity**, company **Google** (`google`), program `agy`. Plenipo looks for it on the PATH
  and where Google's installer puts it (`%LOCALAPPDATA%\agy\bin\agy.exe`; `~/.local/bin/agy`
  elsewhere). It runs only the native program.
- It comes after Ollama in the AI tools' order.

### 2. Sign-in and billing

- **The sign-in check is `agy models`,** before every task:
  - it lists models, including another company's → **signed in with Google** (a subscription);
  - it lists only Gemini's models → **an API key** (that is the list a Gemini API key gives), never
    used;
  - "sign in" / "log in" → **signed out**;
  - anything else → **not known**, never used.
- Its task stream never names the sign-in, so only this check lets a task run (as Grok and Codex).
- Plenipo never passes `GEMINI_API_KEY` or any key variable (the contract suite refuses them), and
  never writes `modelProvider`.

### 3. One task

- `agy -p= --input-format stream-json --output-format stream-json --mode plan --sandbox`, plus
  `--model <name>` and `--conversation <ID>` when set. The task's words go in on standard input as
  one JSON message (`{"event":"user","message":{"role":"user","content":"…"}}`), never on the
  command line.
- The answer streams as it is written; the `result` gives the answer, the conversation ID, the
  token counts, and any tools it was refused. Errors are sorted like every tool's (usage limit,
  sign-in, network).
- Stopping a task ends its process, as for the other tools that are not ACP tools.

### 4. Least privilege: a settings folder of its own

- **A home folder of its own.** Every process of Antigravity (the check, tasks, sign-in, update)
  runs with its home folder (`USERPROFILE` on Windows) pointed at a folder Plenipo keeps in its app
  data (`runtime\ai-tool-homes\antigravity`). So none of the owner's own Antigravity settings,
  hooks, add-ons, or conversations apply. The owner's Google sign-in still works (it is in
  Credential Manager).
- **Plenipo's settings,** written again before every run:
  - `"useG1Credits": false` — paid AI credits off;
  - `"toolPermission": "strict"`;
  - `permissions.deny`: `command(*)`, `read_url(*)`, `read_file(*)`, `write_file(*)`, `mcp(*)` —
    programs, web pages, reading and writing files, add-on tools.
- **Checked in every task:**
  - its `init` must report `strict` permissions — the sign that it read Plenipo's settings.
    Anything else stops the task before it begins;
  - if one of its own tools **finishes** (other than finishing, waiting, or asking), Plenipo stops
    the task. A tool it was refused is shown and the task goes on.
- In one-task mode Antigravity also refuses, by itself, any tool that needs permission.
- **Workers on Antigravity are conversation only** (`accepts_tools: false`): no Plenipo tool
  server. Offering Plenipo's tools through its add-on tools (`mcp`) is a later step.
- **Self-updates off in tasks:** `AGY_CLI_DISABLE_AUTO_UPDATE=true` (ADR-059: Plenipo updates AI
  tools between tasks).

### 5. Models, makers, and effort

- **The fourteen models** `agy models` listed on the owner's PC (1.2.13), in its order: eleven of
  Google's (Gemini 3.8 / 3.7 / 3.6 Flash at high, medium, low; Gemini 3.1 Pro at high, low), two of
  Anthropic's (Claude Sonnet 4.6 and Opus 4.6, Thinking), and one of OpenAI's (GPT-OSS 120B).
- **It runs other companies' models** (ADR-081 §1). **Who made its default is not known**: nothing
  it reports says which model runs when none is named, so a review that needs a different company
  plays safe with it (ADR-081 §7).
- **No effort setting.** Each model's name carries its thinking level; Plenipo never passes
  `--effort`.

### 6. The AI tools page (ADR-058 to ADR-060)

- **Sign in** opens a terminal tab that runs `agy` on its own. That is Google's documented way to
  sign in ("Launch the CLI without arguments to sign in"). You sign in, then type `/exit`.
  Antigravity has no sign-out command.
- **Update** runs `agy update`, which checks and installs in one step (it has no check-only form),
  so the page does not show a newer version ahead of time.
- **Models:** its check is `agy models` (no conversation, no usage). A model it lists that Plenipo
  has not checked shows as "new, not checked yet", and who made it is not known.
- **Plan left:** Antigravity does not report it where Plenipo can read it.

### 7. Adding to the adapter contract

- **`RuntimeAdapter::own_home`**: the settings files an AI tool reads from its home folder. Not
  empty: the tool gets the home folder of its own described in §4. Default: none, so every other
  tool is unchanged.
- **`TurnParser::input`**: all of standard input for a task that does not talk. Default: the
  prompt itself; Antigravity wraps it in its one JSON message.
- **A sign-in command with no words**: the tool started on its own. The contract suite allows it for
  Antigravity only; signing out always has words.

### 8. Checked by the owner at acceptance

On the owner's PC, through Plenipo: one task, the same conversation again, a cancel, a model named,
and a refused sign-in (signed out). Also: ask it to search the web. Antigravity's web search runs on
Google's side and is not one of the permission kinds above; if it runs, Plenipo stops the task
(§4). The result is recorded in the acceptance report.

## Consequences

- The owner can use Gemini models through a Google sign-in, and two more of Anthropic's and one
  more of OpenAI's, without an API key.
- Antigravity workers cannot touch files, programs, or the web; a role that needs those should use
  another AI tool until its tools are wired in (ADR-015 style).
- Its own settings folder is Plenipo's: what the owner sets inside Antigravity when run from their
  own terminal does not reach Plenipo's tasks, and the other way round.
- If a later Antigravity version drops a setting or renames a permission kind, a task either stops
  at `init` (settings not read) or at the first tool that runs; nothing is silently allowed.
- Signing out has to be done in Antigravity itself (or by signing out of Google in Credential
  Manager).

## Alternatives considered

- **Gemini CLI.** Google no longer serves personal accounts; it failed the bar (the finding).
- **Using the owner's own Antigravity settings file.** Plenipo would have to change the owner's file
  and could not be sure it stays so; the owner's hooks and add-ons would run in tasks.
- **`excludeTools` in settings.** Accepted by the file but had no effect on 1.2.13.
- **`--dangerously-skip-permissions`.** The opposite of least privilege; never.
- **Wiring Plenipo's tools in now (allow rules for `mcp(plenipo…)`).** Not checked on the owner's
  PC; a later step.
- **Setting a default model so its maker is known.** It would choose a model the owner did not; the
  owner can name one for a position.

## As built

Built on 2026-09-29 as decided:

- `crates/runtime/src/agent/antigravity.rs` — the adapter, its parser, and its unit tests (from the
  owner's recorded output: a task, a resume, the E9 refused tool, the sign-in list).
- `RuntimeAdapter::own_home` and `TurnParser::input` (`adapter.rs`); the runner writes the settings
  and points the home folder there for every process of the tool (`service.rs`,
  `AgentConfig::tool_homes`). A home folder chosen by the tests' own settings is used instead, so
  every test harness keeps working.
- The `agy` persona in `plenipo-fake-agent` reads Plenipo's settings from its home folder and
  reports strict permissions only when they say so; `[own-tool]`, `[refused-tool]`, and
  `[settings]` exercise §4.
- Contract suite: Antigravity passes every check. A new test runs it the way the app does, with
  its own home folder: the settings are written again before each run, a task sees them, a refused
  tool lets it go on, a finished one stops it, and the owner's home folder is never touched.
