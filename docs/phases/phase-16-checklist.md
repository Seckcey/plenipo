# Phase 16 — Implementation Checklist (Wave 1)

**Status:** design written (2026-09-29), **waiting for the owner**: the four choices below, and
the checks on the owner's Windows PC ([the steps](phase-16-owner-checks.md)). Nothing is built
yet. Builds on v1.13.0 (Phase 20A); releases as **v1.14.0**. Built beside Phase 20B (Slack and
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
16 starts at 080; Gemini CLI, if it passes its checks, gets ADR-082. **No new Ledger layout:** who
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
- **Google's Gemini CLI** as a new AI tool — or, if it can't pass the safety bar, a written
  finding that says why.
- **More Ollama cloud models**, only if your paid Ollama plan is active.

**What you need to do:**

1. **Answer the four choices below** (or say "as recommended").
2. **Run the checks on your Windows PC** ([the steps](phase-16-owner-checks.md), about 30
   minutes) and send me the results. I build the maker, review, and list parts as soon as you
   approve; the Claude, OpenAI, Gemini, and Ollama model lists wait for your results, because I
   won't guess what your plans allow.

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

Already decided, not a choice: version **1.14.0** (your instruction); the grouping choice is
remembered on this PC, like the light or dark theme; the usage-limit rule ("wait") keeps comparing
the AI tools' companies, because it is about which subscription pays — moving work between
subscriptions is Wave 3's routes (ADR-081 §4).

## What I need from your PC

[The steps](phase-16-owner-checks.md) — about 30 minutes, in PowerShell 7:

- **Part A — Claude:** which exact version `fable`, `opus`, `sonnet`, and `haiku` point to on your
  Claude Code, and that your subscription runs each exact version by name.
- **Part B — Codex:** Codex's full model list, hidden (older) ones included, and a one-word task on
  each older model to see which your ChatGPT sign-in allows.
- **Part C — Gemini CLI, step 0:** install it, sign in with Google, and see whether its `/about`
  answer can tell your Google sign-in from a pay-per-use key; one task, a resumed task, and the
  signed-out and key cases (with a made-up key). **I stop here for Gemini until you report
  back.**
- **Part D — Ollama:** only if the paid plan is active.

What I saw on the build machine, signed out, before writing the steps:
[evidence/phase-16/](evidence/phase-16/README.md). In short: Gemini CLI 0.61.0 reads the task from
standard input and prints JSON lines; it has **no sign-in status command**; over ACP, `/about`
answers without asking the model and names the saved sign-in method and the plan (and the email,
which Plenipo would drop); with a key only in the environment, `/about` says nothing about it; the
check leaves a small conversation file in Gemini's own history; a one-off task needs
`--skip-trust`; and npm installs only Node.js shortcuts on Windows, not a real `gemini.exe`.

## Code map (read at `936c8e6`, `main`, v1.13.0)

| Area                     | File                                                                                                                                                                                                                           | Today                                                                                     | Wave 1                                                                                                                                                                                  |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A model an AI tool lists | `crates/runtime/src/agent/dto.rs` (`KnownModel`, `RuntimeCapabilities`)                                                                                                                                                        | name, label, effort levels                                                                | + `maker` (ID and name, `None` when not known), + `points_to` (the exact model a name follows); the tool gains its default model's maker and "runs only its own company's models"       |
| Each AI tool's list      | `crates/runtime/src/agent/{claude_code,codex,grok,kimi}.rs`, `ollama/mod.rs`                                                                                                                                                   | fixed lists (Claude Code 2.1.283, Codex 0.157.1, Grok 1.0.41, Kimi 0.34.0, Ollama 0.34.4) | every model gets its maker; Claude Code gains exact versions (Part A); Codex gains older models (Part B); Ollama more models (Part D)                                                   |
| Models a tool reports    | `parse_models` in each adapter                                                                                                                                                                                                 | new models "not checked yet" (ADR-060 §5)                                                 | a reported model's maker: the tool's company when it runs only its own; not known for Ollama                                                                                            |
| The contract suite       | `crates/runtime/tests/contract.rs`                                                                                                                                                                                             | identity, models, effort, no key variables                                                | + every listed model has a maker; one name per company across tools; a one-company tool lists only its company's models; key variables still refused for every subscription AI tool     |
| The stand-in AI tool     | `crates/runtime/src/bin/plenipo-fake-agent.rs`                                                                                                                                                                                 | a persona per AI tool                                                                     | new reported models for the maker tests; a Gemini persona only if Gemini passes                                                                                                         |
| The Router's view        | `crates/router/src/dto.rs` (`ToolInfo`, `ModelInfo`, `RouteChoice`)                                                                                                                                                            | a model's company is its tool's                                                           | a model's maker (worked out, never saved); `RouteChoice` says who made the chosen model                                                                                                 |
| The decision             | `crates/router/src/engine.rs` (`route`, `same_company`, `never_by`)                                                                                                                                                            | compares `info.provider` (the AI tool's company) for review and never-use                 | review compares makers (§3); never-use per choice 2; the usage-limit rule unchanged                                                                                                     |
| The Router service       | `crates/router/src/service.rs` (`RouteRequest::reviewed`), `config.rs` (`check_companies`)                                                                                                                                     | `reviewed: &[String]` (AI tools only); never-use accepts the tools' companies             | the reviewed work is an AI tool **and a model**; never-use also accepts makers (choice 2)                                                                                               |
| Whose work is reviewed   | `crates/liaison/src/service.rs` (`reviewed_work`), `directory.rs` (`Directory::place`)                                                                                                                                         | the runtime IDs of the referenced tasks, or the requester's                               | each with its model: the task's `model`, else the model its result reported, else the AI tool's default                                                                                 |
| Placing a worker         | `crates/workforce/src/directory.rs`, `service.rs`                                                                                                                                                                              | passes runtime IDs on                                                                     | passes the AI tool and model on                                                                                                                                                         |
| Your models              | `apps/desktop/src/components/models/ModelList.tsx`                                                                                                                                                                             | one table: Name, AI tool, Model the tool runs, …                                          | **Group by: Who made it / AI tool** (the design system's `Segmented`), a heading per group, a "Who made it" column; remembered with `useStoredState` (choice 1)                         |
| Model menus              | `apps/desktop/src/routing/format.ts` (`modelGroups`, `companies`), `components/models/ModelPicker.tsx`                                                                                                                         | "Claude Code's models", "Your models", "Seen in use"                                      | for a tool that runs several companies' models, each option says who made it; a name that points to the newest model says what it points to now; `companies()` offers makers (choice 2) |
| AI tools page            | `apps/desktop/src/components/aiTools/ModelsTab.tsx`                                                                                                                                                                            | each model's label, name, and effort                                                      | + "made by …", + "now …" for Claude's names                                                                                                                                             |
| Types for the screens    | `packages/types/src/generated` (`pnpm bindings`)                                                                                                                                                                               | —                                                                                         | regenerated                                                                                                                                                                             |
| Tests                    | `crates/router/src/engine.rs` tests, `crates/liaison/tests/handoffs.rs`, `crates/workforce/tests/`, `apps/desktop/src/components/models/ModelSettings.test.tsx`, `aiTools/aiTools.test.tsx`, `tests/e2e/specs/routing.e2e.mjs` | cross-company review by AI tool                                                           | the plan's tests (below)                                                                                                                                                                |
| Gemini, if it passes     | `crates/runtime/src/agent/gemini.rs`, `mod.rs` (`builtin_adapters`), `docs/development/setup.md` §3                                                                                                                            | —                                                                                         | a new adapter over ACP, its persona, its ADR-082; or `docs/phases/ai-tools-gemini-finding.md`                                                                                           |

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
  Gemini lands). One name per ID everywhere (the contract suite checks).
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

### 5. Exact Claude versions (ADR-081 §8; after Part A)

- For each of `fable`, `opus`, `sonnet`, `haiku`: `points_to` = the exact model Claude Code's own
  `init` reported on the owner's PC, labeled "now …" on screen ("Opus — now Opus 5.5").
- Each exact version the owner's subscription ran (Part A: `Outcome : success`, `SignIn : none`)
  is listed as its own model, with the same effort levels as its name. `checked_version()` moves
  to the owner's Claude Code version if it is newer, with the list checked against it.

### 6. Older OpenAI models (ADR-081 §9; after Part B)

Codex's list gains each model that Codex's app server lists as hidden **and** that ran on the
owner's ChatGPT sign-in, with the effort levels Codex reported for it, after today's models. A
name that is refused is not listed. `checked_version()` moves with the check, as above.

### 7. Google's Gemini CLI (after Part C; I stop until then)

- The bar is ADR-014's five items, with ADR-015's ACP route. What I could see signed out is in
  [the evidence](evidence/phase-16/README.md). The open question is bar item 4 (a sign-in check
  that tells a subscription from a key): `/about` over ACP is the candidate.
- **If it passes:** a Gemini adapter over ACP like Kimi's and Grok's, in read-only mode, with its
  own decision record (ADR-082) for what differs: starting Node.js with Gemini's program (npm gives
  no `.exe`), `--skip-trust` for Plenipo's own empty task folder, a check that leaves a small
  conversation in Gemini's history (as Kimi's does), and Google's "AI API Gateway" sign-in refused
  like a key. Gemini gets a persona in the stand-in AI tool and passes the whole contract suite.
- **If it fails:** `docs/phases/ai-tools-gemini-finding.md`, as Copilot's, saying which bar item
  failed, the evidence, and what would change the answer.

### 8. More Ollama cloud models (choice 4; Part D)

Only if the paid plan is active: each model the owner picks, checked with `ollama show` (its
thinking levels) and a one-word task, is added with its maker, as ADR-017 did. The six "(paid
plan)" labels stay: those models still need a paid plan.

### 9. Guard and the capability broker — the owner's rule

Wave 1 adds no new reach: no new file access, program, network address, browser, or screen use.
Model lists are data in the adapters. Every AI tool program is still started only by the
supervisor as an approved program, through the same checks as today. If Gemini lands, its
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
needed (for example for Gemini), it is added for the main window only, and the IPC tests show the
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
- [ ] Google's Gemini CLI as an AI tool, or a written finding (after Part C)
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
- [ ] If Gemini lands: it passes the whole contract suite with its own persona.

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
- [ ] "As built" sections in ADR-080 and ADR-081 (and ADR-082 if Gemini lands).
- [ ] New word pairs in `docs/design/vocabulary.md`.
- [ ] Before merging: check whether Phase 20B or 20C is still open, and tell the owner that they
      would move from 1.13.x to 1.14.x if this merges first.

## Left for the owner (on Windows)

- The checks on [the check page](phase-16-owner-checks.md) (before the lists are built).
- At acceptance: Settings → AI models grouped both ways; a cross-company review with an Ollama
  model; a task on an exact Claude version and on an older OpenAI model; Gemini, if it lands: one
  task, a resumed task, cancel, and a refused sign-in.
