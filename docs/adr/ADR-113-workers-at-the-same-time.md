# ADR-113: Workers at the same time — Free's fourth waits its turn; Pro runs four in each organization

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 11A
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Amends:** [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md) and
  `docs/editions.md`: Pro's "Unlimited" workers at once becomes "4 in each organization"

> **On screen** (ADR-010, plain words and rank names): a worker that has to wait shows "**Waiting
> its turn** — Free runs 3 workers at a time. This one starts when one finishes. Pro runs 4 at a
> time in each organization."

## In short

On Free, three workers can be on the job at once across the whole PC. A fourth is not refused: it
waits its turn, with a plain note, and starts by itself when one finishes. On Pro, each organization
runs up to four at once, which is what every copy does today, and the edition table says so honestly.

## Context

- Today every copy runs up to 4 AI tool turns at once in each organization. The limit is set in the
  AI tool programs (`crates/runtime/src/agent/`), which another session is changing for Phase 16.
  The owner asked that area not be touched without asking.
- A worker that cannot start today waits with no note, because Liaison treats "busy" as "try again
  later". The plan says a limit must never be hit silently.
- A Supervisor often hands out several jobs at once. Refusing the fourth would break the Development
  flow that Free must be able to finish (Phase 11A's acceptance).

## Decision

1. **Free: three at once, across the whole PC**
   ([ADR-110](ADR-110-one-person-any-of-their-pcs.md)). A fourth waits its turn with the note above,
   and starts by itself when a place frees up. The note is on the worker's card and in the Activity
   trail.
2. **Pro: four at once in each organization**, today's limit. `docs/editions.md` says "4 in each
   organization" instead of "Unlimited".
3. **Checked before a worker starts,** through `Entitlements::check`, outside the AI tool programs.
   The AI tool programs are not changed.
4. **When Pro ends mid-work,** workers already on the job finish. New ones wait until fewer than three
   are working.
5. **Raising Pro's four** is a later change inside the AI tool programs, made with the owner's
   agreement and that area's owner.

## Consequences

- Free's whole Development flow still finishes, just more slowly when more than three workers are
  needed.
- The Free and Pro page makes no promise the code does not keep.

## Alternatives considered

- **Refuse the fourth worker.** Not chosen: it breaks a Supervisor's normal hand-out of work.
- **Pro unlimited now.** Not chosen: it means changing the AI tool programs during Phase 16's work,
  and too many AI tools at once can swamp a PC.

## As built (v1.18.0)

- **Handed-on work.** A worker past the third waits its turn. Liaison records "Waiting its turn: Free
  runs 3 workers at a time" once, and starts the work by itself when a place frees up.
- **Starting together.** A worker let in counts at once, for up to a minute, so workers starting
  together never pass three.
- **Waiting for a teammate.** A worker waiting for a teammate's answer is not on the job, so a team
  never stops itself.
- **Limit: work the owner starts.** An objective the owner gives, or an AI tool session the owner
  starts, past the third is refused with the same plain words, not queued. The owner starts it again
  when one finishes.
