# Phase 11A — Implementation Checklist

**Status: Phase 11A built for v1.18.0** (2026-09-30; [acceptance report](phase-11a-acceptance-report.md)).
Built on v1.17.0, beside Phase 16's third and fourth waves and Phase 22 (the 8 West account service,
in its own private repository). Below, "[x]" is done. Plenipo is made by 8 West Ventures, LLC.

Source: `ROLLOUT_PLAN.md`, Phase 11A — Free and Pro Editions and the License Key, and the records
written for it:

- [ADR-021 (Free and Pro editions under the Elastic License)](../adr/ADR-021-editions-and-license.md)
- [ADR-022 (subscription pricing and the weekly license check)](../adr/ADR-022-subscription-and-license-check.md)
- [ADR-068 (Connections and add-on tools are part of Pro)](../adr/ADR-068-connections-are-pro.md)
- [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](../adr/ADR-100-phase-11a-22-owners-answers.md)
  and the records it lists, ADR-101 to ADR-118. The ones for this phase:
  - ADR-104 (the signing key lives in AWS KMS; keys stay Ed25519)
  - ADR-105 (the domain and the check's address)
  - ADR-110 (one person, any of their PCs)
  - ADR-112 (lessons pause on Free)
  - ADR-113 (workers at the same time)
  - ADR-114 (business departments)
  - ADR-115 (a Free copy never contacts 8 West)
  - ADR-116 (the weekly answer is signed)
  - ADR-117 (paid AI keys are Free)

**Numbers:** ADR-100 to ADR-118. **No new Ledger layout** (it stays at 11). The license record is
`license.json` in Plenipo's data folder. The key is in the Vault as `plenipo-license-key`.

Dates are Pacific time. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): **Free**, **Plenipo Pro**, **part of Pro**,
**license key**, **the key's ID**, **the weekly check with 8 West**, **waits its turn**, **paused**.

**Goal (plan):** "Make the Free and Pro split real."

## In short, for the owner

- **Free** is what every copy is until a key is entered:
  - 1 organization, 1 department, 1 project, and 3 workers on the job at a time
  - every safety feature and every AI tool
  - it never contacts 8 West
- **Settings → License** takes a license key. Pro turns on at once, with no restart. The screen shows
  the key's ID (never the key), the plan, and the paid-through date. It has **Check now** and
  **Remove the key**.
- **The weekly check** sends only the key's ID and the version of Plenipo. Pro stays on for 30 days
  without a check, and changing the PC's clock does not add days.
- **When Pro ends, nothing is taken away.** Only new things past a Free limit wait. Connections,
  add-on tools, and lessons pause, and come back with Pro.

## Deliverables

- [x] **`crates/licensing/`**:
  - the edition model and key checking
  - the Entitlements snapshot (`Entitlements`)
  - the check's request and signed answer (`answer`), and the state (`state`, `license`)
- [x] **A signed license key** carrying the edition, holder, key ID, plan, and paid-through date:
      `plenipo1.<contents>.<signature>`, Ed25519 (ADR-104).
- [x] **Checked on the PC** against the public keys built into the app, at every start and on entry,
      with no network. (The production public keys go in once the owner makes them in AWS KMS; see
      "Left for the owner".)
- [x] **The weekly check to the 8 West license service**, sending the key's ID and the app version
      and nothing else. The contract is in `contracts/license-check/v1`, and both repositories test
      against it.
- [x] **30 days offline; fail-open on every error**, counted from 8 West's signed time (ADR-116).
- [x] **The key in the Vault** (Windows Credential Manager), under the first organization's name
      (ADR-110). Uninstalling with "delete my data" removes it.
- [x] **Settings → License**: enter a key, see the plan and the paid-through date, see when it last
      checked and when it checks next, **Check now**, and remove the key (asks first).
- [x] **One enforcement point**: `Entitlements::check(limit)` gives Allowed, or Blocked with plain
      words.
- [x] **Free's limits in Workforce and Liaison**: 1 department, 1 project, and 3 workers on the job
      at once, counted across the PC. The fourth worker waits its turn (ADR-113). Also 1 organization
      (ADR-110).
- [x] **Business departments** (from a business template) are Pro, checked when the team is set up
      (ADR-114).
- [x] **Connections and add-on tools** are Pro (ADR-068):
  - **Connect**, keys, another account, and **Add a program** are refused on Free
  - no Connection or add-on tools are offered to a task that started on Free
  - a task that started on Pro keeps its tools until it finishes
  - **Disconnect** always works
  - Pro back: they work again, with no new sign-in
- [x] **Lessons pause on Free** (ADR-112).
- [x] **Plain words on every blocked path**, naming what Pro adds and where to enter a key; each
      message is tested as written.
- [x] **Ledger events** for:
  - a key entered, refused (its reason), and removed
  - each check's answer, or why it failed
  - each change between Free and Pro

  Always by the key's ID, never the key.

- [x] **Lapse never deletes, hides, or breaks anything.** Only making something new past a limit
      waits.
- [x] **`docs/editions.md`** describes what ships: the prices, what the check sends, and what
      changed in ADR-113 and ADR-117.

## Tests (the plan's list)

- [x] Keys:
  - a valid key is accepted
  - a tampered payload is refused
  - a key signed by another key is refused
  - a malformed key is refused

  Tests in `key.rs`: `a_valid_key_is_accepted_with_what_it_says`,
  `a_tampered_payload_is_refused`, `a_key_signed_by_another_key_is_refused`,
  `malformed_keys_are_refused_in_plain_words`.

- [x] Free: a second department and a second project are refused; a fourth worker waits its turn
      (ADR-113 changed "blocked" to "waits"). Tests:
      `free_allows_one_department_and_one_project_and_refuses_the_second` and
      `free_runs_three_workers_at_once_and_the_fourth_waits_its_turn`.
- [x] Free: the whole Development flow finishes on 1 department, 1 project, and 3 workers.
      `development.e2e.mjs` now runs on Free, and
      `free_sets_up_development_once_and_never_half_a_team`.
- [x] Pro: departments and projects without limit, and workers at Pro's 4 per organization. Tests:
      `pro_makes_as_many_departments_and_projects_as_you_want` and `pro_never_waits_for_frees_limit`.
- [x] Business department setup is refused on Free and allowed on Pro:
      `a_business_template_is_part_of_pro`.
- [x] Connections and add-on tools (ADR-068):
  - `on_free_connecting_is_part_of_pro_and_disconnect_always_works`
  - `when_pro_ends_connections_pause_and_come_back_without_a_new_sign_in`
- [x] A key entered turns Pro on with no restart. The key removed is Free, with nothing deleted.
  - `a_key_makes_pro_and_the_check_sends_only_its_id_and_the_version`
  - `an_ended_subscription_or_a_removed_key_is_free_and_nothing_else_changes`
  - `license.e2e.mjs`
- [x] Lapse with three departments: everything is still listed and runnable, and only new ones are
      refused: `when_pro_ends_nothing_is_lost_and_only_new_ones_are_refused`.
- [x] **No internet: Pro stays on through day 30 and drops on day 31**:
      `pro_stays_30_days_without_a_check_and_the_clock_cannot_extend_it`.
- [x] **Service down, 500, timeout, or a garbage answer: Pro stays on and the check retries**:
  - `failed_checks_keep_pro_on_and_retry_later`
  - `garbage_from_the_service_is_a_failure_and_pro_stays_on`
  - `another_status_or_a_redirect_is_a_failed_check`
  - `no_internet_is_a_failed_check_in_plain_words`
- [x] **Cancelled: Pro stays until the end of the paid period, then drops**:
      `cancelled_keeps_pro_until_the_end_of_the_paid_period_then_drops`.
- [x] **The check's body is exactly the key's ID and the app version, byte for byte**:
  - `the_check_sends_exactly_the_key_id_and_the_app_version` (the exact headers too)
  - `the_contract_files_match_the_code_byte_for_byte`
  - `license.e2e.mjs` test 3
  - the account service's contract tests
- [x] **A Free copy makes no request to 8 West** (ADR-115):
  - `a_free_copy_never_contacts_8_west`
  - `the_license_check_reaches_only_its_one_address`
  - `license.e2e.mjs` test 1
  - `development.e2e.mjs`, last test
- [x] Winding the clock back does not extend the 30 days:
      `winding_the_clock_back_does_not_extend_the_grace`, and
      `pro_stays_30_days_without_a_check_and_the_clock_cannot_extend_it`.
- [x] Every blocked path's plain words, snapshot tested:
  - `every_blocked_path_says_what_free_has_what_pro_adds_and_where_to_go`
  - `the_messages_are_as_written`
  - `a_free_limit_reaches_the_screen_as_part_of_pro`
- [x] The Ledger records a key entered, refused, and removed, and each check's result, with the key
      redacted:
  - `a_key_makes_pro_and_the_check_sends_only_its_id_and_the_version`
  - `a_free_copy_never_contacts_8_west`
- [x] Permissions, approvals, and Guard behave the same on Free and Pro:
      `guard_and_permissions_behave_the_same_on_free_and_pro`.

## Beyond the list

- [x] The new commands are for organizations' windows only. IPC tests refuse other windows, the
      sign, a pop-out, and web pages: `the_license_commands_are_an_organization_windows_alone`.
- [x] Settings → License never shows the key: `settings_license_never_shows_the_key`,
      `license.test.tsx`.
- [x] A second organization, a copy, or bringing one back is part of Pro on Free:
      `on_free_a_second_organization_is_part_of_pro_and_nothing_is_lost`.
- [x] The same key entered again never restarts the 30 days:
      `entering_the_same_key_again_never_restarts_the_30_days`.
- [x] A Vault that cannot be read leaves the record alone:
      `a_vault_that_cannot_be_read_leaves_the_record_alone`.
- [x] Only a copy built for the tests can point the check at a stand-in:
      `a_copy_checks_only_8_wests_one_address_unless_built_for_the_tests`.

## Left for the owner

- [ ] **Make the two signing keys in AWS KMS** (the command is in the acceptance report). Then the
      builder adds their public halves to the app as `prod-1` and `prod-2`. Until then, a released copy
      accepts no key, so every copy stays Free.
- [ ] **On a real Windows PC**:
  - enter a test key in Settings → License, and see Pro turn on
  - restart, and see Pro kept (Windows Credential Manager)
  - remove the key
  - uninstall with "delete my data", and see the key gone from Credential Manager
