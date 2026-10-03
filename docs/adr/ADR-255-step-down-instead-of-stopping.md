# ADR-255: Step down instead of stopping

- **Status:** Accepted (the owner, first list, kept in Phase 25: "Manage your token use through
  effort levels and model downgrading."; item 4.5 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), with the owner's answer 6: "Stepping
  down is on by default, with every step shown and recorded, and a switch to turn it off").
- **Date:** 2026-10-03
- **Phase:** 25, Wave 4 (item 4.5)
- **Number:** in the 250s with Phase 25's other later decisions (see ADR-250).
- **With:** [ADR-253 (when a plan runs out)](ADR-253-when-a-plan-runs-out.md) and
  [ADR-254 (your subscription first, then the same company's key)](ADR-254-your-subscription-first-then-the-same-companys-key.md):
  they are the last rungs of the ladder.

> **On screen** (ADR-010, plain words and rank names): Settings → Switches → **Step down instead
> of stopping** (On), **Start stepping down at 80% used**. A worker's **Why** ends with "Claude
> Code's plan is 92% used (your line is 80%), so it steps down: Sonnet 5.5 instead of Opus, at
> medium effort."

## In short

Before, a model ran at the effort you set until its plan ran out, and then the work stopped.
Nothing used less of the plan when it was running low.

**Accepting this record means:** when an AI tool's plan is past your line (80% used to start
with), new work steps down a ladder:

1. **Past the line:** one effort level lower (high becomes medium).
2. **Halfway from the line to the limit** (90% for a line at 80%): also the next smaller model
   from the same company: Fable, then Opus, then Sonnet, then Haiku.
3. **At the limit:** the same model on the same company's key, when paid keys are on, a key is
   saved, and the caps have room (ADR-254).
4. **Then it waits** for the reset (ADR-253).

It is **on to start with**, with a switch in Settings → Switches to turn it off, and the line can
be 70%, 80%, or 90%. Every step is shown in the worker's reason and recorded with the worker.

## Decision

- **What "used" is.** The fullest plan window the AI tool reported that has not started again
  (the 5-hour or the weekly), or 100% when it said it is limited (ADR-060: only what each AI tool
  reports). An AI tool that reports nothing never steps down; item 4.6 adds estimates for those.
- **When.** As the Router chooses a model for new work (a new on-call worker, or a full-time
  agent's new conversation). Work already running is not changed, and a full-time agent's
  conversation keeps its model and effort.
- **Which ladder.** Effort: one level lower among those the model takes; for the AI tool's own
  default, medium. A model that takes no effort setting (Haiku) has none. Smaller models: only
  Claude's family is laddered (Fable, Opus, Sonnet, Haiku, linked by the short name each shares,
  ADR-036 §4). Codex's models are not ranked by size in what OpenAI publishes, so Codex steps down
  by effort only.
- **What holds.** Reviewers never step down (their model is chosen for the review: ADR-081). An
  agent you set to its own model (its own rule in Who uses what, or "always use") holds it. A paid
  route never steps down (it is not using a plan).
- **Recorded.** The decision keeps the owner's model (`modelId`) and says how it stepped down
  (`steppedDown`); it is saved with the worker in the Ledger, like every routing choice, and the
  switch's changes are recorded (`router.options_changed`).

## Consequences

- **Easier:** a plan lasts longer before the work stops, and you see each step.
- **Harder:** work near the end of a plan is done by a smaller model or at a lower effort, which
  can be weaker. The line, and the switch, are yours.
- **Watch for:** an AI tool whose report is old. A window that has started again is ignored, so an
  old report never keeps work stepped down past its reset.

## Alternatives considered

- **Step down only after asking each time.** Rejected (the owner's answer 6): on by default, shown
  and recorded, with a switch.
- **Rank Codex's models by price.** Not now: a cheaper model is not always a smaller one, and
  OpenAI's own lists do not say. Effort covers Codex until they do.
