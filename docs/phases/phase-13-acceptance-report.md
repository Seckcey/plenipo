# Phase 13 — Acceptance Report

|              |                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 13 — Windows Service, Installer, Updates, and Recovery                                                                                                                                                                                                                                                                                                     |
| **Branch**   | `claude/phase-13` (PR_LINK)                                                                                                                                                                                                                                                                                                                                |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings` (no diff), and the full `pnpm e2e` (E2E_TOTAL tests) against the release build. GitHub CI: Rust, Frontend, E2E (Linux), and Windows, where the installer tests install, run, crash, upgrade, update, and uninstall the real installer — see the pull request.                     |
| **Date**     | 2026-09-27                                                                                                                                                                                                                                                                                                                                                 |
| **Result**   | All three acceptance criteria and all nine Phase 13 tests pass; every deliverable is built. Version **1.9.0**. Decisions: ADR-036 (background work) and ADR-037 (updates), both accepted by the owner and built. A real Windows restart, Start with Windows after signing in, and a real update from GitHub are the owner's checks on Windows (section 7). |

Screenshots (from the end-to-end run in the real app):

- **How Plenipo last stopped:** [after it was ended the hard way](evidence/phase-13/recovery-closed-unexpectedly.png)
- **Settings:** [Start and close](evidence/phase-13/settings-start-and-close.png) ·
  [Updates, with a new version ready](evidence/phase-13/settings-updates-ready.png) (and **Update
  ready** in the top bar)
- **Diagnostics:** [backups of the Ledger](evidence/phase-13/diagnostics-backups.png) ·
  [a diagnostics file saved](evidence/phase-13/diagnostics-file.png)

Test totals: **RUST_TOTAL Rust** · **FRONTEND_TOTAL frontend** (304 design system + APP_TOTAL app) ·
**E2E_TOTAL end-to-end** tests against the real release binary · **the Windows installer tests**
on GitHub's Windows machine (section 6).

On screen the plan's words become plain ones ([word list](../design/vocabulary.md)): the
"daemon" is Plenipo **in the tray**, "startup control" is **Start Plenipo with Windows**, a
"crash" is **Plenipo closed unexpectedly**, a "reboot" is **Windows restarted while Plenipo was
running**, a "migration" is **updating the Ledger**, a "provider process crash" is **AI tool
stopped unexpectedly**, a "diagnostics bundle" is a **diagnostics file**, and "log rotation"
keeps five **log files**. Quotes from the plan keep the plan's words.

CI has no AI tool accounts, and tests never touch the internet: the update tests use a throwaway
updater key made in each run and a local server on `127.0.0.1`, and Windows restarts are
simulated (Plenipo is ended, and its "running" note is dated before Windows started).

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                      | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| --- | ---------------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Plenipo installs and updates cleanly on a fresh Windows machine. | **Pass** | Windows installer tests (CI, `scripts/windows/installer-tests.ps1`): a clean install is listed in Windows' Apps list as 1.9.0 with its Start menu shortcut, runs, and keeps its Ledger and log; an upgrade over the published 1.8.0 backs up the Ledger first; an update is found, checked, backed up, handed to the installer, and installed the way Plenipo does it; uninstalling leaves nothing behind when asked to delete the data. The owner's check on a real PC is listed in section 7. |
| 2   | A UI restart does not lose the Ledger.                           | **Pass** | The Ledger lives in the one Plenipo process, not the window (ADR-036). A crashed window is brought back while the work goes on (Windows test: the window's WebView2 programs are ended; Plenipo reloads or reopens it, records `plenipo.window_recovered`, and exits cleanly). Closing the window hides Plenipo while work is going (`start_close` tests). Ending Plenipo itself keeps the Ledger and says what stopped (E2E, Windows tests).                                                   |
| 3   | The user can understand and recover from a failed runtime.       | **Pass** | Every way Plenipo, Windows, the window, or an AI tool can stop is explained in plain words at the next start or on the task, with what stopped and **Run again** or **Leave stopped** (section 3; `upkeep.test.tsx`, `recovery` tests, E2E [screenshot](evidence/phase-13/recovery-closed-unexpectedly.png)). Diagnostics restores a backup and saves a diagnostics file; damaged settings can be reset after a backup.                                                                         |

## 2. Deliverables → evidence

| Plan deliverable              | Built as                                                                                                                                                                                                                                                                                                                                                                           | Tests                                                                                                                                                                                                                          |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **signed installer path**     | The Release workflow signs as 8 West Ventures, LLC (as before), then signs the installer with the updater key (`createUpdaterArtifacts` in `tauri.signing.conf.json` only), checks that signature and the version in it (`verify_update` example), writes `latest.json` (`scripts/update-manifest.mjs`), and publishes both. It refuses to start without the three updater values. | `update-manifest.test.mjs`; capabilities `a_signed_newer_release_is_found_downloaded_and_checked`, `a_download_that_is_not_ours_or_not_the_version_it_claims_is_refused`; Windows test (a signed update installs).             |
| **uninstall**                 | `windows/hooks.nsh` and `windows/English.nsh`: asks a running Plenipo to quit cleanly; keeps your data unless **Also delete my Plenipo data** is ticked (`/DELETEAPPDATA` when silent), which also removes the secrets in Windows Credential Manager (`uninstall.rs`, `--plenipo-forget-secrets`); Start with Windows and shortcuts always go.                                     | `uninstall` `deleting_my_data_forgets_every_secret_plenipo_kept`, `the_mode_is_only_for_its_own_argument`; Windows tests (keeping, then deleting, your data; what is left behind).                                             |
| **upgrade path**              | The installer asks a running 1.9+ to quit the normal way; the new version's first start backs up the Ledger before using it and records `plenipo.version_changed` (`backup_host.rs`).                                                                                                                                                                                              | `backup_host` `a_new_version_backs_up_the_ledger_before_its_first_use`, `a_ledger_from_before_1_9_is_backed_up_as_an_earlier_version`, `an_update_that_already_made_a_backup_is_not_backed_up_twice`; Windows test over 1.8.0. |
| **background service/daemon** | ADR-036 option D: the work stays in the one Plenipo process, in the tray; the window comes and goes. A second launch shows the first (`tauri-plugin-single-instance`); `--quit` quits it cleanly.                                                                                                                                                                                  | `start_close` tests; Windows test (a second launch hands over; `--quit` leaves no "running" note).                                                                                                                             |
| **startup control**           | Settings → **Start and close**: **Start Plenipo with Windows** (off by default; `tauri-plugin-autostart`, in the tray with no window), and what closing the window does (keep while work is going, always keep, quit).                                                                                                                                                             | `upkeep.test.tsx` Start and close (2); `start_close` `the_choice_is_kept_in_the_ledger_and_read_safely`; IPC `the_start_with_windows_plugin_is_reachable_by_no_window`; E2E upkeep (the choice is kept after a restart).       |
| **system tray**               | Reused. Show opens the window the same way a second launch does; Quit stops the work cleanly.                                                                                                                                                                                                                                                                                      | Earlier tray tests; Windows test (`--quit`).                                                                                                                                                                                   |
| **crash recovery**            | `recovery.rs` (the "running" note and how the last run ended), `window_watch.rs` (the window brought back), `RecoveryBanners.tsx` (the notice above the page, Run again, Leave stopped), a pop-up notice (Notifications → **Plenipo itself**).                                                                                                                                     | `recovery` tests (8), `window_watch` tests (5), `upkeep.test.tsx` recovery (3), `App.test.tsx` Phase 13 (2), Ledger `plenipo_says_when_it_recovered_…`; E2E; Windows tests.                                                    |
| **database backup**           | Ledger `backups.rs`: backups by kind (yours, daily, before a new version, before an update, before a layout change, before a restore), each kept to its own number; the daily one waits for idle; **Restore** from Diagnostics at the next start, keeping the Ledger as it was.                                                                                                    | Ledger `backups_of_each_kind_…`, `a_restore_happens_at_the_next_start_…`, `a_restore_that_cannot_be_done_…`, `a_backup_from_a_newer_plenipo_…`, `an_interrupted_layout_change_…`; `backup_host` daily (2); E2E backups.        |
| **log rotation**              | `logs.rs`: `logs\plenipo.log`, 2 MB each, five kept; every line redacted first.                                                                                                                                                                                                                                                                                                    | `logs` tests (6), including `secrets_never_reach_the_file`; E2E (nothing typed in the terminal reaches the log).                                                                                                               |
| **diagnostics bundle**        | `diagnostics.rs`: **Save a diagnostics file** (Diagnostics, Settings → Diagnostics): `README.txt`, `about.json`, `recent-events.json` (types and times only), and the log files; five kept.                                                                                                                                                                                        | `diagnostics` tests (2); `upkeep.test.tsx`; E2E (the file's contents, and nothing typed in the terminal).                                                                                                                      |
| **safe update mechanism**     | ADR-037: `update_host.rs`, capabilities `updates.rs`, Guard `outbound.rs`; Settings → **Updates** and **Update ready** in the top bar. Checking once a day (Free and Pro); installing only on **Install now**, only a release signed for the version it claims; a backup first; any failure before the installer starts leaves this version running.                               | capabilities updates (5), Guard outbound (3), `update_host` (5), `upkeep.test.tsx` Updates (3), `App.test.tsx`; E2E (found, shown, a failed install leaves this version); Windows test (installed).                            |
| **version display**           | The top bar and Settings → About (reused), Settings → Updates, the diagnostics file, and Windows' Apps list.                                                                                                                                                                                                                                                                       | E2E upkeep; Windows test (`DisplayVersion`).                                                                                                                                                                                   |

## 3. Recovery states

The plan asks to "define recovery states". Each is recorded in the Ledger, told in plain words,
and tested.

| State (plan)                      | What Plenipo does, in plain words                                                                                                                                                                                                                     | Tests                                                                                                                 |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| UI crash                          | The window's page answers every 5 seconds. Quiet for 30 seconds while showing: reloaded; still quiet: closed and opened again. The work goes on. "The window stopped unexpectedly at … and was reloaded (or opened again). Your work kept running."   | `window_watch` (5); Windows test `window-crash` (its WebView2 programs ended).                                        |
| daemon crash                      | "Plenipo closed unexpectedly at …": the tasks and programs that stopped, with **Run again** or **Leave stopped**, and a pop-up notice.                                                                                                                | `recovery`; E2E; Windows test (ended with a task and a program running).                                              |
| Windows reboot                    | "Windows restarted at … while Plenipo was running": the same, with the cause told apart (its last heartbeat is before Windows started).                                                                                                               | `recovery` `a_note_left_behind_tells_a_crash_from_a_windows_restart`; Windows tests (idle, and with a task).          |
| provider process crash            | As before, only that AI tool's task stops: the conversation says **AI tool stopped unexpectedly** (was "Crashed"), and the task shows in **What's stuck** on Home, where it can be given again.                                                       | Runtime `agents` (a crashed turn), Workforce (a failed objective is stuck).                                           |
| incomplete task                   | Listed at the next start with who had it; **Run again** gives the same request to the same worker as a new task; **Leave stopped** keeps it stopped. Nothing runs again by itself.                                                                    | `recovery` `run_again_gives_the_owners_objective_to_the_same_worker`, `a_recovery_is_recorded_…`; `upkeep.test.tsx`.  |
| interrupted database migration    | "Plenipo was stopped at … while updating the Ledger. Nothing was lost: the unfinished step was undone and done again, with a backup from before it."                                                                                                  | `recovery` `a_note_left_while_changing_the_layout_…`; Ledger `an_interrupted_layout_change_is_undone_and_done_again`. |
| (also) damaged settings or Ledger | Settings Plenipo could not read are named, with **Restore a backup** or **Reset to starting settings** (a backup first). A damaged Ledger is set aside and a new one started, as before. A damaged "running" note is an unclean end of unknown cause. | `settings_health`; Windows tests (a damaged note, a damaged Ledger).                                                  |

## 4. Plan tests → evidence

| Plan test                             | Test                                                                                                                                              |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| clean install                         | Windows installer tests: **Clean install** (silent install, Apps list, Start menu, first run, Ledger and log in their folders, a clean exit).     |
| upgrade                               | Windows: **Upgrade from the published 1.8.0 installer** (a backup before 1.9.0 first uses the Ledger; the upgrade recorded); `backup_host` tests. |
| uninstall                             | Windows: **Uninstall, keeping your data**, **Uninstall, deleting your data**, **What is left behind** (nothing); `uninstall` tests.               |
| reboot during idle                    | Windows: **Windows restart while idle (simulated)**; `recovery` tests. A real restart: the owner (section 7).                                     |
| reboot with recoverable task metadata | Windows: **Windows restart with a task running (simulated)** (the task is named at the next start); `recovery` tests.                             |
| forced crash                          | Windows: **Forced crash while a task and a program run**; E2E upkeep (ended the hard way, then told).                                             |
| corrupted config                      | Windows: **A damaged "running" note**, **A damaged Ledger**; `settings_health` (damaged permissions reset after a backup).                        |
| database backup/restore               | Ledger backup and restore tests (5); `backup_host`; `upkeep.test.tsx`; E2E upkeep (Create backup, listed by why).                                 |
| version rollback strategy             | Windows: **Rollback: back to 1.8.0, then forward again** (same Ledger); a failed update leaves this version (`update_host`, E2E); ADR-037 §4.     |

## 5. The owner's decisions and rules → evidence

| Decision or rule (2026-09-27)                                                  | Evidence                                                                                                                                                                             |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ADR-036 (background work), accepted as recommended                             | Built as written (section 2); no separate Windows service.                                                                                                                           |
| ADR-037 (updates), accepted; checking always on for Free and Pro               | `Updates::checks_by_itself` in every copy with the updater key; no switch; README, `docs/editions.md`, and `SECURITY.md` say so.                                                     |
| No secrets in chat; the updater key only in GitHub secrets, added by the owner | [Code signing → the updater key](../development/code-signing.md#updates-the-updater-key-phase-13-adr-037); CI makes a throwaway key in each run; nothing secret in the repository.   |
| The network only through Guard                                                 | `Guard::check_outbound` with `Purpose::Updates` on every request and redirect; refusals recorded (`guard_refuses_any_address_but_the_releases_and_records_it`).                      |
| New commands for the main window only                                          | IPC `keeping_plenipo_dependable_is_the_main_windows_alone` (the sign window and web pages are refused), `the_start_with_windows_plugin_is_reachable_by_no_window`.                   |
| Updates install only when the owner says so, and only releases 8 West signed   | `install_update` is the only way in; `a_download_that_is_not_ours_or_not_the_version_it_claims_is_refused`.                                                                          |
| Logs and diagnostics files hold no secrets and nothing typed in the terminal   | `secrets_never_reach_the_file`, `the_file_holds_what_helps_and_nothing_of_the_owners_work_or_secrets`; E2E types a line in the terminal and checks the diagnostics file and the log. |
| Version 1.9.0                                                                  | Every manifest, `Cargo.lock`, `scripts/check-versions.mjs`, and the Phase 13 row in `docs/development/versioning.md`.                                                                |

## 6. Test totals

- **RUST_TOTAL Rust** tests (Linux), including the recovery (8), window watch (5), backups (Ledger
  5, desktop 5), updates (capabilities 5, desktop 5, Guard 3), logs (6), diagnostics (2),
  uninstall (2), and the IPC checks.
- **FRONTEND_TOTAL frontend** tests: **304** in the design system and **APP_TOTAL** in the app.
- **E2E_TOTAL end-to-end** tests against the real release binary (Linux), including **4 new** in
  the Phase 13 group.
- **The Windows installer tests** on GitHub's Windows machine (in every pull request's Windows
  check): clean install, a forced crash with work running, Windows restarts while idle and with a
  task (simulated), a damaged "running" note, a crashed window, one Plenipo at a time and
  `--quit`, uninstall keeping your data, upgrade from 1.8.0, back to 1.8.0 and forward, an update
  installed the way Plenipo does, a damaged Ledger, uninstall deleting your data, and what is
  left behind.

## 7. Notes

- **Checked on Windows by the owner** (a real PC is needed):
  1. A fresh install, then an upgrade from 1.8.0, and an uninstall both ways (keep, then delete
     your data), by hand.
  2. A real Windows restart with work running: the next start says **Windows restarted while
     Plenipo was running** and lists the task.
  3. **Start Plenipo with Windows** on, then sign out and in: Plenipo is in the tray, with no
     window.
  4. An update from GitHub, once the updater key is in GitHub (ADR-037 §6) and a release after
     1.9.0 is out.
- **ADR-036 (background work) and ADR-037 (updates) are accepted** by the owner (2026-09-27).
- **From earlier phases, still to do on Windows:** Phase 11's check against a real server, and
  Phase 12's real notices and the terminal with PowerShell and a real server.
- **Deviations**, recorded in the ADRs: no separate Windows service (ADR-036, option D); no
  Windows restart registration and no WebView2 crash events, because Plenipo forbids `unsafe`
  code (the page's heartbeat stands in, ADR-036); Plenipo's own updater instead of Tauri's, which
  quits without stopping the work (ADR-037).
