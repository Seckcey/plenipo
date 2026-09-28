# Phase 13 — Implementation Checklist

**Status:** delivered in v1.9.0 (2026-09-27). Built on v1.8.0 (Phase 12).
Acceptance report: [`phase-13-acceptance-report.md`](phase-13-acceptance-report.md).

Source: `ROLLOUT_PLAN.md`, Phase 13 — Windows Service, Installer, Updates, and Recovery,
[ADR-037 (background work)](../adr/ADR-037-background-work.md), and
[ADR-038 (updates)](../adr/ADR-038-updates.md).

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): the plan's "daemon" is Plenipo **in the
tray**, a "crash" is Plenipo **closing unexpectedly**, a "migration" is **updating the Ledger**, a
"diagnostics bundle" is a **diagnostics file**.

**Goal (plan):** "Make Plenipo dependable as installed Windows software."

## Owner decisions (2026-09-27)

- **The plan for this phase is approved**, as version **1.9.0**: installs, updates, and
  uninstalls cleanly on a fresh Windows computer; closing, crashing, or restarting the window
  never loses the Ledger or stops approved work unless configured; after a crash the owner can
  see what happened and recover.
- **ADR-037 (background work): accepted, as recommended (option D).** The work stays in the one
  Plenipo program, which lives in the tray; no separate Windows service.
- **ADR-038 (updates): accepted, as recommended**, with the owner's choice: **update checking is
  always on, for Free and Pro**, with no switch. Installing still happens only when the owner
  says so, and only a release signed with 8 West's updater key.
- Numbers: **ADR-037 and ADR-038** (drafted as ADR-035 and ADR-036; pull requests #69 and #71 took
  ADR-034 to ADR-036 first). No Ledger migration (the Ledger's layout is unchanged; backups are
  told apart by their file names, and Plenipo's own state uses the existing `settings` table).
- The owner adds the updater key to GitHub themselves (two secrets and one variable, ADR-038 §6);
  nothing secret is committed or pasted into a chat.
- Out of scope (plan): Microsoft Store distribution, macOS and Linux packaging.

## Design (2026-09-27)

Written before building, from a map of the code.

- **Background work (ADR-037).** One per-user Plenipo process, in the tray. The main window
  starts hidden and is shown unless Plenipo started with `--in-tray`. Closing the window follows
  the owner's choice (keep while work is going, the default; always keep; quit). A second launch
  shows the first (`tauri-plugin-single-instance`, Windows); `--quit` asks the running one to
  quit the normal way. Start with Windows through `tauri-plugin-autostart` (`HKCU\…\Run`,
  `Plenipo`, `--in-tray`), off by default. A window watch (the page's heartbeat) reloads, then
  reopens, a window that stops answering.
- **Recovery states.** A "running" note with a heartbeat and the step Plenipo is on; its absence
  means a clean exit. Left behind, it tells a crash from a Windows restart (the heartbeat is
  before Windows started) and from an interrupted layout change; a damaged note is "unknown".
  The next start lists the tasks and programs that stopped and offers Run again or Leave
  stopped. `unsafe` stays forbidden, so no Windows restart registration and no WebView2 events:
  the heartbeat stands in (ADR-037 records it).
- **Backups and restore.** Kinds by file-name prefix, each kept to its own number; a daily
  backup that waits for idle; one before a new version first uses the Ledger; one before an
  update; restore as a request applied at the next start, before the Ledger opens.
- **Logs and diagnostics.** The `log` crate into rotating files, every line through Guard's
  redactor and the broker's secret filter; a zip of what helps, never the owner's work.
- **Updates (ADR-038).** Plenipo's own updater (Tauri's quits without stopping the work):
  `latest.json` from GitHub Releases through Guard's outbound rules, the updater signature and
  the signed version checked, backup, stop the work, start the installer, quit.
- **Installer.** The existing NSIS per-user installer, with hooks: quit a running 1.9+ cleanly,
  keep data unless asked, forget the secrets when deleting data; plain words in its own language
  file.

## Deliverables (plan)

- [x] **Signed installer path** — the Release workflow signs as 8 West Ventures, LLC (as before)
      and now also signs the installer with the updater key, checks that signature and its
      version, and publishes `.sig` and `latest.json` (`.github/workflows/release.yml`,
      `scripts/update-manifest.mjs`).
- [x] **Uninstall** — keeps the Ledger, backups, and settings unless "Also delete my Plenipo data"
      is ticked (`/DELETEAPPDATA` when silent), which also removes the secrets in Windows
      Credential Manager (`uninstall.rs`); Start with Windows and shortcuts always go.
- [x] **Upgrade path** — installing over a running 1.9+ asks it to quit cleanly
      (`windows/hooks.nsh`); the first start of a new version backs up the Ledger
      (`backup_host.rs`) and records `plenipo.version_changed`. Tested over the published 1.8.0.
- [x] **Background service/daemon** — ADR-037 option D: Plenipo in the tray, the window
      separate from the work (`start_close.rs`, `lib.rs`).
- [x] **Startup control** — Settings → Start and close: Start Plenipo with Windows (off by
      default) and what closing the window does.
- [x] **System tray** — reused; its Show opens the window the same way a second launch does.
- [x] **Crash recovery** — `recovery.rs`, `window_watch.rs`; the notice above the page with Run
      again and Leave stopped; a pop-up notice (Notifications → Plenipo itself).
- [x] **Database backup** — daily, before a new version, before an update, before a restore,
      and by you; restore from Diagnostics (Ledger `backups.rs`, `BackupsPanel.tsx`).
- [x] **Log rotation** — `logs\plenipo.log`, 2 MB each, five kept (`logs.rs`).
- [x] **Diagnostics bundle** — Save a diagnostics file (`diagnostics.rs`).
- [x] **Safe update mechanism** — ADR-038 (`update_host.rs`, capabilities `updates.rs`,
      `guard::outbound`, `UpdateSettings.tsx`).
- [x] **Version display** — the top bar and Settings → About (reused), Settings → Updates, the
      diagnostics file, and the installer's entry in Windows' Apps list.

## Technical implementation (plan)

- [x] "Separate UI lifecycle from long-running task lifecycle." The window can close, crash, and
      reopen while the work goes on (ADR-037).
- [x] "Closing the main window must not accidentally kill authorized long-running work unless the
      user configured that behavior." The default keeps Plenipo in the tray while work is going;
      only the owner's "Quit Plenipo and stop its work" choice (or Quit) stops it.
- [x] Recovery states defined, each in plain words
      ([report, section 3](phase-13-acceptance-report.md#3-recovery-states)):
  - [x] UI crash — the window is reloaded, then reopened; the work goes on.
  - [x] Daemon crash — Plenipo closed unexpectedly: what stopped, Run again or Leave stopped.
  - [x] Windows reboot — Windows closed Plenipo (a restart, a shutdown, or signing out): the same, with the cause.
  - [x] Provider process crash — the AI tool stopped unexpectedly: only its task stops, and shows
        in What's stuck (as before; the label is now plain words).
  - [x] Incomplete task — listed with Run again (a new task, same request, same worker) or Leave
        stopped; nothing runs again by itself.
  - [x] Interrupted database migration — the unfinished step is undone and done again, from a
        backup made before it; the owner is told.

## Tests (plan)

- [x] Clean install — Windows installer tests (CI).
- [x] Upgrade — over the published 1.8.0 installer (CI); `backup_host` tests.
- [x] Uninstall — keeping, then deleting, your data, and what is left behind (CI); `uninstall`
      tests.
- [x] Reboot during idle — simulated on the Windows machine (the note dated before Windows
      started); `recovery` tests. A real restart is the owner's check.
- [x] Reboot with recoverable task metadata — the same, with a task running: named at the next
      start.
- [x] Forced crash — Plenipo ended from outside with a task and a program running (CI, E2E).
- [x] Corrupted config — a damaged "running" note, a damaged Ledger (set aside), and damaged
      permission or routing settings (reset after a backup).
- [x] Database backup/restore — Ledger tests (each kind, restore at the next start, a bad or newer
      backup refused, an interrupted layout change); E2E backups.
- [x] Version rollback strategy — back to 1.8.0 and forward again (CI); a failed update keeps this
      version; ADR-038 §4.

## Owner's rules for this phase

- [x] Plain words on screen ([vocabulary](../design/vocabulary.md) has the new pairs); ADRs
      named, not just numbered.
- [x] No secrets asked for in chat; the updater key goes into GitHub secrets by the owner only;
      nothing secret committed (CI makes throwaway keys in each run).
- [x] Anything touching the network goes through Guard: the update check and download use
      Guard's outbound rules, and a refusal is recorded (`guard.request_refused`).
- [x] New desktop commands are the main window's alone; the sign window and web pages are
      refused (IPC tests `keeping_plenipo_dependable_is_the_main_windows_alone`,
      `the_start_with_windows_plugin_is_reachable_by_no_window`).
- [x] Updates install only when the owner says so, and only releases 8 West signed.
- [x] Logs and diagnostics files never hold secrets or what is typed in the terminal (`logs`,
      `diagnostics` tests; E2E types a line and checks the file and the log).
- [x] Version 1.9.0 everywhere, with the Phase 13 row in `docs/development/versioning.md`.
- [x] Release notes (`docs/releases/v1.9.0.md`), the plan's Phase 13 status line, this
      checklist, and the acceptance report with screenshots in `evidence/phase-13/`.
- [x] Before each push: `pnpm check`, `cargo fmt --all -- --check`,
      `cargo clippy --workspace --all-targets --locked -- -D warnings`,
      `cargo test --workspace --locked`, `pnpm bindings` with no diff.

## Left for the owner (on Windows)

These need a real Windows PC and are listed in the acceptance report:

- A fresh install, an upgrade from 1.8.0, and an uninstall (both ways), by hand.
- A real Windows restart with work running, and Start with Windows after signing in.
- An update from GitHub, once the updater key is in GitHub and a release after 1.9.0 is out.
- Still open from earlier phases: Phase 11's check against a real server, and Phase 12's real
  notices and terminal (PowerShell and a real server).
