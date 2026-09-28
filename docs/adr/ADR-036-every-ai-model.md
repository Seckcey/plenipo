# ADR-036: Every AI model worth having — API keys with spending caps, models by maker and by app, and more than one route to a model

- **Status:** Accepted (by the owner, 2026-09-27). Its place in the order of work moved up, after
  Phase 19, by [ADR-039](ADR-039-owners-notes-order-of-work.md).
- **Date:** 2026-09-27
- **Phase:** 16 (new, after Phase 15)

> **On screen** (ADR-010, plain words and rank names): the owner sees "AI tool", "model", "who
> made it", "how it is paid for", and "spending cap". Never "adapter", "provider", "BYOK", or
> "metered API".

## Context

Plenipo runs five AI tools today: Claude Code (Anthropic), Codex (OpenAI), Grok (xAI), Kimi
(Moonshot), and Ollama, whose cloud models come from DeepSeek, Z.ai, MiniMax, NVIDIA, and OpenAI.
Every one signs in with the owner's own subscription. API keys — passwords that bill per use —
are refused everywhere: in the AI tool rules (ADR-014, adding AI tools ahead of Phase 15), in how
tasks are run (ADR-007 §4), and in the Router, which skips any AI tool that signed in with a key
(ADR-011, how Plenipo picks each worker's AI model).

On 2026-09-27 the owner asked what it would take to reach every AI model and AI company that
Paperclip reaches. Paperclip (`paperclipai/paperclip`, read at commit `0f14d26`) was read for
comparison. It has fifteen ways to connect. Four are the same AI tools Plenipo already has. Two
more are AI companies' own programs Plenipo does not have yet: Google's Gemini CLI and Cursor's
agent. The rest of its long model list comes from API keys, usually through OpenRouter, a middleman
that resells hundreds of models from dozens of companies, or through outside multi-company programs
such as OpenCode and Pi. Paperclip also carries monthly spending caps that warn at 80% and stop the
work at 100%; Plenipo has nothing like that.

The owner's direction, in answer to four questions:

1. **API keys were always planned.** They matter for AI agents, and especially for Hermes Agent
   (Nous Research), where the owner wants specialist workers for specialist jobs such as security
   review.
2. **Subscriptions are available** as needed. The owner has Microsoft 365 with Copilot and a Google
   subscription (both to be confirmed on the PC), a free Cursor account, and will buy the paid
   Ollama plan on 2026-10-01.
3. **No logo count.** The plan's Phase 15 line "Do not add providers merely to increase a logo
   count" stands as an idea, but counting is not the point: the quality of the service is.
4. **Models should be viewable both ways** — by who made the model and by the app that runs it —
   and one model should be reachable by more than one route. The owner's example: when the Kimi
   subscription runs out, use the paid Ollama account for more Kimi work.

Answers 1 and 4 change rules that ADR-003, ADR-007, ADR-011, and ADR-014 set. This is that record.

## Decision

### 1. A quality bar, not a count

ADR-014's five-item bar stays for every AI tool that signs in with a subscription. Phase 15's
sentence is read as it was meant: an AI tool earns its place by being useful, not by adding a
name. Plenipo adds an AI tool when it brings something the owner does not already have — a model
maker, a specialist skill, or a second route to a model the owner already pays for — and refuses one that
only repeats what is there. A tool that fails gets a written finding, as GitHub Copilot did.

### 2. API keys are allowed, off by default, and never without a spending cap

1. **A new switch, off by default:** Settings → Switches (ADR-023) gains "Let workers use paid
   AI keys". While it is off, Plenipo behaves exactly as it does today, and every test that
   forbids keys keeps passing.
2. **Spending caps are built before the first key works.** No key can be saved while no cap
   exists. Caps are set for the whole business, for a department, and for one position. Each cap
   has a monthly amount, a warning at 80%, and a hard stop. At the hard stop Plenipo stops giving
   out that key and says so; it never keeps working and bills on.
3. **Keys live in the Vault** (Windows Credential Manager, `crates/capabilities/src/vault.rs`),
   as server sign-ins already do. Settings keep only a reference: a name, which AI tool may use
   it, and nothing else. Plenipo never shows a key again after it is saved, never writes one to
   the Ledger, and never puts one in a task's activity trail.
4. **A key reaches only the AI tool it was saved for.** The contract suite keeps refusing key
   variables for every subscription AI tool. A paid AI tool declares which variables it needs,
   and only those are passed, only for that tool, only while the switch is on.
5. **Every paid task is priced and recorded.** A task records what was spent, against which cap,
   and which key was used by name. The Ledger is the record, as for every other task (ADR-006).
   Spending that cannot be priced is recorded as "not priced yet" rather than as zero.
6. **Subscriptions still come first.** With the switch on, the Router prefers a subscription route
   over a paid route unless the owner's policy says otherwise for that position. ADR-003's rule —
   never silently turn subscription use into paid use — is kept by making the choice visible: the
   Router's reason names the route and says whether it costs money.

### 3. A model has a maker and a runner, and the owner can look at either

1. **`KnownModel` gains a `maker` field:** who actually made the model, separate from the AI tool
   that runs it. Ollama's `deepseek-v4-pro:cloud` is made by DeepSeek and run by Ollama.
2. **Cross-company review counts the maker.** Today the Router counts the AI tool's company, so
   every Ollama model counts as "Ollama" and a DeepSeek model could review a GLM model as though
   both came from one company. After this change the maker decides. This fixes a real hole in
   ADR-011's cross-company review, and it is the first work of the phase.
3. **The model list can be grouped either way** — by maker or by the app that runs it — and the
   owner chooses which grouping to see. Both groupings show the same models.
4. **An AI tool that runs other companies' models** (Cursor, Copilot, OpenRouter, Ollama) no
   longer needs its own decision record for how it is counted. The maker field answers it. This
   settles the question ADR-014 left open and the Copilot notes raised.

### 4. More than one route to a model

1. **A route is one way to reach one model:** a model, the AI tool that runs it, and how it is
   paid for. Kimi K3 has three possible routes: the Kimi Code subscription, the paid Ollama plan,
   and an OpenRouter key.
2. **A position can list routes in order.** When the first route is usage-limited, signed out, or
   over its cap, the Router moves to the next and says why in its reason. This is the owner's
   example working: Kimi runs out, the paid Ollama account carries the work.
3. **A route is skipped, not retried forever.** A usage limit is remembered with its reset time,
   as today. A route over its spending cap is skipped until the cap resets or the owner raises it.
4. **The owner sees routes as one model with more than one way to reach it,** not as three
   separate models with confusing names.

### 5. What gets added, in four waves

**Wave 1 — fits today's rules, no key needed.**

- The maker field and maker-based cross-company review (§3).
- Exact Claude model versions beside the plain names, and the older OpenAI models that a ChatGPT
  sign-in really allows, each checked on the owner's PC.
- **Google's Gemini CLI.** ADR-014 named it and it was never built. Paperclip passes the task to
  Gemini as a command-line argument, which ADR-007 forbids; Plenipo uses standard input or ACP
  (ADR-015). Step 0 must find a sign-in status check. If Gemini has none, it gets a finding, as
  Copilot did.
- More Ollama cloud models once the paid plan is active, checked against what Ollama really lists.

**Wave 2 — one AI tool, one decision record each.**

- **Cursor's agent.** One sign-in reaches Cursor's own models and Anthropic's, OpenAI's, Google's,
  xAI's, and Moonshot's. It raises the same two questions that stopped Copilot: charges after the
  plan's allowance runs out, and a sign-in status check. Paperclip runs it with every permission
  approved, which Plenipo will not do.
- **GitHub Copilot, second try.** The existing finding names the possible answer: Copilot's
  `--headless --stdio` mode, which reports sign-in and remaining allowance. That needs its own
  record and a change to the AI tool contract.

**Wave 3 — the spending cap work, then paid routes.**

- Spending caps, pricing, and the paid-key switch (§2). Nothing paid is built before this.
- **OpenRouter, through a Plenipo helper,** built the way the Ollama helper was (ADR-017): a small
  client Plenipo supervises per task, Plenipo keeping the conversation, the key from the Vault.
  This is the one piece of work that brings hundreds of models from dozens of companies, and it
  brings Qwen, Mistral, Meta's Llama, and the rest.
- **Direct keys** for Anthropic, OpenAI, xAI, and Google once the helper exists. Each is small.

**Wave 4 — Hermes Agent, for specialist workers.**

The owner wants Hermes for specialist jobs such as security review. Hermes' own program prints
plain text that has to be read with pattern matching, and it takes the task as a command-line
argument; both fail ADR-014 and ADR-007. Hermes also runs an API server of its own that streams
structured events, which is the route Plenipo should check first, the way ADR-017 checked Ollama's
service. Hermes is its own decision record after step 0, or its own finding.

Security work through any AI tool stays inside the owner's own authority: Plenipo's workers do
security review and defensive work on systems the owner or the owner's clients own, and Guard's approvals
and the never-list (ADR-025) still apply. Nothing here loosens that.

### 6. Out of scope

These stay out, and none of them is a gap to close later without a new record:

- **Work that runs on another company's computers** where Plenipo cannot supervise it or apply
  Guard: Cursor Cloud, hosted "managed agent" services, and Amazon Bedrock AgentCore.
- **Claude or any model through a cloud reseller account** (Bedrock, Vertex, Foundry). Plenipo
  blocks these on purpose and keeps blocking them.
- **Gateways into other agent systems** (Paperclip's OpenClaw and Hermes gateways as a general
  connection). They are other orchestrators, not AI companies.
- **Outside multi-company programs** such as OpenCode and Pi. Wave 3's helper reaches the same
  models with Plenipo holding the key and the permissions. Paperclip runs OpenCode with every
  permission allowed; Plenipo will not.
- **Workers that run any program or call any web address** ("process" and "http" in Paperclip).
  ADR-005 allows only approved programs.
- **AI tools loaded while Plenipo runs** (plugins). ADR-014 §7 refused these, and they stay
  refused: adapters are compiled in and reviewed.
- **Scraping sign-ins, unofficial clients, and driving an interactive screen.** Unchanged.

## Consequences

- **The owner can reach far more models,** and the ones the owner pays for can cover each other when a
  subscription runs out.
- **Plenipo can now spend the owner's money.** That is the real change. It is why the spending cap
  work comes before the first key, why the switch is off by default, and why every paid task is
  recorded.
- **Cross-company review becomes correct.** Today it is quietly wrong for Ollama's models.
- **Four decision records still have to change** when Wave 3 lands: ADR-003 (no silent paid
  fallback), ADR-007 §4 (credentials and billing), ADR-011 (API use fixed off), and ADR-014
  (subscription sign-in only). Each is amended by its wave's own record, not rewritten here.
- **More model lists to keep fresh.** Paperclip re-checked its whole catalog on 2026-09-22 and
  needed a written audit to do it. Plenipo needs the same habit; `checked_version()` already shows
  how old each list is.
- **To watch:** each AI company's terms for using a subscription sign-in from another app; where
  prompts are processed, since OpenRouter and some Ollama models send work to companies outside the
  United States; and 8 West IT client data, which must not go to a paid route the owner has not
  approved for it.

## Alternatives considered

- **Stay subscription-only.** Rejected by the owner: keys were always planned, and Hermes'
  specialist workers need them.
- **Add keys now, caps later.** Rejected. A stuck worker on a paid key spends real money fast, and
  a cap added afterwards is a cap that was missing when it was needed.
- **Bring in OpenCode or Pi to get the long model list cheaply.** Rejected: an outside program
  holds the keys and the permissions, and Plenipo cannot see inside it.
- **Count a model's company by the app that runs it** (today's behavior). Rejected: it makes
  cross-company review meaningless for every app that runs other companies' models.
- **One name per route** (for example, three differently named Kimi K3 entries). Rejected: the
  owner would pick between names that mean the same model.
- **Do all of this inside Phase 15.** Rejected: Phase 15 is already department expansion, Windows
  servers, and Milepost. This is its own phase.
