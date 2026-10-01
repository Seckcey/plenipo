# ADR-131: Wave 4 changes course — Plenipo's own tools for any model, and specialist jobs, instead of Hermes Agent

- **Status:** Accepted (by the owner, 2026-09-30)
- **Date:** 2026-09-30
- **Phase:** 16, Wave 4
- **Changes:** [ADR-036 (every AI model worth having)](ADR-036-every-ai-model.md) §5, Wave 4
  ("Hermes Agent, for specialist workers"). The rest of ADR-036 stands.
- **Finishes:** the tools follow-up in [ADR-017 (Ollama's cloud models)](ADR-017-ollama-cloud-models.md)
  §4, and the "text only in this wave" limit in
  [ADR-086 (OpenRouter through a Plenipo helper)](ADR-086-openrouter-through-a-plenipo-helper.md) §8
  and [ADR-087 (direct keys for every AI company)](ADR-087-direct-keys-for-every-ai-company.md).

> **On screen** (ADR-010, plain words and rank names): "Can use files, programs, and the browser",
> "Answers in text only", and a job's name such as **Security Reviewer**. Never "tool calling",
> "function calling", "agent loop", "harness", or "persona".

## Context

ADR-036 planned Hermes Agent (Nous Research) for Wave 4, because the owner wanted specialist workers
for specialist jobs such as security review. On 2026-09-30, with Wave 3 delivered in v1.17.0, the
owner asked what Hermes really offers that Plenipo's AI tools do not. Hermes' own documentation
(read 2026-09-30) shows:

- **No ready-made specialists to pick.** Hermes has _skills_ (written how-to guides, about 87 come
  with it, more from its Skills Hub, and it writes its own as it works) and _profiles_ (separate
  helpers with their own personality, memory, and skills) that the owner sets up by hand.
- **Its real strength:** it lets any model, including OpenRouter's, use files, a terminal, and a
  browser.
- **The rest overlaps Plenipo or is out of scope:** memory between chats, scheduled jobs, chatting
  from Slack or Telegram, and its own helpers. Plenipo already has an org chart, approvals, the
  Ledger, Connections, and schedules; and ADR-036 §6 keeps gateways into other agent systems out.

Plenipo already has what Hermes would add. It has its own list of tools for workers
(`crates/capabilities/src/tools.rs`: files, search, programs, PowerShell, git, GitHub, the
browser, the screen, and servers), each checked by the capability broker and Guard. Claude Code,
Codex, Grok, and Kimi reach that list today. Ollama, OpenRouter, and the ten direct-key AI
companies do not: their workers answer in text only. All of them speak the same "the model asks for
a tool, the app runs it, the app gives back the result" way of talking.

## Decision

The owner's choice (2026-09-30): drop Hermes, and build the two things the owner actually wanted.

### 1. Hermes Agent is dropped from Wave 4

It is not an AI tool Plenipo adds. If it is ever wanted again, it needs a new record and ADR-014's
step 0 (the rules for adding AI tools).

### 2. Wave 4, part 1 — Plenipo's own tools for workers on Ollama, OpenRouter, and direct keys

A worker on Ollama, OpenRouter, or a direct key gets Plenipo's tool list, the same list other
workers reach:

1. **Plenipo offers the tools** with the request, in the way each service takes them.
2. **The model asks for a tool;** the helper hands the request to Plenipo and does not run it.
3. **Plenipo runs it through the capability broker and Guard** (ADR-013, how Plenipo lets workers
   use your computer safely), exactly as for every other AI tool: the position's permissions,
   approvals, the never-list (ADR-025), the Ledger record, and the activity the owner watches.
4. **The result goes back to the model,** and the request repeats until the model answers.

Limits:

- **Only models that say they can use tools** get them (Ollama's model details, OpenRouter's
  model list). The rest stay "Answers in text only", and the Router's reason says which.
- **A most-steps limit per task,** so a worker that loops stops.
- **Paid routes:** every round is a paid step, priced and set aside under the spending caps
  before it is sent (ADR-085 §2.5). The hard stop still never goes over.
- **What is sent stays fixed and counted** (ADR-086 §4): tool results are cut to the same sizes
  the tool list already uses.
- **Step 0 first** on the owner's PC: one Ollama model and one OpenRouter model each read a file,
  run an allowed program, and open a page, with Guard refusing what the position may not do.

### 3. Wave 4, part 2 — specialist jobs

Plenipo comes with **ready-made jobs** the owner picks when adding a position. Each is a job title,
written instructions, the permissions it needs, and the kind of model that suits it (for example,
"made by a different company from the worker who wrote the code" for a reviewer, ADR-011). A job
works with any AI tool.

- **First set:** Security Reviewer, Code Reviewer, Researcher, Writer, and IT Support for Windows
  servers. The owner may change any of them.
- **Built into Plenipo,** as plain text. Nothing is loaded while Plenipo runs (ADR-014).
- **Security work stays inside the owner's own authority** (ADR-036 §5): systems the owner or the
  owner's clients own, with Guard's approvals and the never-list.
- Phase 15's "reusable role packs" and their import and export stay in Phase 15; this part delivers
  the built-in jobs early.

### 4. Order

Part 1 (tools), then part 2 (specialist jobs). Each part has its own checklist and acceptance report
in `docs/phases/`.

## Consequences

- **"Any model" becomes real.** Hundreds of OpenRouter models, Ollama's models, and the ten
  direct-key companies can do real work, not just answer.
- **No outside program to install, update, or trust,** and no second set of permissions beside
  Guard's.
- **Paid tool use costs more per task** than a single answer, because each round is billed. The
  caps already hold it.
- **Small models use tools poorly.** The Router keeps preferring proven models for work that
  changes files.
- **Open question for the editions work (Phase 11A + 22):** which edition includes the specialist
  jobs. Until that is decided, they are built for every edition.

## Alternatives considered

- **Add Hermes Agent as planned.** Rejected by the owner: it has no ready-made specialists, and its
  real strength is a second permission system beside Guard that Plenipo cannot see inside.
- **Run Ollama's models through another coding program** (`ollama launch`, ADR-017). Rejected for
  now: it changes how that program signs in and bills, and it puts another program between Guard
  and the model.
- **Only OpenRouter, without Ollama.** Rejected: Ollama's flat monthly plan and models on this PC
  are cheaper for steady work; the two carry each other when one runs out.
