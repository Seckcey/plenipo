# ADR-061: Doing Connections before new AI models — Phase 20 moves ahead of Phase 16

- **Status:** Accepted (the owner's own choice, 2026-09-28: "The plan's order says Phase 16 is
  next. I am choosing to do Phase 20 first.")
- **Date:** 2026-09-28
- **Phase:** plan change, after Phase 19 (v1.12.0, pull request #94)
- **Amends:** [ADR-039 (the owner's notes and the order of work)](ADR-039-owners-notes-order-of-work.md)
  §1 — the order-of-work table. Nothing else in ADR-039 changes.
- **Number:** `main` has ADR-060 (usage, "plan left", and new models, accepted 2026-09-28), so
  the next free number is 061.

> **On screen** (ADR-010, plain words and rank names): nothing changes. This record only changes
> which phase is built next.

## In short

Phase 20 (**Connections**: Microsoft 365, Slack, Google, and more) is built next. Phase 16
(**every AI model worth having**: paid AI keys, spending caps, new AI tools) comes right after
it. Accepting this record means the plan's order-of-work table shows Phase 20 as **Next** and
Phase 16 as the phase after it, and the rule "work only on the earliest incomplete phase" reads
that new order.

## Context

ADR-039 set the order of work on 2026-09-28: 13, 17, 18, 19, then **16**, then **20**. Phases 13,
17, 18, and 19 are delivered (v1.9.0 to v1.12.0). By that order, Phase 16 is next.

On 2026-09-28, after Phase 19 merged, the owner chose to build Phase 20 first. The two phases do
not depend on each other:

- **Phase 20 needs** Guard (Phase 7), Plenipo's browser (Phase 10), the Vault, and Microsoft app
  registration by 8 West. All of these exist or are the owner's own step. It does not need paid
  AI keys, spending caps, or any new AI tool.
- **Phase 16 needs** nothing from Phase 20. Its paid keys fill the "How it is paid for" switch
  that Phase 19 put on each AI tool's card. That switch stays shown and locked until Phase 16,
  exactly as Phase 19 left it.
- **Phase 9** (Sales on HubSpot) already waits for Connections (ADR-039 §3). It keeps its place,
  after Phase 20.

## Decision

1. **The order of work changes to:** 13, 17, 18, 19 (all delivered), **20 (next)**, **16**, 21,
   11A + 22, 14, 15, 9, 23, 24.
2. **Only the order changes.** Phase numbers stay, as ADR-039 decided. What each phase delivers,
   and every decision record about Phase 16 (ADR-036, every AI model worth having), stays as
   written.
3. **Rule §8.3** ("work only on the earliest incomplete phase") means the earliest incomplete
   phase in the new list, so Phase 20 is the one to work on now.
4. **What stays locked meanwhile:** the paid-key switch on each AI tool's card stays locked, and
   the Router keeps refusing AI tools signed in with a paid key (ADR-011, ADR-014 §7), until
   Phase 16 is built.
5. **The plan is updated in place:** the order-of-work table in `ROLLOUT_PLAN.md` and the
   Phase 16 and Phase 20 "place in the order of work" lines.

## Consequences

- Workers can use the business's own mail, calendar, files, and chat sooner, through Guard.
- New AI models and paid keys wait one phase longer. Nothing already delivered changes.
- Phase 20's records are written against today's AI tools (Claude Code, Codex, Grok, Kimi, and
  Ollama). Phase 16 adds new AI tools later; each new tool that takes Plenipo's tools gets the
  Connections the same way, through Plenipo's tool server, with no extra work in Phase 20.
- Release numbers follow the order built: Phase 20 is v1.13.0 (or v1.13.x per part, if it is
  split), and Phase 16 takes the next minor version after it.

## Alternatives considered

- **Keep the order (Phase 16 next).** Not chosen: the owner's choice.
- **Build both at once.** Rejected: two large phases in flight at once make larger reviews and
  more merge conflicts, and the plan's rule is one phase at a time.
