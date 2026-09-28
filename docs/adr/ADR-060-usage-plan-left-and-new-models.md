# ADR-060: Usage, "plan left", and new models — only from what the AI tools officially report

- **Status:** Accepted (by the owner, 2026-09-28, with every choice as recommended)
- **Date:** 2026-09-28
- **Phase:** 19
- **Carries out:** ADR-039 (the owner's notes) §2.7 (models after an update) and §2.8 ("usage
  only from official sources")
- **Amends:** ADR-007 (running Claude Code and Codex) §1 and §4 — Codex is also started as its
  **app server**, for a short status check with no task; ADR-014 (adding AI tools) §6 — the
  models an AI tool reports are shown beside the ones Plenipo checked, marked "new — not checked
  yet"

> **On screen** (ADR-010, plain words and rank names): **Usage**, **tokens** (pieces of words),
> **read** and **written**, **today**, **this week**, **last week**, **usage limit**, **resets
> at**, **left of your plan**, **reported by Codex at 1:05 PM**, **How it is paid for:
> Subscription**, **Paid AI key**, and **new — not checked yet**. This record keeps the code's
> words (tokens, rate limit, app server).

## In short

Each AI tool's card shows how much it has been used — tokens (pieces of words) read and written,
today, this week, and last week, for each model — added up from what Plenipo already saved for
every task. It shows the tool's usage limit and when it resets. Where the AI tool itself reports
how much of your plan is left — Claude Code in the messages Plenipo already reads during a task,
and Codex through its official app server — the card shows that too, with the time it was
reported. Grok, Kimi, and Ollama do not report it, so their cards say so. The card also lists the
tool's models; a model the tool offers that Plenipo has not checked yet shows as **new — not
checked yet** and can still be chosen. "How it is paid for" says **Subscription**; the switch to a
paid AI key is shown, but it cannot be turned on until spending caps exist (Phase 16). Accepting
this record means building it as written below.

## Context

Phase 19 of `ROLLOUT_PLAN.md`: "**Usage:** totals of tokens by day and week, by model; the current
usage limit and its reset time; and 'plan left' only where the tool reports it officially";
"**How it is paid for:** 'Subscription' today. The switch to a paid key is shown here and works
when Phase 16's spending caps exist"; "**Models:** the tool's models, with new ones marked 'new —
not checked yet'"; "the usage limits move from Settings → AI models to this page (Settings links to
it)". Technical notes: "**New models** come from the AI tool's own list where it has one (for
example `grok models`, Kimi's and Gemini's ACP `initialize` answer, Ollama's `/api/tags`). A tool
without a list gets its models with Plenipo's own updates (Phase 13). New models are offered as
'new — not checked yet' (ADR-014 §6)"; "**Usage** adds up the token counts already saved per turn.
'Plan left' comes only from an official command or protocol. Plenipo never reads an AI tool's
saved sign-in or calls its unpublished web addresses (ADR-039 §2.8)."

What the code has today (read at `edcd522`, v1.11.0):

- **Token counts** are saved for every task step in the Ledger (`executions.usage_metadata`:
  tokens read, of which reused, and written), and with each `agent.result`. Nothing adds them up
  except one task's steps on its own page. Kimi reports no token counts.
- **Usage limits** are worked out from the Ledger's recent task results (`router/limits.rs`): a
  tool whose last result was "usage limit" is held until its reset time (Claude Code writes one)
  or for an hour, and **Try again now** clears it. They are shown in Settings → AI models, in the
  "AI tools" table, with "Reached — resets in 2 h".
- **Nothing reports how much of a plan is left.** Claude Code sends a `rate_limit_event` in the
  stream Plenipo already reads; Plenipo ignores it.
- **Models** come only from each adapter's fixed list (`capabilities().known_models`), checked
  against one version. Model pickers offer them, and "Type another name…" for anything else.
  `grok models` is already run before every Grok task, as its sign-in check, and lists Grok's
  models; only its first line is read.

What each AI tool officially reports (sources in the Phase 19 checklist):

| AI tool     | "Plan left"                                                                                                                                                    | Its list of models                                                                           |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Claude Code | `rate_limit_event` in its task stream: `utilization` (share used), `resetsAt`, `status` — documented as `SDKRateLimitEvent` in Anthropic's Agent SDK reference | none from the command line; the plan says: with Plenipo's own updates                        |
| Codex       | its app server's `account/rateLimits/read`: each window's `usedPercent`, `windowDurationMins`, `resetsAt` — documented on OpenAI's app server page             | its app server's `model/list` (with each model's effort levels)                              |
| Grok        | none                                                                                                                                                           | `grok models` (already run), and each model's effort levels from ACP `initialize` (recorded) |
| Kimi        | none                                                                                                                                                           | ACP `session/new`'s model choices (recorded on Kimi 0.34.0)                                  |
| Ollama      | none (only the plan's name, as today)                                                                                                                          | the Ollama service's `/api/tags` (Ollama's documented API; recorded)                         |

## Decision

### 1. Usage: added up from what Plenipo already saved

- **What is added up:** each task step's saved tokens for that AI tool, grouped by **model**
  (the model the tool reported for the step, else the one asked for, else "the AI tool's
  default") and by **day** in your PC's time. Weeks run Monday to Sunday.
- **On the card:** for each model, tokens **read** (with how many were reused) and **written**,
  and the number of tasks, for **today**, **this week**, and **last week**; and a list of the
  last 14 days. Kimi's card says "Kimi doesn't report token counts" and counts tasks only.
- **One new read-only query in the Ledger** (sums over `executions` for one AI tool and a set of
  day boundaries the window sends, at most 60 days). No new table and no new Ledger layout.
- A test adds up the same saved turns by hand and compares.

### 2. The usage limit and its reset time move to the card

- The card shows "Usage limit reached — resets at 3:10 PM (in 2 h)" from today's usage-limit
  memory, with **Try again now** (the same command as today).
- **Settings → AI models** loses its AI tools table and gains a line: "Usage limits, sign-in, and
  updates are on the AI tools page", with a link to it. What a usage limit does (wait, or use
  the role's next choice) stays in Settings → AI models: it is a rule for roles.

### 3. "Left of your plan": only where the tool reports it

- **Claude Code:** Plenipo reads `rate_limit_event` from the task stream it already reads (no new
  program, no new request). It keeps the latest report per tool: the share used, the reset time,
  and whether it was allowed, a warning, or refused. The card shows "91% of your plan left ·
  resets at 3:10 PM · reported by Claude Code at 1:05 PM". When `utilization` is missing (Anthropic
  marks it optional), the card shows only what was reported. Plenipo never computes a share
  itself.
- **Codex:** Plenipo starts Codex's **app server** (`codex app-server`, over standard input and
  output) for a short check, with no conversation and no task: `initialize`, then
  `account/read` (only the kind of sign-in and the plan's name are kept — never an email),
  `account/rateLimits/read`, and `model/list`; then it closes. It runs with Codex's own cleared
  environment, through the supervisor, with a 20-second time limit, when the card is checked,
  once a day with the version check, and after each Codex task (at most once every 5 minutes).
  The card shows each window it reports ("5-hour limit: 75% left, resets at 3:10 PM · Weekly
  limit: 60% left, resets Fri 9:00 AM").
- **Grok, Kimi, Ollama:** the card says "Grok doesn't report how much of your plan is left."
- **Never:** reading an AI tool's saved sign-in or session files, calling an AI company's web
  addresses, or scraping its website (ADR-039 §2.8).
- The latest report is kept in the Ledger's settings (not an Activity entry per task), so it
  survives a restart and shows its time.

### 4. How it is paid for

- The card says **How it is paid for: Subscription** (with the plan's name when the tool
  reports it, as today: "Claude subscription (max)").
- **A switch, "Paid AI key (pay per use)", is shown, off and disabled,** with "Comes with
  spending caps in a later version." The desktop command behind it accepts only
  `subscription` and refuses `paidKey` with that sentence, so nothing can turn it on before
  Phase 16 (ADR-036 §2.2: no key works before spending caps).

### 5. Models: the ones Plenipo checked, and the ones the tool reports

- **Where each tool's list comes from** (the table above): Codex's app server; `grok models` and
  Grok's ACP `initialize`; Kimi's ACP `session/new` (in Plenipo's empty check folder, with no
  prompt: Kimi keeps an empty conversation in its own history, so Plenipo asks only after an
  update and when you press **Check again**); Ollama's `/api/tags` through Plenipo's Ollama
  helper. **Claude Code** reports no list from the command line, so its models come with
  Plenipo's own updates (Phase 13), as the plan says.
- **When:** when Plenipo starts, once a day with the version check, after every update, and on
  **Check again**.
- **What is shown:** the models Plenipo checked (the adapter's list), and, beside them, any model
  the tool reports that Plenipo has not checked, marked **new — not checked yet**. A checked
  model the tool no longer lists is marked **not offered by this version**.
- **They can be chosen:** new models appear in every model menu under the tool's models, with
  the same mark, and the Router uses them like a typed model name (ADR-014 §6). Plenipo never
  adds them to your list by itself (ADR-011). Kimi's rule stays: only `kimi-code/` models.
- A change to a tool's list is recorded once (`ai_tool.models_changed`, with the names added and
  removed).

## Consequences

- You can see this week's usage for each model, the limit and when it resets, and — for Claude
  Code and Codex — how much of your plan is left, without leaving Plenipo.
- "Plan left" for Claude Code is as fresh as its last task; the card says when it was reported.
- Codex runs one more short, task-free program (its app server) from time to time. It is
  OpenAI's documented way for apps to read an account's limits and models.
- New models reach the menus between Plenipo updates, clearly marked, and the owner decides
  whether to use them.
- Tests: usage totals match the saved turns; the limit and reset time show on the card; a fake
  Claude Code `rate_limit_event` and a fake Codex app server's limits show as "left of your
  plan", and the other tools say they don't report it; a model the fake tool reports shows as
  "new — not checked yet" and can be chosen; the paid-key switch cannot be turned on.

## Alternatives considered

- **Work out "plan left" from token counts.** Refused: each AI company counts differently and
  changes its limits; a guess would look official.
- **Read Codex's or Claude Code's own session files for limits.** Refused: they are the tools'
  own files (ADR-039 §2.8).
- **Ask Claude Code for its models through the Agent SDK's start-up message**
  (`supportedModels()`). The message's types are documented, but the exchange itself is the
  SDK's private wiring with the program; the plan chose Plenipo's own updates for a tool
  without a list.
- **`codex debug models` for Codex's models.** A debugging command; the app server's
  `model/list` is the documented one.
- **Hide the paid-key switch until Phase 16.** The plan asks for it to be shown.
