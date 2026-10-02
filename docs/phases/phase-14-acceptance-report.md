# Phase 14 — Acceptance Report

|              |                                                                                                                                                                                                                                                                                                                                  |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 14 — Plenipo on Your Phone: a Web Interface Built From Scratch                                                                                                                                                                                                                                                                   |
| **Branches** | `claude/phase-14-phone` (14A), `claude/phase-14b-phone` (14B), `claude/phase-14c-phone` (14C), `claude/phase-14-page-home` (the page's own server, ADR-148)                                                                                                                                                                      |
| **Verified** | Locally on Windows for each part: `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff). GitHub CI on each pull request, including Windows and the real-app tests on Linux. The page's server, seen serving v1.19.2 at `https://remote.getplenipo.com` on 2026-10-02.       |
| **Date**     | 2026-10-01 to 2026-10-02 (Pacific time)                                                                                                                                                                                                                                                                                          |
| **Result**   | **Phase 14 delivered**, in three parts: 14A as **v1.19.0**, 14B as **v1.19.1**, 14C as **v1.19.2**. Every deliverable and every test in the plan's list passes. The phone's page is live on its own server. Phones reach their PC once the relay answers at `relay.getplenipo.com` (section 5). Plenipo by 8 West Ventures, LLC. |

**In short, for the owner.** Plenipo on your phone is built and released, in three parts: your
phone pairs with your PC and signs in with its face, fingerprint, or passcode; it reads every page,
answers approvals, and stops or allows work (14A, 14B); and it gets sealed notices when its page is
closed (14C). The page is live at `remote.getplenipo.com`, on its own small server. The tests used a
stand-in relay and a real browser as the phone. A released copy says **Coming soon** until 8 West's
relay answers at `relay.getplenipo.com` (ADR-140 §4); what is left for that is in part 14A's
section 5.

## Part 14A — the sealed line and the approvals (v1.19.0)

### 1. Deliverables → result

| #   | Deliverable (checklist, part 14A)                                                                          | Result   | Evidence                                                                                                                                        |
| --- | ---------------------------------------------------------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `Limit::PhoneAccess`, Pro only, with its plain words; pauses when Pro ends                                 | **Done** | `crates/licensing`; `phone_access_is_part_of_pro`; `free_and_pro`                                                                               |
| 2   | The switch, off to begin with; "Part of Pro" on Free; "Coming soon" until the relay is live                | **Done** | `PhoneSwitch.tsx`, `remote.test.tsx`; `remote_host::built_for` tests; the Release workflow's `PLENIPO_RELAY_LIVE`                               |
| 3   | Guard's purpose **phone access**: only the relay's name, only on Pro with the switch on                    | **Done** | `crates/guard/src/outbound.rs` (`check_relay` tests); `the_relay_serves_only_a_pc_on_pro`                                                       |
| 4   | The PC's relay key and Noise key in the Vault; the relay connection built only on Pro                      | **Done** | `crates/remote/src/keys.rs`, `link.rs`; uninstall removes them (`forget_phone_access`)                                                          |
| 5   | `contracts/phone-relay/v1`: messages, codes, limits                                                        | **Done** | `contracts/phone-relay/v1` (README, messages, Noise test answers, pairing codes)                                                                |
| 6   | A stand-in relay for the tests, with a bad relay mode                                                      | **Done** | `stand_in::Relay` (`Bad::Change`, `Replay`, `Drop`, `Invent`), and the `plenipo-test-relay` program for the real-app test                       |
| 7   | Noise on the PC and on the phone, each passing the test answers and each other                             | **Done** | `crates/remote/src/noise.rs`; `apps/remote/src/lock/lock.test.ts` (the same answers, byte for byte)                                             |
| 8   | Settings → Devices: Add a phone, Is this your phone?, the list, Rename, Remove, Un-pause                   | **Done** | `DevicesSettings.tsx`, `remote.test.tsx`; the real-app test                                                                                     |
| 9   | The passkey: made at pairing, checked by the PC at sign-in                                                 | **Done** | `crates/remote/src/webauthn.rs`; the real-app test signs in with a passkey made by the browser's own authenticator                              |
| 10  | Sign-in and its end: 30 minutes idle, 12 hours, Sign out, Remove, switch off, Pro ends                     | **Done** | `signing_out_and_lapsing_end_the_sign_in`, `switching_off_cuts_every_phone_off_at_once`, `free_and_pro`                                         |
| 11  | Wrong tries: pairing codes, failed meetings, refused passkey answers                                       | **Done** | `a_wrong_code_fails_three_times_and_dies`, `three_dead_codes_pause_adding_a_phone`, `too_many_failed_meetings_…`, `three_refused_checks_…`      |
| 12  | `guard::remote`: the fixed list as one `enum`, and Guard's checks in order                                 | **Done** | `crates/guard/src/remote.rs` (9 tests); `a_phone_cannot_ask_for_anything_off_the_list`                                                          |
| 13  | The Ledger: `remote.*` events with the phone's ID and name; "Approved by you, from …"                      | **Done** | `Broker::resolve_approval_via`; Activity's words (`describeRemoteEvent`); the real-app test reads them back                                     |
| 14  | Keep these approvals on my PC only, none ticked; "Approve on your PC" on the phone                         | **Done** | `approvals_kept_on_the_pc_cannot_be_answered_from_a_phone` (Guard and the PC); the page test                                                    |
| 15  | `apps/remote`: the page, phone screen first, both themes, from the keyboard, design system and plain words | **Done** | `apps/remote` (25 tests); the real-app test's screenshots in both themes                                                                        |
| 16  | The page's rules: no outside scripts, only its own files and the relay                                     | **Done** | `apps/remote/vite.config.ts` writes the page's Content Security Policy at build; CI checks the built page names only the relay it was built for |
| 17  | PC offline, and a request lost part way                                                                    | **Done** | `when_the_pc_is_offline_…`, `after_a_lost_connection_the_phone_asks_what_happened`; the page tests; the real-app test closes Plenipo            |
| 18  | New desktop commands are the main window's alone, with refusal tests                                       | **Done** | `phone_access_settings_are_the_main_windows_alone`; `capabilities/default.json`                                                                 |
| 19  | `docs/editions.md`: what Plenipo sends to the relay, and phone access on Pro                               | **Done** | [Free and Pro](../editions.md#plenipo-on-your-phone-and-what-it-sends-to-8-wests-relay); `the_pc_sends_the_relay_only_what_the_contract_lists`  |
| 20  | Release notes, the plan's status, the order of work, the roadmap, the vocabulary, this report              | **Done** | [v1.19.0](../releases/v1.19.0.md), `ROLLOUT_PLAN.md`, [roadmap](../roadmap.md), [plain words](../design/vocabulary.md)                          |

### 2. The owner's answers → as built

| Answer (2026-10-01)                                                  | As built                                                                                                                                  |
| -------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Picture code or 16-letter code; "Is this your phone?" (ADR-141)   | Settings → Devices shows both; the code works once, for 10 minutes, 3 wrong tries; nothing is added until **Add**.                        |
| 2. The passkey **only at sign-in** (ADR-142)                         | The phone's face, fingerprint, or passcode at sign-in; approvals inside a sign-in need no second check.                                   |
| 4. 30 minutes idle, 12 hours at most (ADR-142, ADR-143)              | As decided; the PC ends the sign-in and the phone asks again.                                                                             |
| 5. The relay checks Pro with 8 West's signed weekly answer (ADR-143) | The PC shows the relay the newest signed answer; the stand-in relay refuses a PC that is not on Pro.                                      |
| 7. No approvals kept on the PC to begin with (ADR-145)               | None ticked; the choice is in Settings → Devices.                                                                                         |
| 8. `remote.getplenipo.com`, next to the website (ADR-146)            | The page is built from `apps/remote`, with the relay's name built in. Hosting it on its own server is an owner step (section 5, ADR-148). |
| 9. Three parts (ADR-140)                                             | This is part 14A, v1.19.0.                                                                                                                |
| Relay passes last 90 days (ADR-147, under the standing order)        | A phone used now and then stays paired; a copied pass alone still cannot reach the PC.                                                    |

### 3. Tests → evidence

The plan's list, test by test (each also ticked in the [checklist](phase-14-checklist.md#tests-the-plans-list)):

| Test (the plan's list)                                                                            | Evidence                                                                                                                                                  |
| ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pairing a device, and a wrong or expired pairing code                                             | `a_phone_is_paired_at_the_pc_and_signs_in`, `a_wrong_code_…`, `an_expired_code_is_refused`; real app: a cancelled and a wrong code, then pairing          |
| Signed-in connection                                                                              | `a_phone_is_paired_at_the_pc_and_signs_in`; real app: sign out, then **Check it's you** with the browser's passkey, checked by the PC                     |
| Unknown device (never paired)                                                                     | `an_unknown_phone_is_refused`                                                                                                                             |
| Removed device and ended session, refused at once                                                 | `a_removed_phone_is_refused_at_once`, `a_phone_removed_while_away_is_told_it_is_not_listed`; real app: removed, then refused even with a copy of its keys |
| Replay protection                                                                                 | `a_copied_request_is_refused`; the bad relay's **Replay** mode                                                                                            |
| Too many wrong tries                                                                              | `too_many_failed_meetings_slow_then_stop_the_pc`, `three_dead_codes_…`, `three_refused_checks_pause_the_phone`                                            |
| Approving and refusing from the web interface, after the phone confirms it is the owner           | `approving_and_refusing_go_through_guard_and_the_first_answer_counts`; desktop `a_phone_answers_an_approval_…`; real app: **Approve** on the phone        |
| An approval answered on the PC first, then on the phone, and the other way round                  | desktop `a_phone_answers_an_approval_the_same_way_and_the_first_answer_counts`; real app: both orders                                                     |
| An approval kept "on the PC only" cannot be answered from another device                          | `approvals_kept_on_the_pc_cannot_be_answered_from_a_phone` (Guard and the PC); the page shows "Approve on your PC"                                        |
| Stop all from another device                                                                      | real app: **Stop all** on the phone stops the PC's browser, desktop, and server work                                                                      |
| PC offline, and connection lost part way through                                                  | `when_the_pc_is_offline_…`, `after_a_lost_connection_…`; page tests; real app: Plenipo closed and started again                                           |
| The relay cannot read, answer, or make up a request; a replayed one is refused                    | `the_relay_cannot_read_change_replay_or_invent_a_request`; real app: the stand-in relay never saw the words                                               |
| Free edition: nothing connects to the relay, and the switch says it comes with Pro                | `free_and_pro`, desktop `phone_access_is_part_of_pro`; real app: on Free, the relay heard nothing even with the switch saved on                           |
| The owner turns the switch off on the PC while a device is connected                              | `switching_off_cuts_every_phone_off_at_once`; the page test "switched off on the PC"                                                                      |
| The web interface cannot start an AI tool, run a program, reach a shell, files, … or change rules | `a_phone_cannot_ask_for_anything_off_the_list`; the fixed list (`RequestKind`); desktop commands refuse other windows and web pages                       |
| The web interface on a phone-sized screen, in both themes, from the keyboard, with no errors      | real app: every page at 390 × 844 in both themes, no sideways scrolling, no errors in the browser; the page test "works from the keyboard alone"          |

Also from the phase's rules: every request is recorded with the phone that sent it (the real-app
test reads Activity back); new desktop commands refuse a second window, the sign, and a web page;
and no log holds a key, a code, a pass, a passkey answer, or a sealed message (the link logs only
plain reasons).

### 4. Screenshots

From the real-app test on GitHub's Linux machine (`tests/e2e/specs/remote.e2e.mjs`): a copy of
Plenipo built for the tests, a stand-in relay, and Google Chrome at a phone's size as the phone,
with made-up names.

- **Turning it on, on the PC:** [on Free, Devices says it is part of Pro, and the PC connects to
  nothing](evidence/phase-14/phone-devices-free.png) · [the switch Use Plenipo from another device,
  on](evidence/phase-14/phone-switch-on.png) · [Add a phone: a picture code and a typed code that
  work once, for 10 minutes](evidence/phase-14/phone-devices-code.png)
- **Pairing:** [a wrong code, refused, with nothing
  added](evidence/phase-14/phone-pair-wrong-code.png) · [the phone's name and the typed
  code](evidence/phase-14/phone-pair.png) · [the phone waits for your yes (its line breaks are fixed
  in part 14B)](evidence/phase-14/phone-pair-waiting.png) · [the PC asks Is this your
  phone?](evidence/phase-14/phone-devices-ask.png) · [the phone on the PC's list, signed
  in](evidence/phase-14/phone-devices-listed.png)
- **Approvals:** [a second approval waiting on the phone, and the first one answered from
  it](evidence/phase-14/phone-approvals-answered.png) · [on the phone afterwards: nothing left
  waiting, and both answers listed](evidence/phase-14/phone-approvals-dark.png) · [on the PC, each
  answer says who gave it, and from which phone](evidence/phase-14/phone-pc-approvals-answered.png)
- **Every page fits a phone's screen, in both themes:** Home
  ([dark](evidence/phase-14/phone-home-dark.png), [light](evidence/phase-14/phone-home-light.png)) ·
  Approvals ([dark](evidence/phase-14/phone-approvals-dark.png),
  [light](evidence/phase-14/phone-approvals-light.png)) · Work
  ([dark](evidence/phase-14/phone-work-dark.png), [light](evidence/phase-14/phone-work-light.png)) ·
  Activity ([dark](evidence/phase-14/phone-activity-dark.png),
  [light](evidence/phase-14/phone-activity-light.png)) · More
  ([dark](evidence/phase-14/phone-more-dark.png), [light](evidence/phase-14/phone-more-light.png)).
  More's AI tools show their codes here, and Activity the pages the phone opened; both are fixed in
  part 14B.
- **Stop all:** [the phone asks first](evidence/phase-14/phone-stop-all-ask.png) · [the PC stops
  browser, desktop, and server work](evidence/phase-14/phone-pc-stopped.png)
- **Signing in, the PC turned off, and a removed phone:** [signing in again with the phone's
  passkey](evidence/phase-14/phone-sign-in.png) · [the PC is off: Your PC can't be reached. Nothing
  was changed.](evidence/phase-14/phone-offline.png) · [Remove asks first, and says the phone is cut
  off at once](evidence/phase-14/phone-devices-remove.png) · [a removed phone, refused even with a
  copy of its keys](evidence/phase-14/phone-removed.png)
- **Activity on the PC:** [each request, with the phone that sent
  it](evidence/phase-14/phone-pc-activity.png)

### 5. What the owner needs to do, and the checks only the owner can do

**Before phone access can reach a real phone:**

1. Approve the [relay change request](phase-14-relay-change-request.md) in the relay's own
   repository, and have it built there.
2. In Cloudflare: point `relay.getplenipo.com` at the relay (WebSockets on). **Done for the
   page:** `remote.getplenipo.com` points at the page's own Tunnel (ADR-146).
3. **Done, 2026-10-02:** the phone page's own small AWS server (ADR-148, the phone's page on its
   own server, about $7 to $8 a month), set up by the builder from the steps in
   [`apps/remote/deploy/README.md`](../../apps/remote/deploy/README.md), with the owner's one
   click in Cloudflare. It serves each release's page by itself; seen serving v1.19.2.
4. When the relay answers, set the repository variable `PLENIPO_RELAY_LIVE` to `true`; the next
   release turns the switch on.

**Checks only the owner can do, on real phones and the real relay** (the tests prove the design
with stand-ins):

- [ ] Pair an **iPhone** from its Home Screen page, and an **Android** phone from Chrome
- [ ] Face ID or Touch ID on the iPhone, and the fingerprint or screen lock on Android, at sign-in
- [ ] Approve and refuse from each phone, and see "from your phone" in Activity on the PC
- [ ] Turn the switch off on the PC while the phone is open, and see it cut off
- [ ] **Remove** a phone on the PC, and see it refused at once
- [ ] Notices (part 14C): listed in its section when it is built

## Part 14B — everything else that is safe from the page (v1.19.1)

**In short, for the owner.** From the phone you can now also **Allow again** after Stop all,
**Stop the worker** on a working task, give a worker an objective in words, **Run again** or
**Leave stopped** after Plenipo closed unexpectedly, and **Keep** or **Discard** a lesson. Each goes
through Guard and shows in Activity with your phone's name. What you need to do is still the list
in part 14A's section 5.

### 1. Deliverables → result

| #   | Deliverable (checklist, part 14B)                                   | Result   | Evidence                                                                                             |
| --- | ------------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------- |
| 1   | **Allow again** after Stop all                                      | **Done** | The phone's top bar (`Shell.tsx`); page test; real app: Stop all, then Allow again, from the phone   |
| 2   | **Stop** one worker's task                                          | **Done** | **Stop the worker** on Work (`Work.tsx`), asks first; page test; real app                            |
| 3   | **Run again** and **Leave stopped** after an unexpected stop        | **Done** | Home's notice, each organization's own; page and PC tests; real app: Plenipo stopped with no warning |
| 4   | **Keep** and **Discard** a lesson, as written                       | **Done** | More (`More.tsx`); page test; the PC refuses an unknown lesson in plain words                        |
| 5   | **Send an objective** to a position that takes objectives, in words | **Done** | **Give objective** on a worker's page; page test; real app: the phone's objective runs on the PC     |
| 6   | Release notes and this section                                      | **Done** | [v1.19.1](../releases/v1.19.1.md)                                                                    |

### 2. Tests → evidence

| Test (the plan's list)                                                                  | Evidence                                                                                                                         |
| --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Allowing (Allow again) from the web interface, after the phone confirms it is the owner | Page test "Allow again after Stop all"; real app: the phone's **Allow again** lets the PC's work go again                        |
| Stop a task, Allow again, Run again, and Leave stopped from another device              | Page tests; desktop `a_phone_does_the_rest_of_what_is_safe`; real app: each from the phone, with Plenipo stopped with no warning |
| Sending an objective from another device                                                | Page test; desktop test (refusals in plain words); real app: the objective runs on the PC, and the phone follows it              |

Also: every one of these is recorded with the phone that sent it (the real-app test reads them
back from Activity), and a PC on v1.19.0 never sees a request it cannot read.

**Also fixed in part 14B,** from part 14A's screenshots: More → **AI tools** named each AI tool by
its code ("claude-code") and said **Ready** for every one. Your PC now sends each AI tool's own
name and whether it can work now (never where it is installed), and the phone lists them in plain
words (page test; desktop `a_phone_reads_the_same_pages`). The phone's **Activity** was mostly
the pages it had opened ("Test phone asked to read Home"); those stay recorded in the Ledger and
on the PC's Activity, and the phone's page now leaves them out (same desktop test). A notice that
is one paragraph no longer puts each bold word on its own line ("Click / **Add** / on your PC.").

### 3. Screenshots

From the real-app test on GitHub's Linux machine (`tests/e2e/specs/remote.e2e.mjs`), as in part
14A.

- **From the phone:** [a worker's objective, given from the phone, running on the
  PC](evidence/phase-14/phone-conversation-running.png) · [after Stop all: Allow again in the
  phone's top bar](evidence/phase-14/phone-stopped-allow-again.png) · [after Plenipo closed
  unexpectedly: Run again or Leave stopped](evidence/phase-14/phone-stopped-unexpectedly.png)
- **Fixed from part 14A's screenshots:** More, each AI tool by its name with what it can do now
  ([dark](evidence/phase-14/phone-14b-more-dark.png),
  [light](evidence/phase-14/phone-14b-more-light.png)) · Activity, without the pages the phone
  opened ([dark](evidence/phase-14/phone-14b-activity-dark.png),
  [light](evidence/phase-14/phone-14b-activity-light.png)) · [waiting for your yes, in one
  paragraph](evidence/phase-14/phone-14b-pair-waiting.png)

## Part 14C — notices when the page is closed (v1.19.2; Phase 14 delivered)

**In short, for the owner.** Your phone now gets a **notice** when something needs you, even when
Plenipo's page is closed. Only your phone can read it: your PC seals it with your phone's keys and
signs it with its own key, and 8 West's relay never sees notices at all. On Android you can
**Refuse** or **Discard** right from the notice; **Approve** and **Keep** open Plenipo on that item.
On an iPhone, a tap opens it. You choose what the lock screen shows. With this part, Phase 14 is
built; it reaches real phones once the relay and the page's home are ready (part 14A, section 5).

### 1. Deliverables → result

| #   | Deliverable (checklist, part 14C)                                                           | Result   | Evidence                                                                                                                 |
| --- | ------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------ |
| 1   | The PC's notice key, in the Vault; signing up for notices inside the sealed line            | **Done** | `keys.rs` (made with the PC's keys); the welcome names it; `noticesOn` in the sealed line; the page test "signs up"      |
| 2   | Guard's purpose **phone notices**: only the four notice services, only to a phone's address | **Done** | `outbound.rs` (`check_notice_service`); `phone_notices.rs` test "only a phone's notice service is reached"               |
| 3   | Notices sealed for the phone (RFC 8291), signed by the PC's key (RFC 8292)                  | **Done** | `webpush.rs`: the standard's own example, byte for byte; the real-app test opens and checks them with node's own crypto  |
| 4   | The short line, from Guard's cleaned approval card; the lock-screen choice on the phone     | **Done** | The PC's own notice's words (`phone_notice`); **On the lock screen** on the phone (`Notices.tsx`, `notice.ts`)           |
| 5   | Android: **Approve** / **Refuse**, **Allow again**, **Keep** / **Discard**; iPhone: one tap | **Done** | `notice.ts` (buttons; a notice never approves by itself), `sw.js`; **Refuse** and **Discard** answer in a notice meeting |
| 6   | An answered item shows "Already answered"; a repeated notice shows once                     | **Done** | The page tests "already answered"; one tag per item; the real-app test opens an answered approval                        |
| 7   | The Home Screen guide for iPhone                                                            | **Done** | More → Notices on this phone, on an iPhone outside the Home Screen; the page test                                        |
| 8   | Release notes, this report, and the plan: Phase 14 delivered                                | **Done** | [v1.19.2](../releases/v1.19.2.md), `ROLLOUT_PLAN.md`, [roadmap](../roadmap.md)                                           |

### 2. Tests → evidence

| Test (the plan's list)                                                                                              | Evidence                                                                                                                                                                                                                             |
| ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Approving, refusing, and allowing right from a notice (Android), and one tap to that approval (iPhone)              | `notice.test.ts` (each button: Refuse and Discard answer, Approve, Keep, Allow again, and a tap open the item); `notices.test.tsx` (Refuse from a notice meeting); `a_notice_may_only_say_no` (the PC takes only a no from a notice) |
| A notice's words can be read only on the owner's phone, and the lock-screen choice shows only "Something needs you" | `webpush.rs` tests and `notices_go_only_to_phones_that_asked_and_only_they_can_read_them`; real app: the stand-in notice service saw only a sealed notice; `noticeFor` with the lock-screen choice                                   |

Also: notices go only to phones that asked, only on Pro with phone access on and **Notices on my
phones** on, and only to a phone's own notice service (Guard checks the address when the phone
signs up, and again for every notice); a gone notice address is forgotten; and Activity records
each notice with the phone and its kind, never its words.

**Also fixed in part 14C,** from part 14B's screenshots: the phone's Activity showed an event it
had no words for by its code ("guard grant closed"). It now has plain words for what matters, and
leaves out the rest, which stays in Activity on the PC under All events (page test `words.test.ts`).

### 3. Screenshots

From the real-app test on GitHub's Linux machine (`tests/e2e/specs/remote.e2e.mjs`), as in part
14A.

- **On the phone:** [More: Notices on this phone, off](evidence/phase-14/phone-notices-off.png) ·
  [on, with the lock-screen choice](evidence/phase-14/phone-notices-on.png) · [a notice opened: that
  approval, on the phone](evidence/phase-14/phone-notice-opened.png) · [opened again after the PC
  answered it: Already answered](evidence/phase-14/phone-notice-already-answered.png)
- **On the PC:** [Settings → Notifications: Notices on my
  phones](evidence/phase-14/phone-pc-notifications.png)
- **Fixed from part 14B's screenshots:** the phone's Activity in plain words
  ([dark](evidence/phase-14/phone-14c-activity-dark.png),
  [light](evidence/phase-14/phone-14c-activity-light.png))

### 4. Checks only the owner can do, on real phones

- [ ] On Android (Chrome): **Notices on this phone** on; a notice with the page closed; **Refuse**
      from the notice; **Approve** from the notice (unlock, sign in if asked, one tap)
- [ ] On an iPhone: add Plenipo to the Home Screen, pair from there, turn notices on, and one tap
      from a notice to that approval
- [ ] **On the lock screen → Show only "Something needs you"**, and see the lock screen say only that
