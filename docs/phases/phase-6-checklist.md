# Phase 6 — Implementation Checklist

**Status:** accepted by the owner on 2026-09-26; released as v0.7.0 (see the
[acceptance report](phase-6-acceptance-report.md)).

Source: `ROLLOUT_PLAN.md`, Phase 6 — Model Policy and Intelligent Role Routing. Phase 5 is
implemented and merged ([PR #7](https://github.com/Seckcey/plenipo/pull/7)); the owner asked to
begin Phase 6 on 2026-09-26. This checklist keeps the plan's words where it quotes the plan; the
app uses the plain words in [`docs/design/vocabulary.md`](../design/vocabulary.md).

**Goal:** make model/provider selection configurable by role rather than hard-coded into
coordinators.

## Design decisions (details in ADR-011, how Plenipo picks each worker's AI model)

- **A new crate, `crates/router` (`plenipo-router`)**, as planned by ADR-004: the Model
  Registry, role model policies, the routing engine (a pure function), and the Router service
  that reads the configuration and live AI tool state.
- **Configuration is owner data in the Ledger** (the `routing` setting, one document changed in
  one transaction per edit, each change recorded as a `router.*` event with its new value). No
  schema migration.
- **Model Registry.** Each entry: the AI tool, the model name the tool accepts (or none: the
  tool's own default), the owner's name for it (the alias, kept apart from the provider's
  name), what it can do (sees images, makes images, uses a computer), context size, and a cost
  class. One built-in entry per AI tool ("its default model"). Model names are never assumed:
  the owner adds them, and Plenipo lists the models each AI tool **reported running** ("seen in
  use") so the owner can add them. The CLIs Plenipo uses (ADR-007) do not list models.
- **Provider Registry** is the AI tools Plenipo ships adapters for, with live installation,
  sign-in, billing method, and usage-limit state.
- **Role policy** (every field the plan lists): an ordered model list (first choice, then the
  fallback order), required capabilities, minimum context, AI companies the role never uses,
  cost preference, and cross-company review (off / prefer / require). Subscription-only and
  "API use disabled" are global and fixed in this phase (ADR-007: API billing stays off until a
  later phase configures it); the router refuses API-key sign-ins with that reason.
- **Usage/capacity state** is derived from the Ledger: an AI tool whose latest turn hit a usage
  limit is limited until its reported reset time (or an hour when none is reported), until a
  later turn succeeds, or until the owner says "try again now". Survives restarts by
  construction.
- **Usage-limit behavior** (global, plan §6): **wait** (default — never move work to another AI
  company because of a usage limit) or **use the next choice**.
- **Routing explanation.** Every decision carries one plain sentence and a verdict for each
  model considered. It is recorded with the worker (`org.worker_spawned`) and its task
  (`metadata.routing`), shown on the canvas and in the activity trail, and previewed live in
  Settings for every role.
- **Positions are Automatic or Fixed.** New positions follow their role's policy (Automatic).
  Positions created before Phase 6 keep the AI tool the owner chose (Fixed), so upgrading never
  switches an existing position's AI company silently; the owner can switch either way.
- **When routing happens.** On-call workers: at every handoff, in the transaction that records
  the worker. Full-time agents: when their conversation starts (their first objective), then
  they keep it (continuity); a new agent picks again.
- **Cross-company review.** The work under review is the tasks a request references, or else
  the requester's own work; a reviewer role that prefers (or requires) another AI company is
  routed away from those companies.
- **Effort** (added at the owner's review, ADR-011 §15): each model has an optional effort level
  and each role can set its own for any model; the AI tool gets it on every turn (Claude Code
  `--effort`, Codex `model_reasoning_effort`), the reason says it, and a conversation keeps it
  (Ledger schema 5).
- **Model menus** (added at the owner's review, ADR-011 §16): wherever a model is chosen, a menu
  of the AI tool's models — its default, the models its CLI offers (Claude Code: fable, opus,
  sonnet, haiku; Codex: GPT-6-Astra, GPT-6-Sol, GPT-6-Luna and its older listed models), your
  models, and models seen in use — with typing a name as a last resort. They are offered, never
  added to your list, and each carries the effort levels it accepts.
- **Template defaults** (plan examples): Designer needs a model that sees and makes images;
  Code Reviewer and Security Auditor prefer another AI company; Documentation Writer prefers
  economical models; Senior Developer prefers premium ones. Seeded once per template role; the
  owner can change them.

## Deliverables

- [x] Model Registry (built-in defaults, owner models with aliases and capabilities, models seen
      in use)
- [x] Provider Registry (AI tools, companies, install, sign-in, billing, usage limits)
- [x] Model Policy Engine (pure, explained, every candidate's verdict)
- [x] Preferred models by role
- [x] Fallback models
- [x] Capability requirements
- [x] Provider availability checks
- [x] Usage/capacity state
- [x] Routing explanation (recorded and shown)
- [x] Settings UI (models, AI tools, role model choices with a live "next worker" preview,
      usage-limit behavior)
- [x] Automatic and Fixed positions on the Organization canvas
- [x] Effort per model and per role choice (owner request)
- [x] Model menus wherever a model is chosen (owner request)
- [x] ADR-011 (how Plenipo picks each worker's AI model); architecture, README, vocabulary,
      setup updated

## Phase 6 tests (from plan)

- [x] Preferred model available
- [x] Preferred unavailable → fallback
- [x] Provider unauthenticated
- [x] Usage cap reached
- [x] Capability requirement mismatch
- [x] API fallback disabled
- [x] No eligible model
- [x] Cross-provider reviewer rule

Also: disallowed companies, project AI tool rules, cost preference, minimum context, removed
models, usage limits recovered after a restart and cleared by a success, Fixed positions
unchanged by policy, full-time agents routed once per conversation, IPC boundary tests for every
new command, frontend tests, end to end through the real app.

## Acceptance criteria (from plan)

- [x] Changing a role's model preference in Settings changes the next worker Plenipo launches
      without modifying coordinator prompts or source code.
- [x] Plenipo clearly explains why a particular provider/model was selected.

### Owner check on Windows (~20 minutes)

Uses the organization from the Phase 5 check (Development → Website, with its Senior Developer and
Code Reviewer). Positions created before Phase 6 keep the AI tool you gave them ("Fixed").

1. **AI tools** → **Re-check**: Claude Code and Codex both **Ready**.
2. **Settings** → **AI models**. _Your models_ lists _Claude Code (default model)_ and _Codex
   (default model)_; _AI tools_ shows both "Yes"; _Pay-per-use API billing: Off_. Every role shows
   the model its next worker would get and why (the Designer shows "None right now": no model is
   marked as able to see and make images).
3. **Add a model** → AI tool _Claude Code_ → **Model**: the menu lists _The AI tool's default
   (already in your list)_, then Claude Code's models (_fable_, _opus_, _sonnet_, _haiku_) and
   any models seen in use. Choose _sonnet_ (your name for it fills in as _Sonnet_), Effort
   _Medium_ → **Add model**. The table shows its effort. Try **Add a model** → _Codex_ too: its
   menu lists _gpt-6-astra_, _gpt-6-sol_, _gpt-6-luna_ and Codex's older models, and Effort offers
   only the levels the chosen model takes (up to _Ultra_ for GPT-6-Sol; _haiku_ has none). The
   same menu appears wherever you choose a model: hiring a fixed position, a new department's or
   project's lead, **Edit title or AI model**, and Workers → Advanced. **Type another name…** is
   there for anything not listed.
4. **Organization** → select _Website Supervisor_ → **Hire into team** → Role _Senior Developer_,
   Title _Backend Developer_, AI tool **Automatic** → **Hire**. Its node reads "Auto · Claude Code".
5. **Settings** → **AI models** → _Senior Developer_ → **Change** → add _Codex (default model)_ →
   **Save model choices**. The row now reads "Codex (default model) is Senior Developer's first
   choice and is ready."
6. **Organization** → _Website Supervisor_ → objective: _"Ask the Backend Developer to write a
   Python function that reverses a string, then give me the result."_ Expected: a worker appears
   under _Backend Developer_ with a **Codex** chip; select _Backend Developer_ → **Why the next
   worker gets this model** explains it. (Acceptance criterion 1 and 2.)
7. **Settings** → _Senior Developer_ → **Change** → remove Codex, add _Sonnet_, set its effort
   to _High effort_ → **Save**. The row reads "Sonnet (Claude Code) · high effort". Give the
   supervisor the same objective again: this worker runs on **Claude Code** with model `sonnet`
   at high effort (the worker's details show "Claude Code · sonnet" and why, ending "It runs at
   high effort (Senior Developer's setting for it)."). The supervisor and its instructions did not
   change.
8. **Activity** → the objective's task → the worker's child task: "Worker brought in for Backend
   Developer — Sonnet (Claude Code) is Senior Developer's first choice and is ready. It runs at
   high effort (Senior Developer's setting for it)."
9. Optional: select the Phase 5 _Senior Developer_ (fixed) → **Why this AI model** says you set
   it; **Edit title or AI model** → AI tool **Automatic** makes it follow the role's choices.

Things only real CLIs can confirm (report anything odd): that each effort level runs (Plenipo
offers only the levels each CLI lists; an older CLI without `--effort` fails the worker's task with
its own message), that each model name you add is one your CLI accepts (a wrong name fails that worker's task with the CLI's own message), and how a
real usage limit reads (Claude Code reports its reset time; Plenipo then shows "resets in …").

## Out of scope

Autonomous purchasing, changing subscription plans, unsupported model scraping, hidden provider
switching (plan). Also: pay-per-use API billing (stays off, ADR-007), capability grants (Phase 7),
delegation between full-time positions (Phase 8).
