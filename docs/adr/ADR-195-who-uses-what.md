# ADR-195: Who uses what — every model in one menu, one table, "never use" for the whole organization

- **Status:** Accepted (the owner, 2026-10-03: "It only lets you choose the company, not the
  model. I can't use Claude Fable for coding and Claude Sonnet for documentation", and "Plenipo is
  WAY too complicated"; items 2.5 and 2.6 of [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md),
  accepted the same day).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 2 (items 2.5 and 2.6)
- **Amends:** [ADR-041 (model and effort rules in layers)](ADR-041-model-effort-learning-layers.md):
  where the rules are set on screen, and where "AI companies never to use" is set.
  [ADR-081 (who made each model)](ADR-081-who-made-each-model.md) §5: the never-use list is set
  for the whole organization.

> **On screen** (ADR-010, plain words and rank names): Settings → AI models starts with **Who
> uses what**: "Who", "Model", "Backup", "Effort", and "Next worker gets", with a line under each
> role's next worker saying where its model came from ("from Senior Developer's choices", "from
> the whole organization"). Each AI tool's built-in entry is called "Claude Code: its own
> choice". Rarely needed settings are under **More**.

## In short

Picking Fable for one role took three screens and four forms: add Fable to Your models first,
then open the role's choices and add it there. Nothing said so. Model rules lived in two tables,
"never use" lists added up across four levels on three screens, and a model's cost class mattered
only in a case most people never hit.

**Accepting this record means:**

1. **Every model of every AI tool is in the "Add a model" menu** wherever you choose models for a
   role, a department, the whole organization, or one agent. They are grouped by AI tool, your
   subscriptions first, then each paid AI tool whose key works. Picking one that isn't in Your
   models adds it there by itself.
2. **One table, "Who uses what"**: a row for the whole organization, one for each department, and
   one for each role, each with its model, backup, and effort. A role's row says which model its
   next worker gets, where that came from, and why.
3. **"Never use" is set for the whole organization only.** A list set before on a department,
   role, or agent still counts, and is shown under **More** with a **Remove** button for each
   company.
4. **Under More:** a role's "When no models are listed" and "Reviews", a model's cost, and the
   agents with a rule of their own.
5. **"(default model)" is now "its own choice"**: "Claude Code: its own choice". An install from
   before is renamed once (a name you gave is kept).

## Decision

- Only the screens change. The router still applies every saved rule at every level, including
  never-use lists set before, exactly as ADR-041 says.
- The routing data now says which AI tools are paid, so the menus can put subscriptions first.
- The rename happens once at start, recorded as "Each AI tool's default model is now called its
  own choice".

## Consequences

- Fable for the Senior Developer and Sonnet for the Documentation Writer is two picks on one
  screen.
- Fewer things to look at: the organization, departments, and roles are one table.
- A department or role can no longer start a never-use list of its own. Use the whole
  organization's.

## Alternatives considered

- **Keep "never use" at every level.** Rejected: lists that add up across levels on different
  screens were the main reason a choice was refused without the owner knowing why (item 1.6).
- **A separate "Add to Your models" step.** Rejected: it is the step the owner didn't know about.
