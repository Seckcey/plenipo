# ADR-080: Building Phase 16's first wave alongside Phase 20

- **Status:** Accepted (2026-09-29). The owner's own instruction, accepted with the Phase 16
  design: "Choices 1 to 3 are as recommended", then "yes, I need you to start building. thats the
  whole point of this."
- **Date:** 2026-09-29
- **Phase:** plan change, after Phase 20A (v1.13.0, pull request #96)
- **Amends:** [ADR-061 (doing Connections before new AI models)](ADR-061-connections-before-new-ai-models.md)
  — only for Phase 16's Wave 1, which is built now, beside Phase 20. Waves 2 to 4 keep ADR-061's
  order: after Phase 20. [ADR-067 (Phase 20 in three parts)](ADR-067-phase-20-in-three-parts.md)
  said "Phase 16 starts after"; that now reads "Phase 16's Wave 1 starts now; the rest after".
- **Number:** the owner kept ADR-069 to ADR-079 for Phase 20 (parts 20B and 20C), so Phase 16 uses
  ADR-080 and up. (ADR-069 then went to the website's own record, pull request #98.)

> **On screen:** nothing. This record only changes when part of Phase 16 is built.

## In short

The owner told Plenipo's builder to start Phase 16 (**every AI model worth having**) now, before
Phase 20 (**Connections**) is finished, and to build only its **first wave**: who made each model,
counting that for cross-company review, grouping the model list, exact Claude versions, older
OpenAI models, Google's Gemini CLI (or a written finding; at the owner's direction, Google's
Antigravity CLI in its place), and more Ollama models if the paid plan
is active. Another session builds Phase 20B (Slack and Google) at the same time. Accepting this
record means the order-of-work table shows Phase 16 Wave 1 as **in progress, beside Phase 20**,
and the rule "work only on the earliest incomplete phase" allows it, because the owner said so.

## Context

The order of work ([ADR-039 (the owner's notes and the order of work)](ADR-039-owners-notes-order-of-work.md), changed by ADR-061) is: 13,
17, 18, 19 (delivered), **20 (in progress)**, then **16**. Phase 20 is being built in three parts
(ADR-067): 20A (Microsoft 365) is delivered as v1.13.0; 20B (Slack and Google) is being built now
in another session; 20C (HubSpot, Stripe, WordPress and WooCommerce, add-on tools) comes after.

On 2026-09-29 the owner wrote: "I am telling you to work on Phase 16 now, before Phase 20 is
finished. Rule §8.3 allows this when I say so." Rule §8.3 of `ROLLOUT_PLAN.md` says: "Work only on
the earliest incomplete phase **unless explicitly instructed otherwise**." This is that
instruction.

ADR-061 had rejected building both at once ("two large phases in flight at once make larger
reviews and more merge conflicts"). Wave 1 is small and touches different code than Phase 20:

- **Wave 1 touches** the list of each AI tool's models (`crates/runtime/src/agent/`), the Router
  (`crates/router/`), how Liaison says whose work is being reviewed, Settings → AI models and the
  AI tools page, and, if Google's AI tool passes its checks, a new AI tool.
- **Phase 20 touches** Connections: `crates/capabilities/src/connections/`,
  `crates/guard/src/connections.rs`, the Vault, `apps/desktop/src/settings/connections/`, and the
  Connections tests. Wave 1 stays out of all of these.
- **Both touch** a few shared files: `ROLLOUT_PLAN.md`, `docs/development/versioning.md`,
  `docs/design/vocabulary.md`, the ADR index, the package versions, and `Cargo.lock`. Where both
  change one, the merge keeps both sides.

Wave 1 needs no paid key, no spending cap, and nothing from Phase 20. Waves 2 to 4 (Cursor,
Copilot's second try, spending caps and paid keys, more than one route to a model, OpenRouter,
Hermes) stay after Phase 20.

## Decision

1. **Phase 16 Wave 1 is built now, beside Phase 20.** It is one pull request from its own branch,
   with its own checklist (`docs/phases/phase-16-checklist.md`), acceptance report, and release
   notes. Waves 2 to 4 wait for Phase 20, as ADR-061 set.
2. **Wave 1 is exactly the plan's Wave 1 list** (`ROLLOUT_PLAN.md`, Phase 16, Deliverables):
   - `maker` on every known model: who made it, apart from the AI tool that runs it;
   - cross-company review counts the maker, not the AI tool;
   - the model list grouped by who made it or by the AI tool that runs it, the owner's choice,
     remembered;
   - exact Claude model versions beside the plain names;
   - the older OpenAI models a ChatGPT sign-in really allows, each checked on the owner's PC;
   - Google's Gemini CLI as an AI tool, or a written finding, after step 0 on the owner's PC;
   - added at the owner's direction on 2026-09-29, after Gemini CLI's sign-in was refused on the
     owner's PC ([the finding](../phases/ai-tools-gemini-finding.md)): Google's replacement,
     Antigravity CLI, checked the same way, as an AI tool with its own decision record or a
     written finding. The owner: "gemini is a critical AI LLM we need working in Plenipo so yes,
     do A now please."
   - more Ollama cloud models, only if the owner's paid Ollama plan is active.
3. **Out of Wave 1:** Waves 2 to 4, anything in Connections, and panels and windows (Phase 21).
4. **Staying out of Phase 20's way:** Wave 1 does not change Connections code, the Vault, the
   Connections page, or the Connections tests. If it ever has to, the builder stops and asks the
   owner first. Wave 1's branch merges `main` whenever `main` moves.
5. **Version 1.14.0.** Wave 1 is released as v1.14.0. Phase 20B and 20C were planned as v1.13.1
   and v1.13.2 (ADR-067). The owner plans to merge 20B first. If Wave 1 merges while 20B or 20C
   is still open, those parts move to v1.14.x, and the builder tells the owner before merging.
6. **The order-of-work table** in `ROLLOUT_PLAN.md` shows Phase 16 as "Wave 1 in progress beside
   Phase 20 (ADR-080); Waves 2 to 4 after Phase 20". Phase 20 stays **In progress** until 20C
   merges.

## Consequences

- The owner gets correct cross-company review, the grouped model list, and the extra models
  sooner, without waiting for Slack, Google, and the rest of Connections.
- Two pull requests are open at once. Each merges `main` when the other lands; the shared files
  above can conflict, and each merge keeps both sides.
- Release numbers no longer follow the order of work exactly: v1.14.0 may come before v1.13.1 or
  after it, depending on which merges first. `docs/development/versioning.md` lists the releases
  in the order they merge.
- Rule §8.3 is unchanged. This record is the owner's explicit instruction for one wave, not a new
  rule.

## Alternatives considered

- **Wait for Phase 20 to finish** (ADR-061 as written). Not chosen: the owner's instruction.
- **Build all four waves now.** Rejected by the owner's scope: Waves 2 to 4 bring paid keys,
  spending caps, and new AI tools with their own decision records, which would make two large
  phases in flight, the risk ADR-061 named.
- **Renumber Phase 20B and 20C now** (to v1.14.x) so the numbers follow the merge order. Not done:
  the owner plans to merge 20B first, which keeps 20B as v1.13.1.

## As built

Wave 1 was built on 2026-09-29 as v1.14.0, beside Phase 20B (pull request #100, Slack and Google),
which was still open: no Connections file, the Vault, or Connections test was changed. Shared files
(`ROLLOUT_PLAN.md`, the ADR index, `docs/development/versioning.md`, the word list, the package
versions, and `Cargo.lock`) change by a few lines each, and each merge keeps both sides. At the
owner's direction, Google's Antigravity CLI joined Wave 1 in Gemini CLI's place
([ADR-082 (Antigravity as an AI tool)](ADR-082-antigravity-as-an-ai-tool.md)). More of Ollama's
cloud models wait for the owner's paid plan (a small follow-up).
