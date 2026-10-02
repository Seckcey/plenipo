# Phase 14 — Acceptance Report

|              |                                                                                                                                                                                                                                         |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 14 — Plenipo on Your Phone: a Web Interface Built From Scratch                                                                                                                                                                          |
| **Branch**   | `claude/phase-14-phone`                                                                                                                                                                                                                 |
| **Verified** | Locally on Windows: `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff). GitHub CI on the pull request, including Windows and the real-app tests on Linux.                       |
| **Date**     | 2026-10-01 (Pacific time)                                                                                                                                                                                                               |
| **Result**   | **Part 14A** built, as **v1.19.0**: every 14A deliverable, and every 14A test in the plan's list passes. Parts 14B and 14C follow. The checks only the owner can do, on real phones, are in section 5. Plenipo by 8 West Ventures, LLC. |

**In short, for the owner.** Your PC's side of using Plenipo from your phone is done and tested:
the switch, adding a phone, its face, fingerprint, or passcode, the sealed line through 8 West's
relay, every page to read, **Approve**, **Refuse**, and **Stop all**. The tests used a stand-in
relay on the test machine and a real browser as the phone. A released copy says **Coming soon**
until 8 West's relay is ready (ADR-140 §4, phone access reaches people only after the relay change
is live). What you need to do is in section 5.

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

| Answer (2026-10-01)                                                  | As built                                                                                                                    |
| -------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| 1. Picture code or 16-letter code; "Is this your phone?" (ADR-141)   | Settings → Devices shows both; the code works once, for 10 minutes, 3 wrong tries; nothing is added until **Add**.          |
| 2. The passkey **only at sign-in** (ADR-142)                         | The phone's face, fingerprint, or passcode at sign-in; approvals inside a sign-in need no second check.                     |
| 4. 30 minutes idle, 12 hours at most (ADR-142, ADR-143)              | As decided; the PC ends the sign-in and the phone asks again.                                                               |
| 5. The relay checks Pro with 8 West's signed weekly answer (ADR-143) | The PC shows the relay the newest signed answer; the stand-in relay refuses a PC that is not on Pro.                        |
| 7. No approvals kept on the PC to begin with (ADR-145)               | None ticked; the choice is in Settings → Devices.                                                                           |
| 8. `remote.getplenipo.com`, next to the website (ADR-146)            | The page is built from `apps/remote`, with the relay's name built in. Hosting it on Coastline is an owner step (section 5). |
| 9. Three parts (ADR-140)                                             | This is part 14A, v1.19.0.                                                                                                  |
| Relay passes last 90 days (ADR-147, under the standing order)        | A phone used now and then stays paired; a copied pass alone still cannot reach the PC.                                      |

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

From the real-app test on GitHub's Linux machine: a copy of Plenipo built for the tests, a stand-in
relay, and Google Chrome at a phone's size as the phone, with made-up names.

_Added from the pull request's test run._

### 5. What the owner needs to do, and the checks only the owner can do

**Before phone access can reach a real phone:**

1. Approve the [relay change request](phase-14-relay-change-request.md) in the relay's own
   repository, and have it built there.
2. In Cloudflare: point `relay.getplenipo.com` at the relay (WebSockets on), and
   `remote.getplenipo.com` at Coastline's tunnel (ADR-146).
3. On Coastline: the phone page's piece of the website's updater (the builder writes the steps and
   can do the console work).
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

### 3. Screenshots

_Added from the pull request's test run._
