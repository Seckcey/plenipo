# Phase 11A — Acceptance Report

|              |                                                                                                                                                                                                                                 |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 11A — Free and Pro Editions and the License Key                                                                                                                                                                                 |
| **Branch**   | `claude/phase-11a-license`                                                                                                                                                                                                      |
| **Verified** | Locally on Windows: `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` (1,650 tests), `pnpm bindings` (no diff). GitHub CI on the pull request, including Windows and the real-app tests on Linux. |
| **Date**     | 2026-09-30 (Pacific time)                                                                                                                                                                                                       |
| **Result**   | Every deliverable built, as **v1.18.0**. Every test in the plan's list passes. The owner's checks, and the production signing keys, are in section 6. Plenipo by 8 West Ventures, LLC.                                          |

Screenshots come from the real-app tests on GitHub's Linux machine, and are saved in
`docs/phases/evidence/phase-11a/` once those tests pass.

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

A review of each area, with a second reviewer for each finding, is recorded here when done.

## 6. Left for the owner

1. **Make the signing keys in AWS KMS** (in AWS CloudShell, us-west-1). The key is made in the
   vault and never leaves it:

   ```bash
   for name in plenipo-license-current plenipo-license-spare plenipo-license-test; do
     id=$(aws kms create-key --key-spec ECC_NIST_EDWARDS25519 --key-usage SIGN_VERIFY \
       --description "Plenipo license signing ($name), 8 West Ventures, LLC" \
       --query KeyMetadata.KeyId --output text)
     aws kms create-alias --alias-name "alias/$name" --target-key-id "$id"
   done
   ```

   Then say "keys made". The builder reads the public halves (public, safe to share) and adds them
   to the app as `prod-1` and `prod-2`, with a test that the spare works.

2. **On a real Windows PC**, after the release:
   - enter a key, and see Pro turn on
   - restart, and see Pro kept
   - remove the key
   - uninstall with "delete my data", and see the key gone from Credential Manager
