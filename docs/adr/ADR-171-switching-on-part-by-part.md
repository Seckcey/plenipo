# ADR-171: Switching Community on part by part

- **Status:** Accepted (2026-10-02). The owner asked the builder to propose switching the people part
  on as soon as the attorney approves and the security review passes, with linked organizations and
  collaborators switching on as each lands. The builder recommended it with a security review before
  each part and a switch for each part on the account service; the owner answered "**as
  recommended**".
- **Date:** 2026-10-02
- **Phase:** 24 (Community), part 24C
- **Amends:** [ADR-160 (building Phase 24 alongside Phase 23)](ADR-160-phase-24-alongside-phase-23.md)
  §7 (one security review before anything reaches real people: now one before **each** part), and
  part **24F** of [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md) (one launch: now three).
- **Builds on:** [ADR-170 (Community follows 8 West's switch)](ADR-170-community-follows-8-wests-switch.md)

> **On screen** (ADR-010, plain words and rank names): **Coming soon** on **Link with this
> organization** and **Invite a helper** until their part opens.

## In short

Community has three parts: **people** (profiles, the directory, messages, block, report, leave, and
points), **linked organizations**, and **collaborators**. Each is switched on by itself, as soon as
it is ready: the people part when the attorney approves the texts and the security review of the
people part passes; linked organizations and collaborators each when its own code is released and
its own review passes. The owner turns each switch on at the account service; Plenipo follows
(ADR-170).

## Context

- ADR-161 planned one launch (24F) after all of Phase 24 was built, with one security review
  (ADR-160 §7).
- The people part (24C) needs nothing from linked organizations (24D) or collaborators (24E). Those
  two bring other people's requests through Guard, the riskiest code in Phase 24.
- The account service already has all three parts, and one switch for all of them. With one switch,
  turning on the people part would also open links and collaborations at the service before
  Plenipo's side of them is built and reviewed.

## Decision

1. **The people part** (part 24C) is switched on when both are done:
   - the attorney has approved the texts, and they are published on the website
     (`apps/website/legal/`) and on the account site; and
   - the security review at the highest effort has passed, for Plenipo's people part and for the
     account service's Community.
2. **Linked organizations** (part 24D) are switched on when their code is in a release and their own
   security review at the highest effort has passed. **Collaborators** (part 24E) the same.
3. **A switch for each part on the account service.** While a part is closed, everything that belongs
   to it answers `not_open` (contract §1), and the service makes none of its notices. "Is Community
   open?" says which parts are open, and Plenipo shows **Coming soon** on that part's buttons.
4. **Before each switch goes on,** 8 West sets the lowest version of Plenipo allowed in Community to
   the release that review passed, if it is newer (ADR-170 §5).
5. **The owner turns each switch on.** Plenipo's builder never does.
6. **The acceptance scenario** for each part (ADR-161 §3, two test accounts on a test copy of the
   account service) runs before that part is switched on. The acceptance report is written when all
   three parts are on.
7. **The published terms cover all of Community,** including the parts that open later, because the
   attorney reviews them together; they say that some parts open later.

## Consequences

- People can meet and talk in Plenipo sooner, without waiting for linked organizations and
  collaborators.
- Three security reviews at the highest effort instead of one. Each is smaller.
- The account service gets two more switches (the list in
  [`docs/phases/phase-24-account-service-changes.md`](../phases/phase-24-account-service-changes.md)).
- The **Getting started** list cannot be finished, and its 10 points cannot be earned, until both
  later parts are open, because it includes a link and a collaboration (contract §12;
  [ADR-172 (what part 24C changes)](ADR-172-what-part-24c-changes.md) §4).

## Alternatives considered

- **Everything at once, at the end** (ADR-161 as written). One review and one launch, but people wait
  for the riskiest parts before they can even send a message.
- **One switch at the service, and Plenipo hides what is not ready.** A copy that is not Plenipo could
  still use the closed parts at the service; the service's own switch is the lock that counts.
