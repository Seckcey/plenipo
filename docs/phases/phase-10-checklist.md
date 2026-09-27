# Phase 10 — Implementation Checklist

**Status:** complete on `claude/phase-10`. See the [acceptance report](phase-10-acceptance-report.md).
The owner's Windows check with Edge and the real AI tools is below.

Source: `ROLLOUT_PLAN.md`, Phase 10 — Browser Automation, Computer Use, and Advanced Local
Tools. Phase 8 is released as v1.0.0 ([PR #19](https://github.com/Seckcey/plenipo/pull/19)).

On 2026-09-27 the owner asked for three things:

- Postpone Phase 9.
- Give every role clear working instructions.
- Build Phase 10.

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): "Plenipo's browser", "Visit websites",
"Use websites", "See the screen", "Use the mouse and keyboard", "Take over", and "Stop all".

**Goal (plan):** "Add controlled interaction with web applications and the graphical desktop
when structured integrations are unavailable."

## Part 0 — Change of plan (ADR-018, Phase 9 postponed)

- [x] ADR-018 (Phase 9 postponed; no Paperclip; a new Sales department later, built in Plenipo
      on the existing HubSpot account): Proposed
- [x] `ROLLOUT_PLAN.md`:
  - a "Plan changes" note;
  - Phase 9 rewritten as "Sales Department on HubSpot (postponed)", with its new dependencies;
  - Phase 10's dependencies say it runs first;
  - every other Paperclip mention replaced (Phases 0, 2, and 5, the org model, integrations).
- [x] Architecture overview: the integrations row updated

## Part 1 — Every role knows its job (ADR-019)

- [x] Every built-in role has working instructions in four lists: its job, what it hands back,
      what it must not do, and when it asks its lead for help. They go into every worker's
      instructions, with what its permissions let it do and not do.
- [x] Test: every template role states duties, returns, and limits
      (`every_template_roles_instructions_state_its_job_returns_and_limits`)
- [x] Documentation Writer: the built-in Writer set can **save to git**, so its work is
      committed. Pushing still asks. Unchanged installations are upgraded once.
- [x] Researcher: its **Visit websites** permission now has tools, and its instructions match
      them (read only; never forms, sign-ins, buying, or sending)
- [x] Designer: says what it delivers (SVG or PNG, with purpose, size, colors, and fonts) and
      what to do when its model cannot see or make images. Workers are told whether their
      model is marked as able to see images. The Router still prefers such a model.
- [x] VPs and Managers know their job before anyone reports to them (what to do alone, or with
      only an on-call team)
- [x] Custom roles: "What this role does" plus the same four lists, in the owner's words, on
      **New role** and **Edit role** (`update_role`), used the same way
- [x] A new built-in role, **Web Assistant** (the Web assistant permission set)

## Design decisions (details in ADR-020, Plenipo's browser and computer use)

- **Capability order (plan):** official API, then command-line tool, then browser, then the
  mouse and keyboard.
  - Four capabilities: Visit websites, Use websites, See the screen, and Use the mouse and
    keyboard.
  - The **Screen, mouse, and keyboard** set is given to no built-in role.
- **Plenipo's browser:**
  - The installed Edge or Chrome, with its own profile: no sync, no extensions, and never
    saving passwords.
  - Always visible, and controlled over the DevTools Protocol on this computer only.
  - Each worker's step gets its own tab.
- **Website lists** (Settings → Permissions → Websites):
  - Allowed, Blocked, and Other websites (Ask me or Blocked).
  - Addresses on this computer or the local network only when allowed by name.
  - Sites whose terms forbid automated use are blocked to start with.
- **Always the owner's approval:** submitting a form, buying, signing in, and sending. This
  includes what the page's own script sends while a worker acts. The approval card has a
  screenshot of the page.
- **Never:**
  - typing into password, one-time-code, or card fields;
  - typing a secret;
  - clicking or typing in a CAPTCHA;
  - the Windows key.

  The owner signs in, in Plenipo's browser.

- **Screenshots:** after every significant step and before every approval, kept in the Ledger
  and shown in the Activity trail. The worker gets each one as a picture, with a description in
  words.
- **The sign:**
  - a banner on every page, and the footer;
  - the tray menu;
  - a colored frame and label inside the worker's page;
  - a window above all others while the mouse and keyboard are in use.
- **Stop all:** in the app, the tray, and the desktop window. All control halts, those workers'
  permissions end, and nothing restarts until **Allow again**.
- **Take over:** a button, or the owner's own click or key in the page (or moving the mouse on
  the desktop). The worker stops and the tab stays with the owner.

## Deliverables (plan)

- [x] managed browser runtime (Plenipo's browser)
- [x] browser automation capability (Visit websites, Use websites)
- [ ] optional supported browser extension — **not built** (a deviation, recorded in ADR-020
      §10: the DevTools Protocol needs nothing installed, and an extension in the owner's
      browser would reach their sign-ins)
- [x] screenshot/vision pipeline
- [x] computer-observe capability (See the screen)
- [x] computer-control capability (Use the mouse and keyboard)
- [x] domain/application policy (website lists; the mouse and keyboard always ask, with a
      reason)
- [x] user-visible session indicator (the sign)
- [x] emergency stop (Stop all, in the app, the tray, and the desktop window)
- [x] ADR-019 and ADR-020; architecture, README, setup, vocabulary, and versioning updated

## Phase 10 tests (plan)

All against a synthetic website on this computer, stand-ins for the AI tools, and no internet.

- [x] allowed site navigation
- [x] blocked domain
- [x] browser session launch
- [x] screenshot capture
- [x] form interaction in synthetic test environment
- [x] approval-gated submit
- [x] global stop
- [x] timeout
- [x] browser crash
- [x] user takes control
- [x] End to end in the real app (`tests/e2e/specs/browser.e2e.mjs`)

## Acceptance criteria (plan)

- [x] "An authorized agent can complete a controlled browser task in a synthetic environment
      while every significant action appears in the activity trail and the user can
      immediately stop control."

## The owner's rules for Phase 10

- [x] Submitting a form, buying anything, logging in, or sending anything always waits for the
      owner's approval
- [x] Every significant action goes in the Activity trail, with screenshots
- [x] Never bypass CAPTCHAs or site security, never collect passwords, never type a secret the
      worker can see; if a login is needed, the owner does it
- [x] Website terms-of-use risks flagged (Websites settings, ADR-020, and the report §7)

## Out of scope (plan)

Bypassing CAPTCHAs or provider security controls, hidden browser control, unrestricted
credential harvesting, and arbitrary remote surveillance. Also not in this phase:

- an official search API for research;
- HubSpot (the postponed Sales department, ADR-018);
- blurring screenshots;
- noticing the owner's keyboard on the desktop.

## Owner check on Windows (~40 minutes)

This uses Microsoft Edge and the **real** Claude Code and Codex, signed in. It uses one public
test website, **httpbin.org**. It exists for testing web requests: its pizza order form orders
nothing, and only shows back what it received.

1. **Install v1.3.0** and open Plenipo. **AI tools**: Claude Code and Codex are both **Ready**.
2. **Settings → Permissions → Websites**:
   - The warning about websites' terms is shown, and LinkedIn, Facebook, Instagram, X, TikTok,
     and Amazon are on the Blocked list.
   - Add `httpbin.org` to **Allowed** and **Save websites**.
   - Under **Plenipo's browser** it says **Microsoft Edge, with its own profile**.
3. **Open Plenipo's browser** (leave the address empty). An Edge window opens with no
   bookmarks, no sign-ins, and a bar saying it is controlled by automated software. Your own
   Edge is unchanged. Close the new window.
4. **Organization**:
   - **Create a department** `Operations`, then **+ Project** `Web` (no folder).
   - Select **Web Supervisor** and **Hire Web Assistant**.
   - Select the Web Assistant: its details show **What the Web Assistant role does** and
     **Working instructions**.
5. **The task.** Give the Web Supervisor this objective: _"Have the Web Assistant open
   https://httpbin.org/forms/post, fill in the order with customer name Test Owner and a small
   cheese pizza, submit it, and tell me what the page says back."_ Check:
   - A sign on every page says **Web Assistant is using Plenipo's browser**, with **Take over**
     and **Stop all**.
   - The Edge window shows the page with a blue frame and the label "Web Assistant is using
     this browser for Plenipo".
   - The banner **Web Assistant is waiting for your approval** appears. Open **Review**: the
     card says it wants to click **Submit order**, and shows the page's address and a
     screenshot of the filled-in form. **Approve**.
   - The Supervisor's answer reports what the page sent back.
   - **Activity**: the task's trail shows each step (open, type, the approval, click), each
     with **Show screenshot**.
6. **Take over.** Give the same objective again. While the worker is typing, click inside its
   page (or press **Take over**).
   - The sign says **You have control of the browser. Web Assistant stopped.**
   - The frame turns green, and nothing is submitted.
   - Press **Dismiss**, then close the tab.
7. **Stop from the tray.** Give the objective again. When the sign appears, right-click the
   Plenipo icon in the Windows tray and choose **Stop all browser and desktop control**.
   - The sign says **Browser and desktop control is stopped**, and the page's frame turns red.
   - Press **Allow again**.
8. **Blocked.** Give the objective _"Have the Web Assistant open https://www.linkedin.com and
   tell me the page title."_ It is refused; **Approvals → Recently blocked** shows it.
9. **Codex.** In **Settings → AI models**, make Codex the Web Assistant's first choice, and
   repeat step 5. Note whether the answer describes the page (Codex and screenshots are
   unverified; the descriptions in words should be enough).
10. **The mouse and keyboard (optional — take care).** Open Notepad yourself.
    - Create a role **Desk Operator** (**+ Role**), and give it the **Screen, mouse, and
      keyboard** set in **Settings → Permissions → Who may do what**. Hire it under the Web
      Supervisor.
    - Objective: _"Have the Desk Operator look at the screen, take control (reason: typing a
      test line in Notepad), click inside the open Notepad window, type hello from Plenipo
      without pressing Enter, and release control."_
    - The approval card gives the reason: approve it. A small window with **Take over** and
      **Stop** appears above all others, and the text lands in Notepad. Check this with
      Windows display scaling at 125% or 150%, if you use it.
    - Give it again and move your mouse while it works: it stops.
11. Delete the Operations department if you like (**Organization → Operations Manager → Remove
    department**, after archiving the project).

Report anything odd with a screenshot (never a password or token), and it goes into a patch
release.
