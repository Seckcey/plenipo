# Phase 16 — Implementation Checklist (Wave 1)

**Status:** design written (2026-09-29), **waiting for the owner**: choices 1 to 4 below, and the
rest of the checks on the owner's Windows PC ([the steps](phase-16-owner-checks.md)): Part B again
after updating Codex, and Part E (Antigravity CLI). Parts A and C are done; Gemini CLI got a
[finding](ai-tools-gemini-finding.md); choice 5 is answered. Nothing is built yet. Builds on v1.13.0 (Phase 20A); releases as **v1.14.0**. Built beside Phase 20B (Slack and
Google), which another session is building at the same time, at the owner's direction.

Source: `ROLLOUT_PLAN.md`, Phase 16 — Every AI Model Worth Having, **Wave 1 only** ("fits today's
rules, no paid key"), and the records for it:

- [ADR-036 (every AI model worth having)](../adr/ADR-036-every-ai-model.md) — accepted
  2026-09-27; this wave carries out its §3 and the Wave 1 part of §5
- [ADR-080 (building Phase 16's first wave alongside Phase 20)](../adr/ADR-080-phase-16-wave-1-alongside-phase-20.md) —
  **proposed**; the owner's instruction to start now, written down first
- [ADR-081 (who made each model: the maker, cross-company review by maker, and the model list two ways)](../adr/ADR-081-who-made-each-model.md) —
  **proposed**; the design below, with three of the choices
- Read for this design: [ADR-011 (how Plenipo picks each worker's AI model)](../adr/ADR-011-model-policy-routing.md),
  [ADR-014 (adding AI tools)](../adr/ADR-014-adding-ai-tools.md),
  [ADR-015 (running AI tools over ACP)](../adr/ADR-015-acp-ai-tools.md),
  [ADR-017 (Ollama's cloud models)](../adr/ADR-017-ollama-cloud-models.md),
  [ADR-039 (the owner's notes and the order of work)](../adr/ADR-039-owners-notes-order-of-work.md),
  [ADR-060 (usage, plan left, and new models)](../adr/ADR-060-usage-plan-left-and-new-models.md),
  [ADR-061 (doing Connections before new AI models)](../adr/ADR-061-connections-before-new-ai-models.md),
  the [guide for adding an AI tool](../development/adding-an-ai-tool.md), and the
  [Phase 19 checklist](phase-19-checklist.md) (how the AI tools page was built).

**Numbers:** ADR-080 and ADR-081. ADR-069 to ADR-079 are kept for Phase 20 (20B and 20C), so Phase
16 starts at 080; Antigravity CLI, if it passes its checks, gets ADR-082. **No new Ledger layout:** who
made a model is worked out from each AI tool's list every time, never saved.

Dates are Pacific time.

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): the "maker" is **who made it**, a
"runtime" is an **AI tool**, an "alias" such as `opus` is a **name that points to the newest
model**, and "cross-provider review" is **a different AI company than the work it reviews**.

**Goal (plan):** "Reach every AI model and AI company that is worth having, without weakening
Guard, the Ledger, or the owner's control of what gets spent." Wave 1: "fits today's rules, no
paid key".

## In short, for the owner

**What happened first:** you asked me to start Phase 16 now, beside Phase 20B. I wrote that down
as ADR-080 (building Phase 16's first wave alongside Phase 20) and changed the plan's order-of-work
table to say so. Then I read the code and wrote this plan. Nothing is built yet.

**What Wave 1 gives you:**

- **Every model says who made it.** Today Plenipo only knows which AI tool runs a model, so all
  of Ollama's models count as "Ollama". After this, gpt-oss says "made by OpenAI", DeepSeek V4 Pro
  says "made by DeepSeek", and so on.
- **Cross-company review gets fixed.** When a reviewer must come from "a different AI company",
  Plenipo compares who **made** the models. Today, OpenAI's gpt-oss on Ollama can review Codex's
  work (also OpenAI) as if it were a different company. That stops.
- **Your model list, two ways.** Group it by **who made it** or by **AI tool**. Plenipo remembers
  your choice.
- **Exact Claude versions.** "Opus" shows the exact version it points to now, and you can pick an
  exact version so a role stays on it.
- **Older OpenAI models** your ChatGPT sign-in really allows, each checked on your PC.
- **Google's Gemini models.** Gemini CLI can't be used any more: Google stopped serving it to
  personal Google plans (the [finding](ai-tools-gemini-finding.md)). At your direction, Plenipo
  checks Google's replacement, **Antigravity CLI**, as its Google AI tool — or writes a finding for
  it too, if it can't pass the safety bar.
- **More Ollama cloud models**, only if your paid Ollama plan is active.

**What you need to do:**

1. **Answer choices 1 to 4 below** (or say "as recommended").
2. **Finish the checks on your Windows PC** ([the steps](phase-16-owner-checks.md)): update Codex
   and redo Part B, and do Part E (Antigravity CLI). I build the maker, review, and list parts as
   soon as you approve; the OpenAI, Google, and Ollama model lists wait for your results, because
   I won't guess what your plans allow.

## Choices for you

Each choice has my recommendation first. Say "as recommended" to take them all, or name the ones
you want different.

1. **Which list gets "Group by: Who made it / AI tool"?** (ADR-081 §6)
   - **Recommended:** **Settings → AI models → Your models**, the list your roles choose from.
     Every row shows both who made it and the AI tool. Everywhere else a model is listed (each AI
     tool's Models tab, and the model menus) also says who made it.
   - Other: a new list of **every model** each AI tool offers, on the AI tools page, grouped
     either way. Bigger: it shows many models that aren't in your list.
   - Other: both.
2. **"Never use these AI companies": who made it, too?** (ADR-081 §5)
   - **Recommended:** yes. A company on the list is skipped if it made the model **or** its AI
     tool runs it. So "never OpenAI" also skips gpt-oss on Ollama, and the list offers DeepSeek,
     Z.ai, MiniMax, and NVIDIA too.
   - Other: keep today's rule (only the AI tool's company) for this wave.
3. **A model when Plenipo doesn't know who made it** (only an Ollama model that isn't on
   Plenipo's checked list, such as one you typed). (ADR-081 §7)
   - **Recommended: play it safe.** It shows "Who made it: not known". A reviewer that **must**
     come from a different company never uses it, and work done by it is never counted as from a
     different company, so such a review waits and says why. A reviewer that only **prefers** a
     different company tries it last.
   - Other: count it as a company of its own (different from every known one). Simpler, but it
     could let a model review work from its own maker.
   - Other: let you say who made it when you add it to your list (a new "Who made it" menu in
     **Add a model**, only for Ollama).
4. **Is your paid Ollama plan active now?**
   - **If yes:** do Part D of the checks, and tell me which extra cloud models you want. I add
     each one you check.
   - **If not yet:** Wave 1 goes ahead without more Ollama models. They can come in a small
     follow-up once the plan is active.

5. **Gemini CLI can't sign in with a personal Google plan. Check Google's replacement,
   Antigravity CLI, in its place?** — **Answered (2026-09-29): yes, now.** The owner: "gemini is a
   critical AI LLM we need working in Plenipo so yes, do A now please." Antigravity CLI is checked
   in this wave (Part E), gets its own decision record (ADR-082) if it passes, and a finding if
   not.

Already decided, not a choice: version **1.14.0** (your instruction); the grouping choice is
remembered on this PC, like the light or dark theme; the usage-limit rule ("wait") keeps comparing
the AI tools' companies, because it is about which subscription pays — moving work between
subscriptions is Wave 3's routes (ADR-081 §4).

## The checks on your PC

[The steps](phase-16-owner-checks.md), in PowerShell 7. **Results so far (2026-09-29):**

- **Part A — Claude: done.** Claude Code **2.1.284**, Claude Max, signed in with the subscription
  (`SignIn : none` on every task). The names point to: `opus` → `claude-opus-5-5`, `sonnet` →
  `claude-sonnet-5-5`, `haiku` → `claude-haiku-4-5-20251001`, `fable` → `claude-fable-5-1`. Each
  exact version ran by name ("OK"), except Fable, whose name and exact version were both accepted
  but stopped at the plan's weekly Fable limit ("You've reached your Fable limit"): a usage limit,
  not a refusal. Claude Code's own `/model` list shows the same: Opus 5.5, Fable 5.1, Sonnet 5.5,
  Haiku 4.5. (The first run used an old `ANTHROPIC_API_KEY` setting on the PC instead of the
  subscription, and every task was refused; the owner turned it off for the check window. Plenipo
  never passes that setting to Claude Code.)
- **Part B — Codex: redo after updating Codex.** The PC had Codex **0.145.0** (Plenipo is checked
  against 0.157.1), signed in with ChatGPT. Its app server listed GPT-5.6-Sol, GPT-5.6-Terra,
  GPT-5.6-Luna, and GPT-5.5, and two hidden models, GPT-Reserve (`gpt-reserve`) and Codex Auto
  Review (`codex-auto-review`), which both ran. **Every older model tried was refused** on the
  ChatGPT sign-in ("The '…' model is not supported when using Codex with a ChatGPT account"):
  `gpt-5.4`, `gpt-5.3-codex`, `gpt-5.2-codex`, `gpt-5.2`, `gpt-5.1-codex-max`, `gpt-5.1-codex`,
  `gpt-5.1`, `gpt-5-codex`, `gpt-5`, `o3`, `o4-mini`, `gpt-4.1`. The list can differ on a newer
  Codex, so Part B runs again after `npm install -g @openai/codex@latest`.
- **Part C — Gemini CLI: done; it fails the bar.** Sign in with Google was refused: "This client is
  no longer supported for Gemini Code Assist for individuals." npm installed only Node.js shortcuts
  (no `gemini.exe`), and WinGet has no package. The [finding](ai-tools-gemini-finding.md).
- **Part E — Antigravity CLI, step 0: to do.** What the build machine already saw, signed out, is
  in [the evidence](evidence/phase-16/README.md): version 1.2.13, a real `.exe` on Windows, the task
  on standard input (`-p=` with `--input-format stream-json`), JSON lines out, resume by
  conversation ID, `agy models` refusing when signed out (the likely sign-in check, as Grok's), and
  key mode needing both a settings entry and a key variable Plenipo never passes. Open: what `agy
models` shows signed in; the `useG1Credits` switch ("personal AI credit consumption when quota
  exhausted"); and its background self-updates. **I stop for Antigravity until Part E comes back.**
- **Part D — Ollama:** only if the paid plan is active (choice 4).

## Code map (read at `936c8e6`, `main`, v1.13.0)

| Area                      | File                                                                                                                                                                                                                           | Today                                                                                     | Wave 1                                                                                                                                                                                  |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A model an AI tool lists  | `crates/runtime/src/agent/dto.rs` (`KnownModel`, `RuntimeCapabilities`)                                                                                                                                                        | name, label, effort levels                                                                | + `maker` (ID and name, `None` when not known), + `points_to` (the exact model a name follows); the tool gains its default model's maker and "runs only its own company's models"       |
| Each AI tool's list       | `crates/runtime/src/agent/{claude_code,codex,grok,kimi}.rs`, `ollama/mod.rs`                                                                                                                                                   | fixed lists (Claude Code 2.1.283, Codex 0.157.1, Grok 1.0.41, Kimi 0.34.0, Ollama 0.34.4) | every model gets its maker; Claude Code gains exact versions (Part A); Codex gains older models (Part B); Ollama more models (Part D)                                                   |
| Models a tool reports     | `parse_models` in each adapter                                                                                                                                                                                                 | new models "not checked yet" (ADR-060 §5)                                                 | a reported model's maker: the tool's company when it runs only its own; not known for Ollama                                                                                            |
| The contract suite        | `crates/runtime/tests/contract.rs`                                                                                                                                                                                             | identity, models, effort, no key variables                                                | + every listed model has a maker; one name per company across tools; a one-company tool lists only its company's models; key variables still refused for every subscription AI tool     |
| The stand-in AI tool      | `crates/runtime/src/bin/plenipo-fake-agent.rs`                                                                                                                                                                                 | a persona per AI tool                                                                     | new reported models for the maker tests; an Antigravity persona only if it passes                                                                                                       |
| The Router's view         | `crates/router/src/dto.rs` (`ToolInfo`, `ModelInfo`, `RouteChoice`)                                                                                                                                                            | a model's company is its tool's                                                           | a model's maker (worked out, never saved); `RouteChoice` says who made the chosen model                                                                                                 |
| The decision              | `crates/router/src/engine.rs` (`route`, `same_company`, `never_by`)                                                                                                                                                            | compares `info.provider` (the AI tool's company) for review and never-use                 | review compares makers (§3); never-use per choice 2; the usage-limit rule unchanged                                                                                                     |
| The Router service        | `crates/router/src/service.rs` (`RouteRequest::reviewed`), `config.rs` (`check_companies`)                                                                                                                                     | `reviewed: &[String]` (AI tools only); never-use accepts the tools' companies             | the reviewed work is an AI tool **and a model**; never-use also accepts makers (choice 2)                                                                                               |
| Whose work is reviewed    | `crates/liaison/src/service.rs` (`reviewed_work`), `directory.rs` (`Directory::place`)                                                                                                                                         | the runtime IDs of the referenced tasks, or the requester's                               | each with its model: the task's `model`, else the model its result reported, else the AI tool's default                                                                                 |
| Placing a worker          | `crates/workforce/src/directory.rs`, `service.rs`                                                                                                                                                                              | passes runtime IDs on                                                                     | passes the AI tool and model on                                                                                                                                                         |
| Your models               | `apps/desktop/src/components/models/ModelList.tsx`                                                                                                                                                                             | one table: Name, AI tool, Model the tool runs, …                                          | **Group by: Who made it / AI tool** (the design system's `Segmented`), a heading per group, a "Who made it" column; remembered with `useStoredState` (choice 1)                         |
| Model menus               | `apps/desktop/src/routing/format.ts` (`modelGroups`, `companies`), `components/models/ModelPicker.tsx`                                                                                                                         | "Claude Code's models", "Your models", "Seen in use"                                      | for a tool that runs several companies' models, each option says who made it; a name that points to the newest model says what it points to now; `companies()` offers makers (choice 2) |
| AI tools page             | `apps/desktop/src/components/aiTools/ModelsTab.tsx`                                                                                                                                                                            | each model's label, name, and effort                                                      | + "made by …", + "now …" for Claude's names                                                                                                                                             |
| Types for the screens     | `packages/types/src/generated` (`pnpm bindings`)                                                                                                                                                                               | —                                                                                         | regenerated                                                                                                                                                                             |
| Tests                     | `crates/router/src/engine.rs` tests, `crates/liaison/tests/handoffs.rs`, `crates/workforce/tests/`, `apps/desktop/src/components/models/ModelSettings.test.tsx`, `aiTools/aiTools.test.tsx`, `tests/e2e/specs/routing.e2e.mjs` | cross-company review by AI tool                                                           | the plan's tests (below)                                                                                                                                                                |
| Antigravity, if it passes | `crates/runtime/src/agent/antigravity.rs`, `mod.rs` (`builtin_adapters`), `docs/development/setup.md` §3                                                                                                                       | —                                                                                         | a new adapter, its persona, its ADR-082; or `docs/phases/ai-tools-antigravity-finding.md`. Gemini CLI: `docs/phases/ai-tools-gemini-finding.md` (written)                               |

**Not touched** (Phase 20's): `crates/capabilities/src/connections/`, `crates/guard/src/connections.rs`,
the Vault, `apps/desktop/src/settings/connections/`, and the Connections tests.

## Design (2026-09-29)

### 1. Who made each model (ADR-081 §1, §2)

- `KnownModel` gains `maker: Option<Maker>` (`Maker { id, label }`), and `RuntimeCapabilities`
  gains `default_maker` (who made the tool's default model) and `own_models_only` (it runs only
  its company's models).
- The makers of today's checked models:

  | AI tool     | Models                                                                                                        | Who made them                                             |
  | ----------- | ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
  | Claude Code | Fable, Opus, Sonnet, Haiku                                                                                    | Anthropic                                                 |
  | Codex       | GPT-6-Astra, GPT-6-Sol, GPT-6-Luna, GPT-5.6-Sol, GPT-5.6-Terra, GPT-5.6-Luna, GPT-5.5                         | OpenAI                                                    |
  | Grok        | Grok 4.7, Grok 4.7 Fast, Grok 4.6, Grok 4.5                                                                   | xAI                                                       |
  | Kimi        | K3, K3-256k, K2.8 Preview, K2.7 Code Highspeed                                                                | Moonshot AI                                               |
  | Ollama      | gpt-oss 120B / Nemotron 3 Ultra / Kimi K3 / DeepSeek V4 Pro, V4.1 Flash / GLM-5.3, GLM-5.3 Flash / MiniMax M3 | OpenAI / NVIDIA / Moonshot AI / DeepSeek / Z.ai / MiniMax |

- IDs: a maker that is also an AI tool's company uses that company's ID (`anthropic`, `openai`,
  `xai`, `moonshot`); the others are `deepseek`, `zai`, `minimax`, and `nvidia` (and `google` if
  Antigravity lands; if its list also offers other companies' models, each says who made it, as
  Ollama's do). One name per ID everywhere (the contract suite checks).
- A model not on a tool's list: its company for a one-company tool; **not known** for Ollama
  (choice 3). "Its default": the maker of the default model (Ollama: OpenAI).
- Worked out each time from the tool's list; the owner's model list is not changed; no new Ledger
  layout.

### 2. Cross-company review counts who made the models (ADR-081 §3)

- Liaison's `reviewed_work` returns, for each reviewed task, its AI tool and its model: the task's
  `model`, else the model its result reported (as `workforce/src/outcome.rs` already reads it),
  else none (the tool's default). `Directory::place`, the Workforce directory, and
  `RouteRequest::reviewed` carry that pair.
- The engine works out each reviewed maker, then: **prefer** puts models by other makers first;
  **require** skips a model by a reviewed maker. The reason names makers ("It comes from a
  different AI company than the work it reviews (DeepSeek).").
- Unknown makers per choice 3.
- Unchanged: the usage-limit rule (compares AI tools' companies), pay-per-use billing (off), and
  every route decision that has no review in it.

### 3. AI companies never to use (choice 2)

Recommended: `never_by` checks the maker and the AI tool's company; `check_companies` and the
desktop's `companies()` also offer every maker the AI tools list. The saved rules stay as they are
(IDs), so nothing saved needs to change.

### 4. Your models, two ways (choice 1)

- **Group by: Who made it / AI tool** above the table (the design system's `Segmented`, as "Cards /
  List" is). A heading per group ("Anthropic", "DeepSeek", "Not known"; or "Claude Code",
  "Ollama"), ordered by name, the rows in your list's order within each.
- Each row shows **Who made it** and **AI tool**, so both groupings show the same facts, and the
  same models (a test checks the two groupings have the same model IDs).
- Remembered on this PC (`useStoredState`, key `plenipo.models.groupBy`), starting on **AI tool**.
  No new desktop command, no Ledger change.
- Each AI tool's **Models** tab shows "made by …" for each model; the model menus add who made it
  for a tool that runs several companies' models ("DeepSeek V4 Pro — made by DeepSeek").

### 5. Exact Claude versions (ADR-081 §8; Part A done)

- For each of `fable`, `opus`, `sonnet`, `haiku`: `points_to` = the exact model Claude Code's own
  `init` reported on the owner's PC, labeled "now …" on screen ("Opus — now Opus 5.5"):
  `claude-fable-5-1` (Fable 5.1), `claude-opus-5-5` (Opus 5.5), `claude-sonnet-5-5` (Sonnet 5.5),
  and `claude-haiku-4-5-20251001` (Haiku 4.5).
- Each exact version is listed as its own model, with the same effort levels as its name (Haiku:
  none). Opus, Sonnet, and Haiku ran by name on the subscription; Fable's exact version was
  accepted and stopped at the plan's weekly Fable limit, which is a usage limit, not a refusal, so
  it is listed too.
- `checked_version()` moves from 2.1.283 to **2.1.284**, the version checked. The comment that
  says `sonnet` is Sonnet 5 is corrected to Sonnet 5.5.

### 6. Older OpenAI models (ADR-081 §9; Part B again after updating Codex)

- Codex's list gains each **older** model that ran on the owner's ChatGPT sign-in, with the effort
  levels Codex reported for it, after today's models. A name that is refused is not listed.
- **So far none:** on Codex 0.145.0, every older model tried was refused (the results above).
- Codex's two **hidden** models, GPT-Reserve and Codex Auto Review, ran, but they are not older
  models, and Codex keeps them out of its own picker. Plenipo lists what the AI tool's own picker
  offers (ADR-011 §16), so they stay out; a name can still be typed.
- If the run on the newer Codex finds nothing older either, this deliverable closes as "checked:
  a ChatGPT sign-in allows no older OpenAI models in Codex", recorded in the acceptance report.

### 7. Google's AI tool: Gemini CLI's finding, then Antigravity CLI (choice 5; Part E)

- **Gemini CLI fails bar item 3** (subscription sign-in): Google stopped serving it to personal
  plans on 2026-06-18. [The finding](ai-tools-gemini-finding.md) is written; no adapter.
- **Antigravity CLI** (Google's replacement, `agy`) is checked against the same bar (ADR-014, with
  ADR-015's rule that the task goes in on standard input). From the build machine: bar items 1
  (one task, the words on standard input), 2 (JSON lines with the conversation ID, the answer,
  errors, and token counts), and 5 (a version flag, resume by ID) look met. Open, for Part E:
  - **item 3 (subscription only):** `useG1Credits` can spend paid credits once the plan's
    allowance runs out. Plenipo must be able to keep that off for its tasks, or it is a finding,
    as Copilot's paid overage was;
  - **item 4 (a sign-in check that tells a subscription from a key):** `agy models`, as with Grok,
    because key mode needs both a settings entry and a key variable, and Plenipo never passes key
    variables;
  - **least privilege:** what `--mode plan --sandbox` allows, and how Plenipo's tools (Guard) reach
    it;
  - **updates:** it updates itself in the background; Plenipo updates AI tools only between tasks
    (ADR-059).
- **If it passes:** an adapter with its own decision record (ADR-082) for what differs from the
  other AI tools, a persona in the stand-in AI tool, and the whole contract suite. Its models say
  who made them (Google, and any other company its list offers).
- **If it fails:** `docs/phases/ai-tools-antigravity-finding.md`, as Gemini CLI's.

### 8. More Ollama cloud models (choice 4; Part D)

Only if the paid plan is active: each model the owner picks, checked with `ollama show` (its
thinking levels) and a one-word task, is added with its maker, as ADR-017 did. The six "(paid
plan)" labels stay: those models still need a paid plan.

### 9. Guard and the capability broker — the owner's rule

Wave 1 adds no new reach: no new file access, program, network address, browser, or screen use.
Model lists are data in the adapters. Every AI tool program is still started only by the
supervisor as an approved program, through the same checks as today. If Antigravity lands, its
program, its sign-in tab, and its updates go through exactly the paths Phase 19 built (the
supervisor, `Guard::check_ai_tool_action`), and its file requests over ACP through Guard, as
Kimi's do (ADR-027).

### 10. Nothing loaded while Plenipo runs (ADR-014)

Every maker, model, and version is compiled into Plenipo. Nothing is loaded or downloaded while it
runs.

### 11. What is recorded, and what never is

Nothing new is recorded. The Router's reason, already saved with each worker, names makers. No
log or diagnostics file gets anything new; they never hold secrets or anything typed in the
terminal (unchanged).

### 12. Desktop commands

**No new desktop command is planned.** Makers reach the screens through the commands that exist
(`get_routing`, `get_ai_tools`), which are already the main window's alone. If one turns out to be
needed (for example for Antigravity), it is added for the main window only, and the IPC tests show the
sign window and web pages are refused.

### 13. The stand-in AI tool, and tests

The plan's tests (below) at the level that proves each: the Router's unit tests, the contract
suite, Liaison and Workforce integration tests, Vitest for Settings → AI models and the AI tools
page, and **end-to-end tests in the real app** (`tests/e2e/specs/routing.e2e.mjs`) with screenshots
in `docs/phases/evidence/phase-16/`.

### 14. Words on screen

New pairs for the word list: **who made it** (for "maker", "model vendor"); **Group by: Who made it
/ AI tool** (for "group by provider / runtime"); **not known** (for "unknown maker"); **now …**,
as in "Opus — now Opus 5.5" (for "alias resolves to", "snapshot"); **exact version** (for "pinned
model ID").

### 15. Staying out of Phase 20's way

No change to Connections code, the Vault, the Connections page, or their tests. Merge `main`
whenever it moves; in `ROLLOUT_PLAN.md`, `versioning.md`, `vocabulary.md`, the ADR index, package
versions, and `Cargo.lock`, keep both sides.

## Deliverables (plan, Wave 1)

- [ ] `maker` on every known model: who made it, separate from the AI tool that runs it
- [ ] Cross-company review counts the maker, not the AI tool (fixes a real hole in ADR-011 for
      Ollama's models)
- [ ] The model list groupable by maker or by the app that runs it, the owner's choice
      (remembered)
- [ ] Exact Claude model versions beside the plain names (after Part A)
- [ ] The older OpenAI models a ChatGPT sign-in really allows, each checked (after Part B)
- [x] Google's Gemini CLI as an AI tool, or a written finding — **finding**
      ([ai-tools-gemini-finding.md](ai-tools-gemini-finding.md), 2026-09-29)
- [ ] Added at the owner's direction (choice 5): Google's Antigravity CLI as an AI tool, or a
      written finding (after Part E)
- [ ] More Ollama cloud models once the owner's paid plan is active (choice 4, Part D)

## Technical implementation (plan, the parts for Wave 1)

- [ ] **The maker field comes first.** Add `maker` to `KnownModel`, run `pnpm bindings`, and point
      cross-company review at it.
- [ ] **New AI tools follow the existing guide** and ADR-014's bar, with step 0 run on the owner's
      Windows PC before any code; the prompt goes in on standard input, directly or over ACP
      (ADR-015); a tool that fails the bar merges a finding, not a workaround.

## Tests (plan, Wave 1, and the owner's list)

- [ ] Cross-company review: two models with the same maker on different AI tools count as one
      company.
- [ ] Two makers inside Ollama count as two.
- [ ] The model list groups correctly by maker and by AI tool, with the same models in both.
- [ ] The contract suite still refuses key variables for every subscription AI tool.
- [ ] With the paid switch off, nothing changes (the payment switch still refuses a paid key, API
      billing stays off, and a tool signed in with a key is still skipped).
- [ ] Vitest for the page (Settings → AI models, the AI tools page's Models tab).
- [ ] End-to-end tests in the real app, with screenshots in `evidence/phase-16/`.
- [ ] If Antigravity lands: it passes the whole contract suite with its own persona.

## Owner's rules for this wave

- [ ] Plain words on screen (the word list gains the new pairs); ADRs named, never just numbered.
- [ ] No passwords, keys, tokens, or secrets asked for in chat; nothing secret committed.
- [ ] Anything touching files, programs, the network, the browser, or the screen goes through
      Guard and the capability broker (§9).
- [ ] Nothing loads code into Plenipo while it runs (§10).
- [ ] New desktop commands, if any, are the main window's alone; the sign window and web pages
      are refused (IPC tests) (§12).
- [ ] Logs and diagnostics files never hold secrets or anything typed in the terminal (§11).
- [ ] No model names in commits, branch names, or pull requests (model names in the app's model
      list are the feature).
- [ ] Nothing in Connections, the Vault, or Phase 20's tests (§15); `main` merged whenever it moves.
- [ ] Before each push: `pnpm check`, `cargo fmt --all -- --check`,
      `cargo clippy --workspace --all-targets --locked -- -D warnings`,
      `cargo test --workspace --locked` (rerun until the whole suite finishes), and `pnpm bindings`
      with no diff (documentation-only pushes: `pnpm docs:check`).
- [ ] A review across several areas, with a second reviewer checking each finding, before the
      final push; each confirmed finding fixed with a test, or recorded as a design limit.
- [ ] Every GitHub check green, Windows included.

## Paperwork (at the end)

- [ ] This checklist ticked; `docs/phases/phase-16-acceptance-report.md` (Wave 1).
- [ ] `docs/releases/v1.14.0.md`; version 1.14.0 everywhere; a row in
      `docs/development/versioning.md`.
- [ ] The plan's Phase 16 status line and its state in the order of work.
- [ ] "As built" sections in ADR-080 and ADR-081 (and ADR-082 if Antigravity lands).
- [ ] New word pairs in `docs/design/vocabulary.md`.
- [ ] Before merging: check whether Phase 20B or 20C is still open, and tell the owner that they
      would move from 1.13.x to 1.14.x if this merges first.

## Left for the owner (on Windows)

- The checks on [the check page](phase-16-owner-checks.md) (before the lists are built).
- At acceptance: Settings → AI models grouped both ways; a cross-company review with an Ollama
  model; a task on an exact Claude version and on an older OpenAI model; Antigravity, if it lands: one
  task, a resumed task, cancel, and a refused sign-in.
