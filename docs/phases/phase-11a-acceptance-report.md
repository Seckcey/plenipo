# Phase 11A — Acceptance Report

|              |                                                                                                                                                                                                                                 |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 11A — Free and Pro Editions and the License Key                                                                                                                                                                                 |
| **Branch**   | `claude/phase-11a-license`                                                                                                                                                                                                      |
| **Verified** | Locally on Windows: `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` (1,650 tests), `pnpm bindings` (no diff). GitHub CI on the pull request, including Windows and the real-app tests on Linux. |
| **Date**     | 2026-09-30 (Pacific time)                                                                                                                                                                                                       |
| **Result**   | Every deliverable built, as **v1.18.0**. Every test in the plan's list passes. The owner's checks are in section 6. Plenipo by 8 West Ventures, LLC.                                                                            |

Screenshots, from the real-app tests on GitHub's Linux machine (a copy built for the tests, so it
says it also accepts 8 West's test keys):

- **Free:** [Settings → License on Free](evidence/phase-11a/license-free.png) ·
  [a second organization is part of Pro](evidence/phase-11a/license-free-organizations.png) ·
  [Connections on Free](evidence/phase-11a/license-free-connections.png) ·
  [the whole Development flow, finished on Free](evidence/phase-11a/development-on-free-result.png)
- **Pro:** [a key entered, checked with 8 West's stand-in](evidence/phase-11a/license-pro.png) ·
  [the subscription ended: back to Free, nothing taken away](evidence/phase-11a/license-ended.png)

## 1. Deliverables → result

| #   | Deliverable (ROLLOUT_PLAN.md)                                                                 | Result   | Evidence                                                                                          |
| --- | --------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------- |
| 1   | `crates/licensing`: the edition model, key checking, the Entitlements snapshot, and the check | **Done** | `crates/licensing` (64 tests)                                                                     |
| 2   | A signed key: edition, holder, key ID, plan, and paid-through date                            | **Done** | ADR-104 (the signing key in AWS KMS; keys stay Ed25519); `key.rs`; `contracts/license-check/v1`   |
| 3   | The key checked on the PC against the public key built in, at every start                     | **Done** | `License::load`; the production keys go in once made (section 6)                                  |
| 4   | The weekly check, sending the key's ID and the app version and nothing else                   | **Done** | `license_host.rs`, `license_check.rs`; the byte-for-byte tests in section 3                       |
| 5   | 30 days offline; fail-open on every error                                                     | **Done** | ADR-116 (the weekly answer is signed); `state.rs`; `license_host.rs` tests                        |
| 6   | The key in the Vault                                                                          | **Done** | ADR-110 (one person, any of their PCs); `VAULT_ID`; the uninstall test                            |
| 7   | Settings → License                                                                            | **Done** | `LicenseSettings.tsx`, `license.test.tsx`, `license.e2e.mjs`                                      |
| 8   | One enforcement point, `Entitlements::check(limit)`                                           | **Done** | `entitlements.rs`                                                                                 |
| 9   | Free's limits: 1 department, 1 project, 3 workers at once (and 1 organization)                | **Done** | ADR-113 (the fourth waits its turn); the Workforce, Liaison, and IPC tests                        |
| 10  | Business departments are Pro at setup                                                         | **Done** | ADR-114 (business departments); `a_business_template_is_part_of_pro`                              |
| 11  | Connections and add-on tools are Pro, and pause when Pro ends                                 | **Done** | ADR-068 (Connections are part of Pro); the two Connections tests                                  |
| 12  | Plain words on every blocked path                                                             | **Done** | `words.rs` snapshot tests; "part of Pro" reaches the screen as its own kind of message            |
| 13  | Ledger events for every license action and check, with the key redacted                       | **Done** | `license.key_entered`, `key_refused`, `key_removed`, `checked`, `check_failed`, `edition_changed` |
| 14  | Lapse never deletes, hides, or breaks anything                                                | **Done** | `when_pro_ends_nothing_is_lost_and_only_new_ones_are_refused`; `license.e2e.mjs` test 4           |
| 15  | `docs/editions.md` updated: the prices and what the check sends                               | **Done** | `docs/editions.md`                                                                                |

## 2. The owner's answers → as built

| Answer (ADR-100)                                 | As built                                                                                                            |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| 4. Ed25519, key in AWS KMS (ADR-104)             | Ed25519 keys and answers; the production public keys are added once the owner makes them.                           |
| 5. getplenipo.com (ADR-105)                      | The check goes to `https://account.getplenipo.com/v1/check` only.                                                   |
| 10. One person, any of their PCs (ADR-110)       | One license for the PC, kept under the first organization's name; Free's limits count across the PC.                |
| 12. Lessons pause on Free (ADR-112)              | No new lessons and no kept ones on a task that started on Free; kept lessons say "Paused — part of Pro".            |
| 13. Free's fourth waits; Pro 4 per org (ADR-113) | Handed-on work waits its turn and starts by itself; work the owner starts past the third is refused in plain words. |
| 14. Business department = from a template (114)  | Each template says whether it is a business department; Development is not.                                         |
| 15. Free never contacts 8 West (ADR-115)         | No key, no request; only test copies can use a stand-in; tested three ways.                                         |
| 16. The weekly answer is signed (ADR-116)        | Signed answers; the 30 days count from 8 West's signed time; the clock cannot extend them.                          |
| 17. Paid AI keys are Free (ADR-117)              | `docs/editions.md` says so; Phase 16's code was not touched.                                                        |

## 3. Tests → evidence

The plan's list, test by test, is in the [checklist](phase-11a-checklist.md#tests-the-plans-list).
The ones the plan marks in bold:

| Test (the plan's list)                                           | Evidence                                                                                                     |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| No internet: Pro stays on through day 30 and drops on day 31     | `pro_stays_30_days_without_a_check_and_the_clock_cannot_extend_it`                                           |
| Service down, 500, timeout, garbage: Pro stays on and retries    | `failed_checks_keep_pro_on_and_retry_later`, `another_status_or_a_redirect_is_a_failed_check`                |
| Cancelled: Pro stays until the end of the paid period, then goes | `cancelled_keeps_pro_until_the_end_of_the_paid_period_then_drops`                                            |
| The check's body is the key's ID and the app version, exactly    | `the_check_sends_exactly_the_key_id_and_the_app_version`; the contract tests; `license.e2e.mjs` test 3       |
| A Free install makes no request to 8 West                        | `a_free_copy_never_contacts_8_west`; `license.e2e.mjs` test 1; `development.e2e.mjs` (a whole flow, on Free) |

The whole feature is new, so its tests could not run on the code before it. Each test added for a fix (the review's, and the same key entered twice) was run on the code before its fix, and fails there.

## 4. Known limits

Each is recorded in its ADR as built:

- **Work the owner starts past the third worker** on Free is refused with plain words, not queued
  (ADR-113).
- **Bringing back a department** on Free brings its projects back too, even past one project
  (ADR-021).
- **The 30 days are kept on the PC**: deleting the license record starts a new 30 days (ADR-022).
- **A Vault that cannot be read** at start means Free for that run (ADR-021).
- **No pop-up notice when Pro ends**: Settings → License and the Activity trail say so (ADR-021).

## 5. Review

Three areas were reviewed. A second reviewer then checked every finding in the code. Each fix
has a test, and each test was run on the code before its fix and fails there (except where the
table says it guards the fix).

**The license and the weekly check** (8 findings: 7 confirmed, 1 not real; the second reviewer
found 1 more):

| Finding                                                                         | Fixed by                                                                                                                                        | Test                                                                            |
| ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| A clock once set far ahead kept a paying owner on Free, even after a new answer | 8 West's next newer signed answer resets Plenipo's time; a check is due when the clock is put right; the screen says how far ahead the clock is | `a_clock_set_ahead_once_is_undone_by_8_wests_next_answer`                       |
| Removing a key and entering it again started a new 30 days, and forgot "ended"  | The record stays when the key is removed or missing                                                                                             | `removing_and_entering_a_key_again_never_restarts_the_30_days_or_forgets_ended` |
| A Vault that could not be read kept a paying owner on Free until a restart      | Each look tries the Vault again, with the record on disk                                                                                        | `a_vault_that_answers_again_brings_the_key_back_at_the_next_look`               |
| Any window could overwrite the key through the secret commands                  | **Not real:** those commands accept only Plenipo's own secret IDs                                                                               | —                                                                               |
| A kept key that stopped checking was dropped with no word, and its record wiped | The screen says why; the record is kept                                                                                                         | `a_kept_key_that_no_longer_checks_says_why_and_keeps_its_record`                |
| A time entered missing or in the future let the 30 days slide on                | Not believed; set to now                                                                                                                        | `a_missing_or_future_entered_time_is_not_believed`                              |
| The contract did not list the `Accept` header; its key schema was looser        | README and schema match the code (and the service is pinned to them again)                                                                      | the contract tests in both repositories                                         |
| Short blocking calls while the license was held                                 | `get_license` runs off the main thread; the Ledger and the Vault are written outside it                                                         | (no behaviour to test)                                                          |
| (Second reviewer) A clock held back stopped Plenipo's time                      | While Plenipo runs, a clock that only moves forward carries its time on                                                                         | `holding_the_clock_back_while_plenipo_runs_does_not_stop_its_time`              |
| A replayed answer must never undo the clock fix                                 | Only a strictly newer answer lowers Plenipo's time                                                                                              | `a_replayed_answer_never_brings_back_what_the_clock_took` (guards the fix)      |

**Where Free's limits are enforced** (7 findings: 5 confirmed, 2 partly; the second reviewer found
1 more):

| Finding                                                                   | Fixed by                                                                                  | Test                                                            |
| ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Projects archived before 1.10.0 still counted, blocking Free for good     | They count as archived                                                                    | `a_project_archived_before_1_10_does_not_count`                 |
| A worker's place was held up to a minute after its task started           | The place is freed as soon as the start returns; the Ledger counts the task from then     | `on_free_a_worker_that_finished_frees_its_place_at_once`        |
| A waiting member task was given a new conversation on every try           | Its place is found first; no conversation until it starts                                 | (covered by the loop test below)                                |
| A race could let a fourth worker start                                    | Who is on the job is read under the same lock that lets one in                            | `workers_starting_together_never_pass_three`                    |
| Bringing a department back skips the project limit                        | **Kept as a limit** (ADR-021 as built): its projects were made on Pro                     | —                                                               |
| Two creates at the same moment could both pass                            | One lock from the Free check to the Ledger write                                          | `on_free_projects_made_at_the_same_moment_never_pass_the_limit` |
| Work the owner starts past the third reached the screen as the wrong kind | A "part of Pro" error, mapped on every path                                               | `free_runs_three_workers_at_once_and_the_fourth_waits_its_turn` |
| (Second reviewer) A waiting task was retried hundreds of times a second   | Its try ending no longer wakes Liaison at once (549 tries a second before; a handful now) | `free_runs_three_workers_at_once_and_the_fourth_waits_its_turn` |

The account service's review is in the [Phase 22 report](phase-22-acceptance-report.md), section 6.

## 6. Left for the owner

1. **The signing keys in AWS KMS: done** (2026-09-30). The app trusts the current key as `prod-1`
   and the spare as `prod-2`; `the_key_in_use_and_the_spare_are_both_trusted` checks both.

2. **On a real Windows PC**, after the release:
   - enter a key, and see Pro turn on
   - restart, and see Pro kept
   - remove the key
   - uninstall with "delete my data", and see the key gone from Credential Manager
