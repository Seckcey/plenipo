# ADR-117: Paid AI keys are in the Free edition

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 11A
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Amends:** `docs/editions.md` (one row) and [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md)
  §4 (every AI tool stays in Free)

## In short

Paid AI keys with spending caps ([ADR-085](ADR-085-paid-ai-keys-with-spending-caps.md), Phase 16) are
Free, like every AI tool. A paid key is just another way to pay for an AI model. Spending caps and the
record of every paid task keep money in bounds, and nothing that keeps work in bounds is ever Pro.
Only one row of `docs/editions.md` changes. Phase 16's code is not touched.

## Context

- `docs/editions.md` still says "Your own sign-ins, never API keys". Phase 16's third wave adds paid AI
  keys with spending caps.
- ADR-021 §4: every AI tool, and everything that keeps a worker in bounds, is in the Free edition.
- Another session is building Phase 16's third and fourth waves. The owner asked this phase not to
  change the spending caps, Settings → AI models, or the AI tools page.

## Decision

1. **Free, like every AI tool.** The row becomes "Your own sign-ins, or paid AI keys with spending
   caps (Phase 16)", Yes on Free and Yes on Pro.
2. **Spending caps and the record of paid tasks are outside the edition system entirely,** like Guard
   and the Ledger, so no licensing mistake can weaken them.
3. **No change to Phase 16's code or screens.**

## Consequences

- The edition table matches what the app does.

## Alternatives considered

- **Paid keys as Pro.** Not chosen: it would put a way of reaching an AI model behind Pro, against
  ADR-021 §4.
