# Switches and learning (v1.4.0) — Acceptance Report

|              |                                                                                                                                                                                                 |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Scope**    | On/off switches in Settings (ADR-021) and workers learning from their work (ADR-022), asked for by the owner when accepting ADR-016, ADR-018, ADR-019, and ADR-020                              |
| **Branch**   | `claude/phase-10` (a new pull request after [PR #22](https://github.com/Seckcey/plenipo/pull/22))                                                                                               |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, and the new end-to-end spec against the release build with a real Chromium. GitHub CI: Rust, Frontend, E2E (Linux), Windows on the PR. |
| **Date**     | 2026-09-27                                                                                                                                                                                      |
| **Result**   | Every item in the [checklist](switches-learning-checklist.md) is built and tested. Version **1.4.0**. The owner's Windows check is in the checklist.                                            |

Screenshots (from the end-to-end run in the real app, taken on the build just before the version
number changed, so the header says v1.3.0):

- [Settings → Switches](evidence/switches-learning/switches-settings.png)
- [a new lesson on the Approvals page](evidence/switches-learning/new-lesson.png)
- [a role's lessons, and Learn on its own](evidence/switches-learning/role-lessons.png)

## 1. What the owner asked for → evidence

| Asked for                                                        | Built                                                                                                                                               | Evidence                                                                                                                                                         |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A switch for Plenipo's browser                                   | **Plenipo's browser** (on). Off: Blocked for every role, and workers using it now stop                                                              | `switches_turn_features_off_and_let_website_actions_go_ahead`, `switching_the_browser_off_stops_it`, `switching_a_feature_off_stops_only_its_sessions`           |
| A switch for the screen, mouse, and keyboard                     | **Screen, mouse, and keyboard** (off to start)                                                                                                      | the same Guard test; e2e checks the starting state                                                                                                               |
| A switch for worker learning                                     | **Worker learning** (on)                                                                                                                            | `learning_through_ipc`, `Lessons.test.tsx`, e2e                                                                                                                  |
| Sending, buying, signing in: "on, they don't ask; off, they ask" | Three **without asking** switches (off), on the Allowed list only; Blocked wins; a role set to ask still asks                                       | `switches_turn_features_off_and_let_website_actions_go_ahead`, `switches_send_without_asking_and_screenshots_off`                                                |
| CAPTCHA attempts, then three retries                             | **Declined** (getting past site security). Instead: **Hand me checks that a person is using a website** (on): the owner solves it, the worker waits | `a_person_check_is_handed_to_the_owner`                                                                                                                          |
| A screenshots switch                                             | **Screenshots in the Activity trail** (on). Off: no step pictures; approval cards keep theirs                                                       | `switches_send_without_asking_and_screenshots_off`                                                                                                               |
| Workers learn as they work, "Ask me, switch per role"            | Lessons wait for Keep or Discard; **Learn on its own** per role; kept lessons go to the role's later workers                                        | `workers_learn_lessons_the_owner_keeps`, `lessons_wait_for_the_owner_and_can_be_kept_edited_or_removed`, `a_role_that_learns_on_its_own_keeps_them_at_once`, e2e |
| (safety) a website must not plant a lesson                       | Lessons from tasks that used websites or the screen, or handed work to one that did, always wait                                                    | `lessons_from_websites_always_wait_for_the_owner`, `web_or_screen_use_anywhere_below_a_task_counts`                                                              |

## 2. End to end in the real app

`tests/e2e/specs/learning.e2e.mjs`, against the release binary, with the stand-in AI tools:

1. **Settings → Switches:** the starting states are right; a switch turned off stays off after
   leaving the page and coming back; turned on again.
2. **A lesson:** a Supervisor's answer carries a `plenipo-lesson` block. The sidebar's Approvals
   count shows 1, **Approvals → New lessons** shows the card, and the owner edits and keeps it.
3. **The role's details** show **What it has learned** with the owner's words; **Learn on its
   own** turns on; **Activity → All events** shows "You kept a lesson" and "… now learns on its
   own".

Result: 3 of 3 pass.

## 3. Defects found and fixed

| Defect                                                                                                                                                                   | Fix                                                                                   |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| With sending set to **Blocked**, data a page sent after a click asked the owner instead of being refused (from v1.3.0)                                                   | `decide_held` refuses it                                                              |
| A worker stopped by a switch was told about the emergency Stop                                                                                                           | The refusal checks the switches first and names Settings → Switches                   |
| Workers' instructions told them to stop at a CAPTCHA, so they would never hand it over                                                                                   | The browser note and the Web Assistant's instructions point to `browser_person_check` |
| A Supervisor's lesson could carry what its Web Assistant read on a website, without being flagged; so could a task that saw the screen                                   | The check covers every task handed on from it, and screen use                         |
| The Phase 10 end-to-end test read the new **Plenipo's browser** switch (same label, higher on the Settings page) instead of the browser's status box, and timed out (CI) | The test looks for the status box itself (`div.plenipo-browser`)                      |

## 4. Security notes

- **"Without asking" can be abused by a website.** With a switch on, words on an allowed page
  could lead a worker to send, buy, or press Sign in. Safeguards: Allowed websites only, never
  the mouse and keyboard, Blocked rules and a role's "ask me" still win, everything is recorded,
  and a warning sits above the switches. All three start off.
- **CAPTCHAs are never answered by a worker.** Clicking, typing, and pressing keys in one are
  refused as before. The hand-off gives the page to the owner (the owner's clicks are not a take
  over), and the worker only reads the page again afterwards.
- **Lessons** are at most 3 a task and 300 characters each, secrets are already hidden in every
  answer, and a lesson can never change permissions. Lessons from website or screen work always
  ask. Lessons from files do not force a review (ADR-022, Consequences).

## 5. Deviations

- Not in the rollout plan: both features are recorded in ADR-021 (on/off switches in Settings)
  and ADR-022 (workers learn from their work), both **accepted by the owner** on 2026-09-27.
- ADR-020 (Plenipo's browser and computer use) section 4 said sending, buying, and signing in
  **always** ask. ADR-021 amends it at the owner's request; off, the default, keeps it.
- The owner's "CAPTCHA attempts" switch is not built (ADR-021, Alternatives considered).

## 6. Owner items

| ID  | Item                                                                                                                                                                                                                                                             | Recommendation                                |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| O1  | ADR-021 (on/off switches in Settings) — **Accepted** by the owner on 2026-09-27. It means the switches decide what workers may use, and that with a "without asking" switch on, workers send, buy, or press Sign in on your allowed websites without asking you. | Done; keep the "without asking" ones off.     |
| O2  | ADR-022 (workers learn from their work) — **Accepted** by the owner on 2026-09-27. It means workers write down lessons, you keep or discard each one (or let a role learn on its own), and kept lessons go to that role's later workers.                         | Done.                                         |
| O3  | Windows check (~20 min): the steps in [switches-learning-checklist.md](switches-learning-checklist.md#owner-check-on-windows-20-minutes).                                                                                                                        | Recommended with v1.4.0; fixes go in a patch. |
| O4  | If a role of your own has the **Screen, mouse, and keyboard** permission set, turn that switch on after updating (it starts off).                                                                                                                                | After installing v1.4.0.                      |

## 7. Verification

| Check                                                                     | Result                                      |
| ------------------------------------------------------------------------- | ------------------------------------------- |
| `pnpm check` (versions, format, lint, typecheck, tests)                   | Pass — 157 frontend tests                   |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | Pass                                        |
| `cargo test --workspace`                                                  | Pass — 674 tests                            |
| `learning.e2e.mjs` against the release build (Linux, Xvfb, Chromium)      | Pass — 3 of 3                               |
| Full `pnpm e2e`                                                           | On the PR's E2E (Linux) job                 |
| Generated TypeScript bindings                                             | Up to date (`pnpm bindings` leaves no diff) |
