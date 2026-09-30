# ADR-112: Lessons pause on Free, like Connections

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 11A
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Carries out:** [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md) §3 and
  [ADR-024 (workers learn from their work)](ADR-024-workers-learn-from-work.md): worker learning is
  Pro
- **Follows:** [ADR-068 (Connections are part of Pro)](ADR-068-connections-are-pro.md) §4 (when Pro
  ends, it pauses)

> **On screen** (ADR-010, plain words and rank names): **Worker learning** in **Settings →
> Switches** says "**Part of Pro**" on a Free copy, with what Pro adds, as every Free limit does.
> Kept lessons still show under each role, marked "Paused — part of Pro".

## In short

On a Free copy, workers do not write new lessons, and the lessons already kept are not added to their
instructions. Nothing is deleted: every lesson stays listed and readable, and each agent keeps its
experience. When Pro comes back, lessons work again.

## Context

- Worker learning is Pro (ADR-021, ADR-024), but Phase 11A's list left it out. Today it is on for
  every copy.
- Lessons are used by adding a role's kept lessons, and a short "how to write a lesson" note, to a
  worker's instructions. They are made from a worker's answer when a task finishes.
- ADR-068 settled how Connections behave when Pro ends: they pause, nothing is deleted, running work
  finishes, and they resume when Pro returns.

## Decision

1. **On Free, workers do not write lessons.** The "how to write a lesson" note is not added to their
   instructions, and any lesson in an answer is ignored.
2. **On Free, kept lessons are not used.** They are not added to workers' instructions.
3. **Nothing is deleted or hidden.** Kept lessons stay listed under each role. Lessons still waiting
   for the owner can be kept or discarded. Any lesson can still be removed. Each agent's experience
   score stays.
4. **The owner's choices are kept.** The learning switches for the organization, each role, and each
   agent keep their settings. When Pro returns, they work as the owner left them.
5. **A task that already started keeps its edition.** Plenipo records the edition when a task starts.
   A task that started on Pro keeps its lessons and its Connection tools until it finishes, even when
   it has several steps. This is how ADR-068 §4's "running tasks finish" is carried out, since tools
   are otherwise handed out one step at a time.
6. **One place decides:** learning asks `Entitlements::check` like every other Pro feature.

## Consequences

- A Free copy behaves like a copy with learning switched off, and loses nothing.
- Pro owners who lapse and come back find their lessons exactly as they left them.

## Alternatives considered

- **Keep using lessons already kept, and only stop new ones.** Not chosen: lessons are an ongoing
  benefit, like Connections, and the pause loses nothing.
- **Hide lessons on Free.** Rejected: lapsing never hides anything (ADR-022 §6).
