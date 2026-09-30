# Phase 16 — Implementation Checklist (Wave 2)

**Status:** built (2026-09-30), v1.15.0; see the [acceptance report](phase-16-wave-2-acceptance-report.md). The checks on the owner's PC are done, and the owner answered
the three choices. Built beside Phase 21 at the owner's direction
([ADR-090 (building Phase 21 alongside Phase 16's second wave)](../adr/ADR-090-phase-21-alongside-phase-16-wave-2.md)).

Source: `ROLLOUT_PLAN.md`, Phase 16 — Every AI Model Worth Having, **Wave 2 only** ("one AI tool,
one decision record each"):

- Cursor's agent (its own models plus Anthropic's, OpenAI's, Google's, xAI's, Moonshot's);
- GitHub Copilot, second try, through its `--headless --stdio` mode.

Records it follows:

- [ADR-014 (adding AI tools)](../adr/ADR-014-adding-ai-tools.md): the bar every AI tool passes,
  and a written finding for one that does not;
- [ADR-007 (how Plenipo runs AI tools)](../adr/ADR-007-runtime-adapters.md): subscription only,
  checked before every task;
- [ADR-036 (every AI model worth having)](../adr/ADR-036-every-ai-model.md): nothing paid before
  spending caps (Wave 3);
- [ADR-081 (who made each model)](../adr/ADR-081-who-made-each-model.md): every listed model says
  who made it, and a model whose maker is not known plays safe;
- [ADR-082 (Antigravity as an AI tool)](../adr/ADR-082-antigravity-as-an-ai-tool.md): a settings
  folder of its own, the task's words wrapped for the tool, and a task stopped if the tool's own
  tools run;
- the [first Copilot finding](ai-tools-copilot-finding.md) and its
  [notes for the next decision record](ai-tools-copilot-decision-notes.md).

New records: **ADR-083 (GitHub Copilot as an AI tool, checked before every task)** and **ADR-084
(Cursor's agent waits: no check a program can run for paid extra use)**.

## Step 0 — checks on the owner's PC (done, 2026-09-30)

Steps were given in chat, one tool at a time, after the build machine had checked both tools
signed out (Copilot 1.0.89, Cursor's agent 2026.09.28-64d2043). Results, with the owner's name
and user folder removed: [evidence/phase-16-wave-2/owner-check](evidence/phase-16-wave-2/owner-check/README.md).

### GitHub Copilot (1.0.89): passes

- [x] Official program, one task, words on standard input, exits by itself: "OK", exit 0.
- [x] JSON lines out: `session.*`, `assistant.message_delta`, `assistant.message`, one `result`
      line with `sessionId`, `exitCode`, and `usage.premiumRequests`.
- [x] A conversation continued by its ID (`--resume`): it remembered "heron".
- [x] **The check before every task**, over `copilot --headless --stdio` (the link GitHub's own
      Copilot SDK uses):
  - `auth.getStatus`: `isAuthenticated: true`, `authType: "user"` (its own sign-in). In an empty
    settings folder it fell back to the GitHub CLI's sign-in: `authType: "gh-cli"`.
  - `account.getQuota`: `chat`, `completions`, and `premium_interactions`, each with
    `overageAllowedWithExhaustedQuota: false`. GitHub's own page agrees: the budget for **All AI
    Credit SKUs** is $0 with **Stop usage: Yes**.
- [x] Its own tools off (`--available-tools` with a name that matches no tool): asked to make a
      file, it only wrote a pretend request as text, and no file was made.
- [x] Its models: the owner's plan offers only **Auto** (Copilot picks); Auto picked Microsoft's
      MAI Code 1.1 Flash. No premium requests are included (it looks like Copilot Free).

### Cursor's agent: a written finding

- [x] On the owner's PC the `agent` command is xAI's Grok program ("grok 1.0.44"), so Cursor's
      own program did not run. The owner: "Grok is cursor."
- [x] Cursor's website shows the **Free** plan, linked to SuperGrok, with **On-Demand Spending:
      Disabled**.
- [x] On the build machine, Cursor's agent (`cursor-agent` 2026.09.28-64d2043, signed out) reads
      the task from standard input, reports JSON lines, and has a sign-in check
      (`status --format json`). But whether paid on-demand use is on shows only on Cursor's website and on its own
      screen (`/usage`); no command, no ACP message, and nothing in a task's output says it. That
      fails ADR-014 bar item 3 the same way Copilot's first try did.

## The owner's choices (2026-09-30)

1. **Cursor:** "as recommended. Grok is cursor." A written finding now (ADR-084); Grok stays the AI
   tool for xAI's models.
2. **Which Copilot sign-in counts:** "accept either" — Copilot's own sign-in (`copilot login`) or
   the GitHub CLI's (`gh auth login`). A token in a variable, a key, or anything else is never
   used.
3. **What Copilot workers do:** "as recommended" — text answers only. All of Copilot's own tools
   stay off, and a task stops if one of them runs anyway.

## Build

### Adapter contract (`crates/runtime/src/agent/adapter.rs`, `discovery.rs`, `service.rs`)

- [x] **A two-way sign-in check** (`RuntimeAdapter::auth_talk`): requests written to the tool's
      standard input, each answered before its input closes (closing early lost answers,
      `evidence/ai-tools-copilot/headless-rpc-close-early.txt`). Default: none, so every other
      tool keeps its status command.
- [x] **Framed messages** (`Framing::Headers`): `Content-Length` headers, the way Copilot's link
      frames JSON-RPC. The existing line-by-line talks (Codex, Grok, Kimi) are unchanged.
- [x] **A settings folder named by the tool's own variable** (`RuntimeAdapter::home_variable`):
      Copilot's folder is `COPILOT_HOME`, pointed at Plenipo's folder for it, so the owner's own
      Copilot settings, hooks, add-ons, and instructions never apply. The owner's home folder
      stays, so the GitHub CLI's sign-in still works.

### Copilot's adapter (`crates/runtime/src/agent/copilot.rs`)

- [x] Identity: **GitHub Copilot**, company **GitHub** (`github`), program `copilot`; Windows:
      WinGet's `copilot.exe`, or the real `copilot.exe` inside the npm package (never the shim).
- [x] The check before every task: `connect`, `auth.getStatus`, `account.getQuota`:
  - signed out → signed out;
  - `user` → "Copilot sign-in"; `gh-cli` → "GitHub CLI sign-in" (the owner's choice 2);
  - `env`, `token`, `api-key`, `hmac` → a key or token, never used;
  - anything else, an error, or no answer → not known, never used;
  - **any allowance with paid extra use on** (`overageAllowedWithExhaustedQuota: true`), or an
    allowance that cannot be read → never ready, with the fix in plain words (GitHub's budget for
    AI Credits at $0 with Stop usage on).
  - The account name is never kept.
- [x] Environment: proxy and certificate settings only; `COPILOT_AUTO_UPDATE=false`; no token
      variable ever (the contract suite refuses them).
- [x] One task: `--output-format json`, `--no-auto-update`, `--available-tools=<no tool>`,
      `--disable-builtin-mcps`, `--no-ask-user`, `--no-custom-instructions`, `--model` when set,
      `--session-id` (Plenipo chooses) or `--resume=<id>`. The words go in on standard input.
- [x] Parser, from the owner's recorded output: text as it is written, the answer, the model Auto
      chose, the session, errors by `errorType` (`quota` and `rate_limit` → usage limit,
      `authentication` → sign in again), a pay-per-use model (`isByok`) → stopped, and one of its
      own tools that runs → stopped (refused ones are shown, and the task goes on).
- [x] Models: none listed; its default (Auto) runs, and who made it is not known, so it plays
      safe (ADR-081 §7). Effort: none.
- [x] The AI tools page: **Sign in** runs `copilot login` in a terminal tab (no sign-out
      command); newest version from GitHub's npm package `@github/copilot` (Guard's list of
      release addresses gains it); **Update** runs `copilot update`; its check reads
      `models.list` and how much of the plan is used from `account.getQuota`.
- [x] Conversation only (`accepts_tools: false`).

### Tests

- [x] Unit tests in `copilot.rs`, from the owner's recorded output.
- [x] A `copilot` persona in `plenipo-fake-agent`: `--headless --stdio` (framed JSON-RPC) for each
      sign-in (`subscription`, `gh-cli`, `api-key`, `signed-out`, `unknown-status`, and
      `paid-extra`), one task, resume, and the markers (`[usage-limit]`, `[auth-expired]`,
      `[own-tool]`, `[refused-tool]`, `[byok]`, …).
- [x] Contract suite: Copilot passes every check with its persona; it still refuses key
      variables for every subscription AI tool; a test runs Copilot the way the app does (its own
      settings folder, the check before each task, paid extra use refused).
- [x] With the paid switch off, nothing changes: a paid key is still refused, and paid extra use
      is never "ready".
- [x] Cross-company review counts Copilot's work by who made the model: its default is not known,
      so a "must be different" review never picks it and work done on it is never counted as a
      different company.
- [x] Desktop IPC tests: the AI tools commands still refuse the sign window and web pages (no new
      command is needed; if one is added, it gets the same tests).
- [x] Vitest for Copilot's card (GitHub sign-in words, no sign-out, who made its models).
- [x] End-to-end in the real app, screenshots in
      [evidence/phase-16-wave-2](evidence/phase-16-wave-2/).

### Paperwork

- [x] [ADR-083](../adr/ADR-083-github-copilot-as-an-ai-tool.md) and
      [ADR-084](../adr/ADR-084-cursor-agent-waits.md), with "As built"; the ADR index.
- [x] [The Cursor finding](ai-tools-cursor-finding.md); the first Copilot finding marked as
      answered.
- [x] Setup guide, the adding-an-AI-tool guide, README, and the screens that list AI tools.
- [x] New word pairs in [the word list](../design/vocabulary.md).
- [x] [Acceptance report](phase-16-wave-2-acceptance-report.md),
      [release notes v1.15.0](../releases/v1.15.0.md), a row in
      [versioning](../development/versioning.md), the plan's Phase 16 line and its order-of-work
      row, and version 1.15.0 (or the next one, if Phase 21 merges first; ADR-090 §6).
- [x] A review across several areas, each finding checked by a second reviewer; each confirmed
      finding fixed with a test, or recorded as a design limit.

## Not in this wave

- Plenipo's own tools for Copilot workers (choice 3). A later step, checked on the owner's PC.
- Copilot models beyond Auto: they are listed when a plan that offers them is checked.
- Cursor's agent: waits for a check a program can run (ADR-084).
- The owner's Windows walk-through from Wave 1, and more Ollama cloud models: their own small
  changes.
