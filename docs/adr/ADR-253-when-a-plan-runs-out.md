# ADR-253: When a plan runs out, say what you can do, and pick the work back up

- **Status:** Accepted (the owner, first list, kept in Phase 25: "Suggest using a subscription
  reset when model usage limit is reached", and answer 5 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md#the-owners-answers-2026-10-03), accepted
  2026-10-03; item 4.2).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 4 (item 4.2)
- **Number:** after Phase 25's block (ADR-190 to ADR-199), like ADR-250.
- **Amends:** [ADR-037 (background work, and recovery after Plenipo stops unexpectedly)](ADR-037-background-work.md):
  "Nothing runs again by itself" still holds after a crash; work a **usage limit** stopped is now
  picked back up once the limit is over, unless you leave it stopped.

> **On screen** (ADR-010, plain words and rank names): "Claude Code is out until 3:00 PM. 2
> objectives wait for it. You can:" **Wait** ("Plenipo picks the work back up at 3:00 PM."),
> **Use a reset** and **Pick it up now** ("If Anthropic gave you a usage reset, you can use it now
> in Claude."), **Use another AI tool**, **Leave stopped**.

## In short

Before, when an AI tool's plan ran out, the task failed. You got a general notice and a **Try
again now** button on the AI tool's card, and nothing started the work again after the reset.

**Accepting this record means:**

1. **A notice for each AI tool at its limit**, on every page, names the work that waits and when
   it picks back up. Each choice is a button.
2. **Waiting is the default.** Once a minute, each organization looks: when an AI tool's limit is
   over, Plenipo gives each objective it stopped to the same worker again, once, and records it.
3. **Use a reset** opens Anthropic's or OpenAI's own usage page in your browser. **Plenipo never
   uses a reset or buys anything for you.** After you use one, **Pick it up now** tries the AI
   tool at once.
4. **Leave stopped** means that work is never picked up by itself.

## Decision

- **What counts as stopped by a limit:** an objective you gave (a task with no task above it),
  failed, whose last step ended at a usage limit, from the last eight days (the Router's
  look-back). A task one worker handed to another is not picked up by itself: the worker that
  handed it over is told it hit a limit, and decides. Side chats are not picked up.
- **When the limit is over:** the Router's own answer, as for routing: the reset time the AI tool
  reported, or an hour after the limit when it reported none, or sooner when you press **Try again
  now** or **Pick it up now**, or when a later step on that AI tool succeeds.
- **How it is given again:** like **Run again** (Phase 13): an objective to a position goes back to
  that position (its project too), through the normal checks; a conversation in Workers continues.
- **Once each.** Each stopped objective is picked up, left stopped, or run again by you at most
  once (`work.picked_up`, `work.left_stopped`, `plenipo.run_again`). One that cannot be given
  again (the position was removed, say) is recorded with the reason (`work.not_picked_up`) and not
  tried again. If you gave the same objective again yourself meanwhile, the old one is not picked
  up, so the work is never done twice.
- **The reset time it waits for (item 4.3).** The reset in the AI tool's message first; else the
  reset its own plan report gives for the window that is full (Claude Code's "limit reached"
  report during the task; Codex's and Copilot's plan check, asked at once after the limit). The
  latest plan each AI tool reported is kept in one plan book for the PC, since the plan is the
  owner's account in every organization. Only a reset after the limit and within a month is
  believed; with none, an hour, as before.
- **Stop all work** (ADR-199) wins: nothing is picked up until Allow again, and **Pick it up now**
  is refused while work is stopped.
- **The look** keeps only a weak link to its organization, so a closed, archived, or deleted
  organization is never kept open by it.
- **Usage resets that wait.** The owner asked Plenipo to check whether Claude Code or Codex reports
  a reset waiting, and to show "You have a reset waiting" if one does, never guessing.
  - **Claude Code:** its documented `rate_limit_event` says whether the limit is reached, how much
    is used, and when it resets. Nothing in it says a reset is waiting.
  - **Codex:** OpenAI's own app-server documentation that Plenipo could reach does not describe one.
    A third-party note says newer Codex versions report banked resets (`rateLimitResetCredits`,
    `availableCount`), but the exact field could not be confirmed in OpenAI's documentation.
  - So, per [ADR-060 (usage and plan left, only from what each AI tool reports)](ADR-060-usage-plan-left-and-new-models.md),
    Plenipo **only reminds** ("If Anthropic gave you a usage reset…") and shows no count. When
    OpenAI documents the field, reading it is a small change in `codex.rs`.

## Consequences

- **Easier:** work that stopped at 2:00 because a plan ran out finishes by itself after the reset,
  with nothing for you to remember.
- **Harder:** a limit with no reported reset time is tried again every hour; each try that hits the
  limit again costs nothing but a line in the record.
- **Watch for:** an objective you no longer want after a limit: press **Leave stopped**. The pop-up
  notice says so.

## Alternatives considered

- **Use a reset for the owner.** Rejected (the owner's answer 5): using one is the owner's choice,
  on the company's own site or app.
- **Pick up every task a limit stopped, handed-over ones too.** Rejected: the worker that handed
  one over has already been told and may have moved on; restarting it under them could do the work
  twice.
- **Keep a separate timer per limit.** Rejected: the Router already works out when each limit is
  over from the record; a look once a minute reuses it and survives a restart of Plenipo.
