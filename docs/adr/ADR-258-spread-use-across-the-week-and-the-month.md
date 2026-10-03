# ADR-258: Spread use across the week and the month

- **Status:** Accepted (the owner, first list, kept in Phase 25: "Adjust throughout the week or
  month so you don't run out of tokens. … Calculate them all and distribute accordingly."; item
  4.6 of [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md)).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 4 (item 4.6)
- **Number:** in the 250s with Phase 25's other later decisions (see ADR-250).
- **Builds on:** [ADR-255 (step down instead of stopping)](ADR-255-step-down-instead-of-stopping.md),
  whose ladder this paces, and
  [ADR-060 (usage, plan left, and new models)](ADR-060-usage-plan-left-and-new-models.md): only
  what each AI tool reports.

> **On screen** (ADR-010, plain words and rank names): AI tools → **Your plans**: "Week · 45%
> used, ahead of pace (27% by now) · Ahead of pace · Work steps down early to make it last ·
> Starts again Monday", and "Paid AI this month: $3.20 of your $50.00 cap." A worker's **Why**:
> "Claude Code's week is 45% used, ahead of pace (27% by now), so it steps down: medium effort."
> Settings → Switches: "A night hour (8 PM to 8 AM) counts as: half a day hour".

## In short

AI companies don't say how many tokens a plan holds. Claude Code, Codex, and Copilot say how
much of each window is used ("5-hour: 62% used", "Week: 40% used") and when it starts again. So
Plenipo paces by **percent of each window over time**.

**Accepting this record means:**

1. **A fair pace for each window.** By now, a fair share is how far through the window the clock
   is. Day hours (8 AM to 8 PM, Pacific time) count fully; a night hour counts **half** to start
   with (your choice: the same, half, a quarter, or nothing).
2. **Ahead of pace, work steps down early** (ADR-255's ladder): 10 points ahead, one effort level
   lower; 25 points ahead, also the next smaller model. **Behind pace, the best model is used
   freely**, even past your line. On pace, your line holds as before.
3. **Low-priority work** (priority 3 or 4) goes to the plan with the most room left among the
   models you listed. Never for a review, and never onto a paid route.
4. **Your plans**, at the top of the AI tools page: every plan, its pace, when it starts again,
   and paid spending this month.
5. **AI tools that report nothing** can be paced against a **weekly budget of tokens** you set
   there, counted from Plenipo's own records of their tasks since Monday at midnight (Pacific
   time), and marked **"estimated"**.

## Decision

- **The window.** Its length and when it starts again come from the AI tool's report (the
  5-hour window is 300 minutes, the week 10,080). A window with either missing is not paced:
  ADR-255's line alone applies. A window that has started again is left out. A month at most.
- **The margin.** Ahead or behind is 10 points from the fair share; work moves to a smaller model
  at 25 points ahead, or halfway from your line to the limit unless behind pace.
- **Which window decides.** The one that steps work down furthest (the fullest when equal), named
  in the reason.
- **Estimated budgets.** The tokens read and written by that AI tool's tasks in this
  organization since Monday at midnight, Pacific time, against your weekly budget (1 to a
  trillion tokens). Recorded when set or removed (`router.budget_changed`).
- **Night weight.** Kept with the step-down settings (`nightWeight`, 0 to 100, half to start
  with); its changes are recorded like theirs.
- **What doesn't change.** Reviewers and agents set to their own model never step down; a paid
  route never does; the switch "Step down instead of stopping" turns pacing off with it.

## Consequences

- **Easier:** a week of steady work lasts to the reset. In the tests' simulated week, paced work
  ends at 97.5% used; with the line alone, the plan runs out on the week's last task.
- **Harder:** work early in a busy week may run at a lower effort or on a smaller model sooner
  than before. Each step is shown and recorded, and the switch is yours.
- **Watch for:** an estimated budget far from the real plan. It is your guess, and it says
  "estimated" wherever it shows.

## Alternatives considered

- **Count tokens for every plan.** Rejected: the AI companies don't publish a plan's tokens, and
  a guess would override what Claude Code, Codex, and Copilot report.
- **Pace by the clock alone.** Rejected by the plan: day and night weights can be changed, since
  most work happens in the day.
- **Hold low-priority work until a plan has room.** Not now: work waits only at a limit (ADR-253).
  Sending it to the plan with the most room is enough to start with.
