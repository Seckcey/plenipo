# ADR-191: A model you pick by name is your choice — an unknown maker warns instead of refusing

- **Status:** Accepted (the owner's report and direction, 2026-10-03: GitHub Copilot was refused
  as the code reviewer, and "I untcked the never use github in settings … and I still get the same
  error message"; the fix is item 1.6 of [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md),
  which the owner accepted the same day).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 1 (item 1.6)
- **Amends:** [ADR-081 (who made each model)](ADR-081-who-made-each-model.md) §7, for a model the
  owner fixed on a position by name only.

> **On screen** (ADR-010, plain words and rank names): on the position's AI model decision,
> "Plenipo can't tell who made GitHub Copilot (default model), so it can't rule out DeepSeek and
> xAI, which Senior Developer never uses. It keeps your choice."

## In short

Some AI tools pick their own model. GitHub Copilot's "Auto" is one. Plenipo can't tell which
company made the model they pick. Until now, if **any** rule anywhere said "never use" **any**
company, Plenipo refused such a tool even when you chose it yourself. Unticking one company didn't
help, because the lists add up across the organization, the department, the role, and the agent.

**Accepting this record means:** when **you** pick an AI tool and model for a position, Plenipo
keeps your choice. If it can't tell who made the model, it says so and names the companies it
can't rule out. Plenipo still plays it safe when **it** picks a model for you.

## Context

ADR-081 §7 said a model whose maker is not known "plays it safe": it never counts as another
company for review, and a layer with companies never to use keeps it from being chosen. That rule
was applied the same way to automatic choices and to a model the owner fixed on a position
(`Planner::fixed` in `crates/router/src/service.rs`).

Copilot lists no model with a known maker (`crates/runtime/src/agent/copilot.rs`). So on any
organization with a "never use" list, Copilot could not be hired at all. The message named the
level ("Senior Developer has AI companies never to use") but not the companies, and the owner
couldn't find which list to change.

## Decision

1. **A fixed choice is the owner's.** In `Planner::fixed`, a model whose maker is not known is kept
   when a layer has companies never to use. The decision's reason adds one sentence naming the
   companies, in the order the layers list them, and who sets the first list.
2. **A known maker on a "never use" list is still refused**, as before (ADR-081 §5). The owner said
   never; Plenipo knows it would break that.
3. **Automatic choices are unchanged.** A model Plenipo picks for you still skips a model whose
   maker is not known when there are companies never to use, and such a model still never counts
   as another company for review.

## Consequences

- Copilot (and Antigravity, and Ollama models that aren't listed) can be pinned on any position.
- The owner sees the risk in plain words on the position's decision, every time it is shown.
- A pinned tool might still run a company on a "never use" list. That is now the owner's informed
  choice, written on the position.

## Alternatives considered

- **Give Copilot's "Auto" a list of possible makers and block only if one is on the list.** Kept
  for later: Copilot doesn't publish that list in a form Plenipo can check (ADR-081 §8).
- **Only make the message clearer.** Rejected: the owner would still be stuck until they found and
  cleared every list at every level.
