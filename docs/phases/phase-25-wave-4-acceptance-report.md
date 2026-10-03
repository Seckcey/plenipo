# Phase 25 — Acceptance Report (Wave 4)

|              |                                                                                                                                                                                                       |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 25 — Fixes and a simpler Plenipo before launch, **Wave 4** ("make your AI plans last, and catch made-up answers")                                                                                     |
| **Branch**   | `claude/relaxed-mccarthy-uxyguw` ([PR #156](https://github.com/Seckcey/plenipo/pull/156)), commits `2ec1aad` to `402d10a`                                                                             |
| **Verified** | `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff), and `pnpm docs:check`, here. GitHub CI on the pull request, Windows and the E2E included. |
| **Date**     | 2026-10-03 (Pacific time)                                                                                                                                                                             |
| **Result**   | All eight items built. One part of 4.8, leads stopping their team mid-task, came in its own pull request after the waves merged (section 4). Plenipo is made by 8 West Ventures, LLC.                 |

## In short

Wave 4 makes your AI plans last and catches made-up answers. Plenipo caches what it sends to
Anthropic models, tells you when a plan runs out and picks the work back up, shows plan numbers in
plain words, moves to your key only when you allow it, steps down to a smaller effort or model
instead of stopping, spreads use across the week, and checks every answer against what really
happened. **What you need to do:** run a real week (section 3). Leads now stop their team
mid-task, as you decided (section 4).

## 1. Items → result

| #   | Item (the checklist)                                       | Result   | Evidence                                                                                           |
| --- | ---------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------- |
| 4.1 | Prompt caching on every Anthropic model                    | **Done** | `2ec1aad`; Usage shows what caching saved; ADR-252.                                                |
| 4.2 | When a plan runs out: say what you can do, pick it back up | **Done** | `ccab4c2`: the notice and its buttons; work picked back up after the reset; ADR-253.               |
| 4.3 | Better plan numbers                                        | **Done** | `ba448a4`: "5-hour: 62% used, resets 3:00 PM"; a limit waits for the reported reset.               |
| 4.4 | Your subscription first, then the same company's key       | **Done** | `ce59e92`: only with paid keys on, a key saved, and room under your caps; ADR-254.                 |
| 4.5 | Step down instead of stopping                              | **Done** | `64c4ff1`: lower effort, then a smaller model, then your key, then wait; on by default; ADR-255.   |
| 4.6 | Spread use across the week and the month                   | **Done** | `402d10a`: a fair pace per window, **Your plans**, estimated budgets; ADR-258.                     |
| 4.7 | Catch made-up answers, step 1                              | **Done** | `f0059f2`: Plenipo's record under every answer, four plain checks, sent back once; ADR-256.        |
| 4.8 | Catch made-up answers, step 2                              | **Done** | `2205c21`: links checked, leads send work back, a notice on repeat failures; ADR-257.              |
|     | ↳ leads stop their team mid-task (the owner, 2026-10-03)   | **Done** | In its own pull request, `claude/leads-stop-their-team`: check-ins and stops; ADR-259 (section 4). |

The tests for each item are named in the [checklist](phase-25-checklist.md). Two worth naming:
the router's `a_simulated_week_keeps_the_pace_and_lasts_to_the_reset` (paced, the week ends at
97.5% used; with the line alone, the plan runs out on its last task), and the Liaison test
`an_answer_that_doesnt_match_the_record_is_sent_back_once`.

## 2. Decisions

- [ADR-252 (prompt caching for Anthropic models)](../adr/ADR-252-prompt-caching-for-anthropic-models.md)
- [ADR-253 (when a plan runs out)](../adr/ADR-253-when-a-plan-runs-out.md)
- [ADR-254 (your subscription first, then the same company's key)](../adr/ADR-254-your-subscription-first-then-the-same-companys-key.md)
- [ADR-255 (step down instead of stopping)](../adr/ADR-255-step-down-instead-of-stopping.md)
- [ADR-256 (check answers against what really happened)](../adr/ADR-256-check-answers-against-what-really-happened.md)
- [ADR-257 (catch made-up answers, step 2)](../adr/ADR-257-catch-made-up-answers-step-2.md)
- [ADR-258 (spread use across the week and the month)](../adr/ADR-258-spread-use-across-the-week-and-the-month.md)
- [ADR-259 (leads stop their team mid-task)](../adr/ADR-259-leads-stop-their-team.md)

## 3. The owner's checks (Windows 11)

- Over one real week, no plan runs out before its reset (AI tools → **Your plans** shows the pace).
- Every step down shows (the worker's **Why**) and is recorded (Activity).
- A worker's false "tests passed" is caught and sent back (a handoff's reply says "Doesn't match
  the record").
- The Usage tab shows what caching saved.
- The cost math for caching matches Anthropic's prices on your PC (ADR-081 §8).

## 4. Leads stop their team mid-task (built after the waves)

- **What changed.** On 2026-10-03 you decided that supervisors, managers, and VPs can stop their
  team while it works. It came in its own pull request after Phase 25's waves merged, with
  [ADR-259 (leads stop their team mid-task)](../adr/ADR-259-leads-stop-their-team.md). It
  replaces ADR-257's "leads send work back; stopping stays yours" and changes how leads wait
  (ADR-008).
- **Check-ins.** While its team works, a lead is woken for a short check-in: when an answer comes
  back while other work still goes, and after a long wait (20 minutes). At most 4 a round, and
  none while Stop all holds the work.
- **What it sees.** Who is working on what, and for how long, in Plenipo's own words, never
  words a worker wrote. Also the answers already back.
- **What it can stop.** Only work it handed on itself, with a reason. It is stopped the same way
  Plenipo stops any cancelled request: a full-time worker keeps its conversation.
- **What you see.** Activity says "Stopped by Website Supervisor: …", and Handoffs says the same.
  Your Stop and Stop all are unchanged.
- **Your check (Windows 11):** give a supervisor an objective with two parts, one quick and one
  long. When the quick one comes back, the supervisor can stop the long one if it no longer makes
  sense. Activity on the long one says who stopped it and why.

## 5. Also in this wave

- **Activity → All events: Show older events** (`c5502b1`). The list shows the newest 200 events;
  after `main` was merged, the phone test's first requests were older than that. You can now see
  further back, and the test presses the button like a person would.
- **Tests whose stand-in reviewers gave no verdict** (`0d44d4d`, `8ba3203`): with 4.7, a review
  without a verdict is sent back, so those reviewers now give one, as real reviewers are told to.

## 6. Fixed after the security review of #156

An independent security review found no blockers and three things to fix. All three are fixed,
with tests (the checklist names them).

- **Link checks (4.8) reach only where the worker could look.** A worker wrote the link, so Plenipo
  now looks only where that worker could have looked itself without asking: a website only with
  "Visit websites" or "Use websites" allowed, and a pull request only with "Read GitHub" allowed,
  in its project's repository. Only https, never a link with a "?" part, only while the browser
  is on, and an answer's check waits six seconds at most for all its links
  ([ADR-257 (catch made-up answers, step 2)](../adr/ADR-257-catch-made-up-answers-step-2.md)).
- **Stop all lasts across a restart** (Wave 3, item 3.4).
- **The three template commands** are in the test that refuses other windows and web pages
  (Wave 2, item 2.8).
- Also asked for: work a usage limit stopped that can't be given back (its worker was archived or
  deleted) now makes a notice that says why in plain words (4.2).
