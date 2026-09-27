# Phase 10 — Acceptance Report

|              |                                                                                                                                                                                                                                                                  |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 10 — Browser Automation, Computer Use, and Advanced Local Tools (with Part 0, Phase 9 postponed, and Part 1, every role knows its job)                                                                                                                           |
| **Branch**   | `claude/phase-10` ([PR #22](https://github.com/Seckcey/plenipo/pull/22))                                                                                                                                                                                         |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, full `pnpm e2e` against the release build with a real Chromium. GitHub CI: Rust (with the runner's Chrome), Frontend, E2E (Linux), Windows — see PR #22.                                                |
| **Date**     | 2026-09-27                                                                                                                                                                                                                                                       |
| **Result**   | The acceptance criterion and all ten Phase 10 tests pass against a synthetic website, stand-ins for Claude Code and Codex, and no internet; the owner's four rules hold. Version **1.3.0**. The owner's Windows check with Edge and the real AI tools is §9, O4. |

Screenshots (from the end-to-end run in the real app):

- [Settings → Permissions → Websites](evidence/phase-10/websites-settings.png)
- [the sign while a worker uses the browser](evidence/phase-10/control-banner.png)
- [sending a form waits for approval, with a screenshot of the page](evidence/phase-10/browser-approval-card.png)
- [the Activity trail, each step with its screenshot](evidence/phase-10/browser-trail.png)
- [Take over](evidence/phase-10/take-over.png)
- [the emergency Stop](evidence/phase-10/emergency-stop.png)

Test totals: **RUST_TOTAL Rust** (Linux, including 12 browser and computer-use integration
tests against a real headless Chromium) · **FRONTEND_TOTAL frontend** · **E2E_TOTAL
end-to-end** against the real release binary (6 Phase 1 + 6 Phase 2 + 8 Phase 3 + 5 Phase 4 + 5
Phase 5 + 5 Phase 6 + 4 Phase 7 + 4 Phase 8 + 6 Phase 10).

On screen the plan's managed browser is **Plenipo's browser**, its domain policy is the
**website lists**, its session indicator is **the sign**, its emergency stop is **Stop all**, and
"user takes control" is **Take over** ([word list](../design/vocabulary.md)). Quotes from the
plan keep the plan's words.

CI has no AI tool accounts, and tests never touch the internet.

- **The AI tools** are `plenipo-fake-agent`, installed as `claude` and `codex`. It follows a
  script and calls Plenipo's browser and screen tools through the real tool relay, like a real
  AI tool would.
- **The websites** are a small synthetic website on `127.0.0.1`:
  - a shop, a contact form, a sign-in page, a checkout, and a CAPTCHA;
  - a page whose script sends data, and one that sends a form by itself;
  - a redirect to a blocked site, and a page that never finishes loading.

  In the integration tests the browser reaches it under made-up names (`shop.test`,
  `blocked.test`, `other.test`), mapped to `127.0.0.1` inside the browser.

- **The browser** is a real Chromium: headless in the integration tests, with a window (under
  Xvfb) in the end-to-end tests.
- **The desktop** in tests is `SyntheticDesktop`. It records every click and key instead of
  moving the real mouse.

## 1. Acceptance criterion → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                                                                                                   | Result (stand-ins) | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | An authorized agent can complete a controlled browser task in a synthetic environment while every significant action appears in the activity trail and the user can immediately stop control. | **Pass**           | Integration `plan_form_interaction`: a Web Assistant (permission set Web assistant) opens the contact form, fills in three fields, and sends it after the owner's approval. Its seven actions are in the trail (`capability.used`), each with a screenshot, and the site gets the form once, only after approval. `plan_global_stop`: **Stop all** halts it at once; the waiting send is refused, nothing is sent, and control stays stopped until **Allow again**. E2E `acceptance: sending a form waits for approval…` and `the emergency Stop halts all control…` do the same in the real app: [approval](evidence/phase-10/browser-approval-card.png), [trail](evidence/phase-10/browser-trail.png), [Stop](evidence/phase-10/emergency-stop.png). |

## 2. Required Phase 10 tests → evidence

`crates/capabilities/tests/browser.rs` runs the whole stack with a real Chromium:

- Workforce, Router, and Liaison;
- the agent runtime and supervisor;
- Guard, and the broker with its tool server and relay;
- a file-backed Ledger.

The organization is **Operations → Web tasks**. A Web Supervisor leads a Web Assistant, a
Researcher, and a Desk Operator (a custom role with the Screen, mouse, and keyboard set). The
website lists are: `shop.test` allowed, `blocked.test` blocked, and other websites ask.

| Test (ROLLOUT_PLAN.md)                         | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ---------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| allowed site navigation                        | **Pass** | `plan_allowed_site_navigation`: an allowed page opens at once in the worker's own tab. The worker gets the page in words, marked as the website's (title, text, and links with references). The trail has `browser_open` (Visit websites) with the page's address and a JPEG screenshot, and no refusal. The session shows while it lasts and ends with the step. E2E: the Web Assistant opens the allowed test site.                                                                  |
| blocked domain                                 | **Pass** | `plan_blocked_domain`: a blocked website is refused (`guard.denied`, rule layer) and a redirect to it is stopped. A website on neither list waits for the owner (the card names the page), opens once approved, then opens again without asking for the rest of the step. Nothing is sent. Also Guard's `websites_are_checked_against_the_lists` and `local_addresses_need_an_allowed_entry`. E2E: `localhost`, on the blocked list, never opens and shows under **Recently blocked**. |
| browser session launch                         | **Pass** | `plan_browser_session_launch`: the first browser call starts Plenipo's browser as a supervised program (in the Runtime overview) with its own profile in Plenipo's data folder. That profile has password saving and autofill off, and the start is recorded (`browser.started`). The worker's tab closes when its step ends.                                                                                                                                                          |
| screenshot capture                             | **Pass** | `plan_screenshot_capture`: `browser_screenshot` gives the worker a JPEG picture and a description in words. It is kept as a Ledger `screenshot` artifact with its SHA-256, and `screen_view` does the same for the screen. `get_screenshot` serves only files in Plenipo's screenshot folder that the Ledger recorded.                                                                                                                                                                 |
| form interaction in synthetic test environment | **Pass** | `plan_form_interaction`: typing goes ahead, and clicking **Send message** waits for the owner (Use websites; Sending or publishing outside this computer), with a screenshot of the filled-in form. After approval the site receives the form once. Typing into the sign-in page's password field is refused, and so is clicking the CAPTCHA.                                                                                                                                          |
| approval-gated submit                          | **Pass** | `plan_approval_gated_submit`: **Buy now** asks as buying, and once refused, nothing reaches the site. Data a page's own script sends after a harmless-looking click is held for the owner. A form a page sends by itself is stopped.                                                                                                                                                                                                                                                   |
| global stop                                    | **Pass** | `plan_global_stop`: **Stop all** stops the session. The worker's waiting request and its later calls are refused, its permissions are revoked, and nothing is sent. A new worker cannot use the browser or the screen until **Allow again**, after which it can. `control.stopped` and `control.allowed` are recorded. Unit `the_listener_hears_changes_in_order`: the sign never shows an older status after a Stop.                                                                  |
| timeout                                        | **Pass** | `plan_timeout`: a page that never finishes loading is stopped at its 5-second limit. The worker is told, and the next page opens normally.                                                                                                                                                                                                                                                                                                                                             |
| browser crash                                  | **Pass** | `plan_browser_crash`: the browser is killed while a worker uses it. The worker's next call says its tab is gone, and the next page it opens starts the browser again (`browser.started`, restarted).                                                                                                                                                                                                                                                                                   |
| user takes control                             | **Pass** | `plan_user_takes_control`: the owner clicks in the worker's page. The worker stops (its next browser call is refused), the tab stays open for the owner, and the trail says "you clicked or typed in the page". On the desktop, moving the mouse does the same ("you moved the mouse"), and held keys and buttons are let go. E2E: **Take over** stops the worker and nothing more is sent; the sign says so until dismissed ([take over](evidence/phase-10/take-over.png)).           |

Also:

- `computer_use_asks_first_and_never_types_secrets`:
  - no mouse or keyboard before taking control, and taking control asks with the worker's
    reason;
  - Enter asks again, a secret is never typed, and the Windows key is refused;
  - coordinates are the screenshot's.
- `the_researcher_reads_but_cannot_click`: the Researcher reads but gets no click tools, and a
  worker without browser permissions gets no browser tools.

Unit tests cover:

- **Guard:** website entries and matching, and the upgrade from v1.0 (lists and sets added
  once).
- **The browser:** click and submit classification (sending, buying, signing in), the
  browser's options and profile, and the tab's time limits.
- **The rest:** the control center, the screenshot store, and the desktop stand-in.

IPC boundary tests cover the new commands (input validation, refused extra fields, and denial
for ungranted windows and remote origins). They also check the indicator window's narrow grant.

Frontend tests cover:

- the sign with Take over, Stop all, Allow again, and Dismiss, and ignoring an older update;
- the Websites settings and opening Plenipo's browser;
- the approval card's page and screenshot, and the new Activity words;
- writing and editing a role's instructions.

All earlier phases' tests pass.

## 3. The owner's rules → evidence

| Rule                                                                                                                                    | How                                                                                                                                                                                                                                                                                                                                                                                                            | Evidence                                                                                                                        |
| --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Submitting a form, buying anything, logging in, or sending anything always waits for my approval.                                       | Before a click or key press, Plenipo decides whether it submits, buys, signs in, or sends, and those always ask. While a worker acts, any request that is not a plain page read is held for the owner. A form a page sends by itself is stopped. These kinds can be **Ask** or **Blocked**, never allowed without asking (sending; buying; signing in and taking control of the mouse and keyboard, both new). | `plan_form_interaction`, `plan_approval_gated_submit`; `crates/capabilities/src/browser/classify.rs` unit tests; E2E acceptance |
| Every significant action goes in the Activity trail, with screenshots.                                                                  | Every browser and screen action records `capability.used` with a screenshot, taken after the action. Every approval request has a screenshot of the page, taken before. Refusals are `guard.denied`. Control changes are `control.*`. The trail shows the pictures.                                                                                                                                            | Every `plan_*` test checks the trail; E2E [trail](evidence/phase-10/browser-trail.png)                                          |
| Never bypass CAPTCHAs or site security, never collect passwords, never type a secret the worker can see; if a login is needed, I do it. | CAPTCHA widgets are recognized, and clicking or typing in one is refused. Password, one-time-code, and card fields are never typed into. Text containing a Vault secret is refused (workers never see secret values). Plenipo's profile never saves passwords. The owner signs in via **Open Plenipo's browser**. Workers' instructions say to stop and ask.                                                   | `plan_form_interaction`, `computer_use_asks_first_and_never_types_secrets`, `plan_browser_session_launch`                       |
| Flag website terms-of-service risks.                                                                                                    | The Websites settings warn, and sites whose terms forbid automated use start blocked. See §7 and ADR-020.                                                                                                                                                                                                                                                                                                      | [Websites](evidence/phase-10/websites-settings.png)                                                                             |

## 4. Parts 0 and 1 → evidence

| Item                                                                  | Result   | Evidence                                                                                                                                                                                                                                                             |
| --------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Phase 9 postponed; no Paperclip; a Sales department later, on HubSpot | Done     | ADR-018 (Proposed); `ROLLOUT_PLAN.md` (plan-changes note, Phase 9 rewritten and postponed, Phase 10 dependencies, every Paperclip mention replaced); architecture overview                                                                                           |
| Every built-in role's instructions state duties, returns, and limits  | **Pass** | `every_template_roles_instructions_state_its_job_returns_and_limits` (`crates/workforce/src/prompt.rs`)                                                                                                                                                              |
| The known gaps                                                        | **Pass** | `the_known_gaps_are_covered`: the Researcher reads only; the Designer's deliverables and what to do without images; VPs and Managers alone. `scenario_documentation_only_change` (`tests/development.rs`) now shows the Documentation Writer's change **committed**. |
| Custom roles in the owner's words                                     | **Pass** | `a_custom_roles_own_words_are_used_the_same_way`; `a_custom_role_gets_its_owners_working_instructions` (Workforce integration); frontend role dialog test                                                                                                            |

## 5. Defects found and fixed during Phase 10

| Found by          | Problem                                                                                                            | Fix                                                                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| Integration tests | Reading the page hung while the browser held a page load waiting for the owner.                                    | Page reads have a short time limit while requests are held, and the page's "worker is acting" flag expires by itself.               |
| Integration tests | After a click that opened a new page, Plenipo waited out the whole page-load limit before answering.               | It now tracks page loads and stops waiting when the new page has loaded.                                                            |
| Unit tests        | On an upgrade, any other settings change wrote the website lists back empty before they were added.                | The website lists are added first.                                                                                                  |
| E2E screenshots   | Two updates could reach the sign in the wrong order, so after **Stop** it could still show the worker as active.   | Updates carry a revision, are told in order, and the app ignores an older one; the tray and desktop window always apply the newest. |
| E2E screenshots   | After **Take over**, the sign vanished as soon as the worker's step ended.                                         | It stays, saying you have control, until you press **Dismiss**.                                                                     |
| E2E screenshots   | The Activity trail showed each screenshot's file path as a separate line.                                          | Left out: the picture shows with its action (the Ledger keeps both).                                                                |
| Word check        | The permission set was named "Computer use", the tray said "runtimes", and one Activity event showed its raw name. | "Screen, mouse, and keyboard"; "programs"; plain words for `guard.sets_updated`.                                                    |

## 6. Deliverables

| Deliverable (plan)                   | Location                                                                                                                                                                                              |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Managed browser runtime              | `crates/capabilities/src/browser/mod.rs` (Plenipo's browser: find, start, own profile, restart), `cdp.rs` (DevTools Protocol)                                                                         |
| Browser automation capability        | `browser/tab.rs`, `page.js` (isolated world), `classify.rs`; tools in `tools.rs`, carried out in `broker/operate.rs`                                                                                  |
| Optional supported browser extension | Not built (ADR-020 §10)                                                                                                                                                                               |
| Screenshot/vision pipeline           | `crates/capabilities/src/screens.rs`; MCP image content in `mcp.rs`; `get_screenshot`; `ScreenshotView.tsx`                                                                                           |
| Computer-observe / computer-control  | `crates/capabilities/src/desktop.rs` (`xcap` on Windows, X11 on Linux, `enigo` for input)                                                                                                             |
| Domain/application policy            | `crates/guard/src/websites.rs`, engine site checks; Settings → Permissions → Websites (`Websites.tsx`)                                                                                                |
| User-visible session indicator       | `control.rs`; `ControlBanner.tsx`, the footer; `tray.rs`; `indicator.rs` and `IndicatorView.tsx`; the label in the page (`page.js`)                                                                   |
| Emergency stop                       | `Broker::stop_all_control`; **Stop all** (app), **Stop all browser and desktop control** (tray), **Stop** (desktop window)                                                                            |
| Role working instructions (Part 1)   | `crates/workforce/src/templates.rs`, `prompt.rs`; `update_role`; `RoleDialog`, the Inspector's role section                                                                                           |
| Commands                             | `get_control_status`, `stop_all_control`, `take_over_control`, `allow_control`, `set_website_rules`, `get_browser_status`, `open_browser`, `get_screenshot`, `update_role` (architecture overview §3) |
| Decision records                     | ADR-018, ADR-019, ADR-020 (all Proposed)                                                                                                                                                              |

## 7. Security notes and website terms

- **The owner's browser is never touched.** Plenipo's browser has its own profile folder. It
  never saves passwords, addresses, or cards, and has no sync and no extensions. Its control
  port listens on this computer only, on a random port, while it runs.
- **Pages cannot steer Plenipo.**
  - Plenipo's helpers run in an isolated world that the page's scripts cannot see.
  - The label in the page sits in a closed shadow root.
  - Only a real click or key press (not a script's) counts as the owner taking over.
  - Page text reaches the worker marked as the website's, never as instructions.
- **Control is always visible and stoppable.** There is no headless mode in the app, a sign
  shows everywhere, and Stop is one click away in the app, the tray, and the desktop window.
- **Known limits** (ADR-020, Consequences):
  - An ordinary page load that changes something is not held.
  - Requests a page makes on its own between actions are not held.
  - Frames an allowed page embeds are not checked, and DNS rebinding is not detected.
  - Another program running as the owner could reach the browser's control port while it runs.
  - The owner's keyboard is not noticed on the desktop.
  - Screenshots cannot be blurred.
  - Whether Codex passes screenshots to its model is unverified.
- **Website terms of use — owner's risk.**
  - Many websites forbid automated access in their terms, and acting through Plenipo's
    browser is acting under the owner's name. Breaking them can get the owner's account
    suspended.
  - LinkedIn, Facebook, Instagram, X/Twitter, TikTok, and Amazon start blocked for that reason.
    The list is a starting point, not a legal review.
  - Google and Bing forbid automated searches: research should use an official search API (a
    later integration).
  - Banks, payment sites, and government sites generally forbid automation, and they are
    where a mistake costs most. Keep them blocked.
  - Before allowing a website, check its terms (often "Terms of Use" or "robots" rules), and
    prefer its official API when it has one (the plan's order).

## 8. Deviations from the plan

| Deviation                                                                                      | Why                                                                                                                 | Record      |
| ---------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ----------- |
| Phase 10 comes before Phase 9, and Phase 9 is postponed with no Paperclip integration          | The owner's decision: Paperclip's Sales department never worked; a new one on HubSpot comes later                   | ADR-018     |
| No browser extension (the plan's "optional supported browser extension")                       | The DevTools Protocol needs nothing installed; an extension in the owner's browser would reach the owner's sign-ins | ADR-020 §10 |
| The owner's Edge or Chrome, with Plenipo's own profile, rather than a bundled browser          | Nothing to download or update; the installer stays small                                                            | ADR-020 §2  |
| Every role gets working instructions; the Writer set can save to git; a new Web Assistant role | The owner asked for them before the browser work (Part 1)                                                           | ADR-019     |

## 9. Owner items

| ID  | Item                                                                                                                                                                                                                                                                                                                                                                                         | Recommendation                                |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| O1  | ADR-018 (Phase 9 postponed, no Paperclip, a Sales department later on HubSpot) — **Proposed**. Accepting it means Plenipo never integrates Paperclip. The next Sales work is a new department built in Plenipo, with HubSpot as its CRM through HubSpot's official API and a private app token kept in Windows Credential Manager, and with no outbound messages sent without your approval. | Accept.                                       |
| O2  | ADR-019 (every role knows its job) — **Proposed**. Accepting it means every worker is told its job, what it hands back, its limits, and when to ask; the built-in Writer set can commit; and you can write the same for your own roles. Built-in roles' instructions stay Plenipo's.                                                                                                         | Accept.                                       |
| O3  | ADR-020 (Plenipo's browser and computer use, through Guard) — **Proposed**. Accepting it means workers may use websites only in Plenipo's own browser, as your website lists allow. Anything that sends, buys, or signs in always asks you. The mouse and keyboard are a last resort that asks every time. Stop and Take over are always one click away. You accept the known limits in §7.  | Accept.                                       |
| O4  | Windows check with Edge and the **real** AI tools (~40 min): the steps in [phase-10-checklist.md](phase-10-checklist.md#owner-check-on-windows-40-minutes).                                                                                                                                                                                                                                  | Recommended with v1.3.0; fixes go in a patch. |
| O5  | Website terms: check the terms of every website before you allow it (§7). Keep banks and payment sites blocked.                                                                                                                                                                                                                                                                              | Before allowing any website.                  |
| O6  | Earlier items still open: ADR-016 (the Development department) is still Proposed, and the Phase 8 Windows check with the real CLIs is still to come.                                                                                                                                                                                                                                         | As in the Phase 8 report.                     |

## 10. Verification

| Check                                                                     | Result                                                        |
| ------------------------------------------------------------------------- | ------------------------------------------------------------- |
| `pnpm check` (versions, format, lint, typecheck, tests)                   | Pass — FRONTEND_TOTAL frontend tests                          |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | Pass                                                          |
| `cargo test --workspace`                                                  | Pass — RUST_TOTAL tests                                       |
| `pnpm e2e` against the release build (Linux, Xvfb, Chromium)              | Pass — E2E_TOTAL of E2E_TOTAL, including the 6 Phase 10 tests |
| Generated TypeScript bindings                                             | Up to date (`pnpm bindings` leaves no diff)                   |
| GitHub CI on the PR                                                       | Linked from the PR                                            |

## 11. Phase boundary

Phase 10 is complete and released as v1.3.0. The owner's Windows check with Edge and the real
AI tools (§9, O4) is recommended with this release. Phase 9 (the Sales department on HubSpot) is
postponed until the owner schedules it (ADR-018). Phase 11 has not been started.
