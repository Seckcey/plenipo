# ADR-132: The final push — after Phase 22 goes live, Phase 14, then Mac and Linux, then Community

- **Status:** Accepted (the owner's own direction, 2026-10-01: "When that is done we will go in
  this order: 1. Phase 14, Plenipo on your phone. 2. Mac and Linux versions. 3. Community.")
- **Date:** 2026-10-01
- **Phase:** plan change, after Phase 16 Wave 3 (v1.17.0), Phase 11A (v1.18.0), and the paid-key fix (v1.18.1)
- **Amends:** [ADR-039 (the owner's notes and the order of work)](ADR-039-owners-notes-order-of-work.md)
  §1, as already amended by
  [ADR-061 (doing Connections before new AI models)](ADR-061-connections-before-new-ai-models.md)
  and [ADR-080 (building Phase 16's first wave alongside Phase 20)](ADR-080-phase-16-wave-1-alongside-phase-20.md).
  It also changes the **timing** of [ADR-131 (Wave 4: Plenipo's own tools for any model, then
  specialist jobs)](ADR-131-tools-for-any-model.md), not its design.
- **Number:** `main` has ADR-131 (Wave 4's change of course), and no other branch had a higher
  number on 2026-10-01, so the next free number is 132. The session finishing Phase 22 should take
  133 if it needs one.

> **On screen** (ADR-010, plain words and rank names): nothing changes. This record only changes
> which phase is built next, and which phases wait.

## In short

The last stretch of the plan is three phases, in this order:

1. **Phase 14**, Plenipo on your phone.
2. **Phase 23**, Mac and Linux.
3. **Phase 24**, Community.

They start only after **Phase 22's go-live** is done. That is being done in its own session:
a practice purchase in Stripe's test mode, the owner's secret settings, an attorney's read of the
terms, and a second security review. Everything else that is not built yet is **parked**: Phase 16's
Wave 4, Phase 15, and Phase 9. Accepting this record means every session reads the plan's
order-of-work list this way, and no session starts a parked phase on its own.

## Context

After v1.18.1 the plan's order of work (ADR-039, changed by ADR-061 and ADR-080) had these phases
left, in this order: 16 (Wave 4), then 11A and 22, then 14, 15, 9, 23, and 24. Phase 11A is
delivered (v1.18.0). Phase 22 is built and waits on its launch steps (its acceptance report,
section 3). Phase 16's Wave 4 was next by the table, planned by ADR-131 on 2026-09-30.

On 2026-10-01 the owner set the final push. The owner's direction names three phases, in a fixed
order, and says all agents are to follow it.

The three do not need what is parked:

- **Phase 14** needs Phase 13 (delivered), Phase 11A (delivered), the relay 8 West already runs
  for Milepost, and Phase 22 if pairing goes through the account. It does not need Wave 4, Phase
  15, or Phase 9.
- **Phase 23** needs Phase 13 (the installer and updates on Windows, as the model) and a Mac to
  test on.
- **Phase 24** needs Phase 22 (accounts), Phase 14 (the signed-in connection), and Phase 18
  (profiles, delivered), and an attorney's review of its terms.

## Decision

1. **The final push** is these three phases, in this order, after Phase 22's go-live:
   1. Phase 14, Plenipo on your phone (a web interface built from scratch, ADR-040).
   2. Phase 23, Mac and Linux.
   3. Phase 24, Community.
2. **Phase 22's go-live is not part of the final push, and it is not stopped.** It continues in its
   own session and its own private repository, `plenipo-account` (ADR-101). **Phase 14 does not
   start until the owner says Phase 22 is live.**
3. **Parked, not cancelled:** Phase 16's Wave 4 (part 1, tools for any model; part 2, ready-made
   specialist jobs; ADR-131), Phase 16's one small leftover (more Ollama cloud models, which waits
   for the owner's paid plan), Phase 15 (more providers and departments, Windows servers, and
   Milepost), and Phase 9 (Sales on HubSpot, postponed by ADR-018). Each stays in the plan as
   written. Each has **no place in the order of work** until the owner schedules it, the same way
   Phase 9 waited under ADR-018.
4. **Rule §8.3** ("work only on the earliest incomplete phase unless explicitly instructed
   otherwise") means: after Phase 22's go-live, the earliest incomplete phase in the final push.
   A parked phase is never "the earliest incomplete phase", and a session never starts one unless
   the owner tells that session to, in so many words.
5. **One at a time.** Each phase in the final push is finished, with its acceptance report, before
   the next starts. Rule §8.12 stands: each stops at its boundary for the owner's review, because
   Phase 14 reaches the PC from outside and Phase 24 reaches other people.
6. **Only the order changes.** Phase numbers stay, as ADR-039 decided. What each phase delivers,
   and every decision record about it, stays as written. ADR-131's design for Wave 4 stands for the
   day the owner schedules it.
7. **The plan is updated in place:** the order-of-work table and the lines around it in
   `ROLLOUT_PLAN.md`; the status lines of Phases 9, 14, 15, 16, 22, 23, and 24; the project's
   `CLAUDE.md`; `docs/roadmap.md`; and `docs/editions.md`, so the Pro table does not promise the
   Sales department as built.

## Consequences

- The phone web interface, a Pro feature, comes first after the store opens. Mac and Linux, then
  Community, follow.
- Specialist jobs and tools for Ollama, OpenRouter, and direct-key workers wait. Until then those
  workers answer in text only, as they do today.
- Windows servers for 8 West IT's clients (Phase 15), the Milepost connection, and the Sales
  department wait. **Pro is sold without the Sales department.** `docs/editions.md` lists it as
  planned, so no buyer is promised it.
- Phase 14's pairing can go through the 8 West account, because Phase 22 is live first. Phase 24's
  dependencies (22, 14, 18) are all met by the order.
- Phase 14 needs a small change in the relay's own repository, with the owner's approval there.
  Phase 23 needs a Mac and an Apple developer account. Phase 24 needs an attorney. These are the
  owner's to arrange, and each phase's checklist says so first.
- Release numbers follow the order built (`docs/development/versioning.md`).

## Alternatives considered

- **Keep the old order** (Wave 4, then 14, 15, 9, 23, 24). Not chosen: the owner's direction.
- **Do Phase 14 while Phase 22's go-live is still open.** Not chosen: the owner said "when that is
  done", and Phase 14 may pair through the account.
- **Delete the parked phases from the plan.** Not chosen: nothing the owner has decided about them
  is withdrawn, only their place in the order. They stay as written, ready to schedule.
