# Switches and learning (v1.4.0) — Checklist

**Status:** complete on `claude/phase-10`. See the [acceptance report](switches-learning-acceptance-report.md).
The owner's Windows check is below.

Phase 10 is released as v1.3.0 ([PR #22](https://github.com/Seckcey/plenipo/pull/22)). When the
owner accepted ADR-016, ADR-018, ADR-019, and ADR-020 on 2026-09-27, they asked for:

- **Learning:** "can we have them learn as they work so they get smarter and smarter on their
  own?" They chose **"Ask me, switch per role"**.
- **Switches in Settings** for Plenipo's browser, the screen, mouse, and keyboard, and worker
  learning.
- **More switches:** sending, buying, signing in, CAPTCHA attempts, and screenshots. Their rule:
  "if it's toggled on, agents don't need to ask. If it's toggled off they have to ask."
- **CAPTCHA attempts, then retries** ("attempt captchas 3 times before passing it off"). Both
  were declined: that is getting past a website's security. A switch to **hand CAPTCHAs to the
  owner** was built instead (ADR-021 §4).

These are not in the rollout plan. They are recorded in two decision records:

- **ADR-021 (on/off switches in Settings)**, which amends ADR-020 (Plenipo's browser and
  computer use).
- **ADR-022 (workers learn from their work)**.

## Switches (ADR-021)

- [x] Guard settings: `Switches` (browser on, desktop off, send, buy, and sign in without asking
      off, CAPTCHAs to the owner on, screenshots on); `set_switches` with event
      `guard.switches_changed`
- [x] A feature switched off: Blocked for every role, with the reason; its sessions stop and
      their approvals are refused (`control.switched_off`); workers are told why they have no
      such tools (`guard.grant_skipped`)
- [x] Without asking: `browser.automate` only, Allowed websites only, the sensitive rule must be
      Ask (Blocked wins), and the role's "ask me" level still asks; the network gate follows the
      same rule
- [x] Fixed: data a page sends is refused, not asked, when sending is set to Blocked
- [x] CAPTCHA hand-off: tool `browser_person_check`, purple sign (mode `handed`), the owner's
      clicks are not a take over, the page comes to the front, the worker continues after
      Approve and stops after Deny or no answer; with the switch off the tool refuses
- [x] Workers never click, type, or press keys in a CAPTCHA (unchanged), and refusal messages
      point to `browser_person_check`
- [x] Screenshots off: steps keep no picture, approval cards keep theirs
- [x] Settings → Switches screen, with the warning above the "without asking" switches and the
      "Always on" note (the sign, Take over, Stop, no passwords, no CAPTCHA attempts)

## Learning (ADR-022)

- [x] Ledger migration 7: table `lessons`; add, keep (optionally edited), discard, remove; part
      of the export
- [x] `plenipo-lesson` blocks read from each worker's answer (at most 3 a task, 300 characters
      each, no duplicates of lessons waiting or kept)
- [x] Lessons wait for the owner unless the role learns on its own
- [x] Lessons from a task that used websites or the screen (itself or any task handed on from
      it) always wait, with a warning
- [x] Kept lessons (the newest 20) and how to write one go into every worker's instructions;
      nothing when learning is off
- [x] Approvals → New lessons (Keep, Discard, edit); a role's details: What it has learned
      (Remove) and Learn on its own; the sidebar count includes waiting lessons
- [x] Settings → Switches → Worker learning
- [x] Activity trail words for every lesson and learning event

## Tests

- [x] Guard: `switches_turn_features_off_and_let_website_actions_go_ahead`
- [x] Control center: `switching_a_feature_off_stops_only_its_sessions`
- [x] Browser, against a real headless Chromium: `switches_send_without_asking_and_screenshots_off`,
      `a_person_check_is_handed_to_the_owner`, `switching_the_browser_off_stops_it`,
      `lessons_from_websites_always_wait_for_the_owner`
- [x] Ledger: `lessons_wait_for_the_owner_and_can_be_kept_edited_or_removed`,
      `a_role_that_learns_on_its_own_keeps_them_at_once`,
      `web_or_screen_use_anywhere_below_a_task_counts`
- [x] Workforce: `lessons_are_read_from_their_block_only`, `workers_learn_lessons_the_owner_keeps`
- [x] Desktop IPC: the switches in `control_and_websites_through_ipc`, and `learning_through_ipc`
- [x] Frontend: `SwitchSettings.test.tsx`, `Lessons.test.tsx`, Activity words
- [x] End to end in the real app: `tests/e2e/specs/learning.e2e.mjs` (3 tests)

## Docs

- [x] ADR-021 and ADR-022 (accepted by the owner on 2026-09-27); ADR-020 notes the amendment; the ADR index
- [x] Architecture overview: the new commands and §12a
- [x] Word list: switches, without asking, checks that a person is using a website, lessons
- [x] README status, versioning guide (1.4.0), release notes `docs/releases/v1.4.0.md`

## Owner check on Windows (~20 minutes)

This uses Microsoft Edge and the **real** Claude Code, signed in, with the Phase 10 setup
(`httpbin.org` on the Allowed list, the **Web** project with its **Web Supervisor** and a **Web
Assistant**).

1. **Install v1.4.0.** Your organization, permissions, and website lists are unchanged.
2. **Settings → Switches.** Check the starting states: Plenipo's browser **On**, Screen, mouse,
   and keyboard **Off**, Worker learning **On**, the three "without asking" switches **Off**,
   Hand me checks **On**, Screenshots **On**. Read the yellow warning.
3. **Asking stays the default.** Give the Web Supervisor the httpbin objective from the Phase
   10 check (_"Have the Web Assistant open https://httpbin.org/forms/post, fill in the order
   with customer name Test Owner and a small cheese pizza, submit it, and tell me what the page
   says back."_). Submitting still asks you. **Approve**.
4. **Without asking.** Turn on **Sending forms and messages** and give the same objective. It
   submits without asking, and the Activity trail shows the click. Turn the switch **off**
   again.
5. **Browser off.** Give the objective again, and while the worker is on the page, turn
   **Plenipo's browser** off. The sign goes away, the worker stops, and its answer says it could
   not finish. Turn the switch back **on**.
6. **A CAPTCHA (optional).** Give the objective _"Have the Web Assistant open
   https://accounts.hcaptcha.com/demo and tell me what the page says after the check."_ (the
   CAPTCHA maker's own demo page). It is not on your Allowed list, so Plenipo first asks to open
   it: approve. Then Plenipo's browser comes to the front with a **purple** sign,
   and the approval asks you to solve the check. Solve it yourself in that window, then
   **Approve** in Plenipo. The worker continues. Workers never answer the check themselves.
7. **Lessons.** After those tasks, check **Approvals → New lessons** (there may be none: most
   tasks teach nothing new). If there is one, it is marked **From a task that used websites**.
   Edit it if you like, then **Keep** or **Discard**. A kept lesson shows in the Web
   Assistant's details under **What it has learned**.
8. **Screenshots off.** Turn **Screenshots in the Activity trail** off, repeat step 3, and check
   that the approval card still has its picture but the trail's steps do not. Turn it back
   **on**.

Report anything that differs; fixes go in a patch release.
