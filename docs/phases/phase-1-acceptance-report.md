# Phase 1 — Acceptance Report

|                     |                                                                                                                          |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| **Phase**           | 1 — Desktop Shell and Local Runtime Supervisor                                                                           |
| **Branch**          | `claude/inspiring-cray-qdwzmr`                                                                                           |
| **Commit verified** | `c46da5a`                                                                                                                |
| **CI run**          | [36207761549](https://github.com/Seckcey/plenipo/actions/runs/36207761549) — Frontend ✅ · Rust ✅ · E2E ✅ · Windows ✅ |
| **Date**            | 2026-09-26                                                                                                               |
| **Result**          | **All Phase 1 acceptance criteria pass.** Manual checks and owner items in §6.                                           |

Test totals on `c46da5a`: **84 Rust** (Linux and Windows) · **23 frontend** · **6 end-to-end**
against the real release binary.

## 1. Acceptance criteria → evidence

| #   | Criterion                                                         | Result   | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| --- | ----------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| A1  | Plenipo can launch, observe, and terminate a local child process. | **Pass** | Runtime integration tests (Linux + Windows CI): `launches_process_and_streams_stdout_and_stderr_incrementally`, `cancels_long_running_process`, `cancel_terminates_the_whole_process_tree`, `enforces_maximum_runtime`. E2E against the real app: tests 2 (launch + observe) and 4 (cancel; PID confirmed gone).                                                                                                                                                                                                               |
| A2  | Live process activity is visible in the UI.                       | **Pass** | E2E test 2 asserts the UI showed `stdout line 1 of 10` **before** `echo complete` existed (i.e. streamed, not dumped at exit), then stderr lines rendered as stderr. Screenshots: [echo succeeded](evidence/phase-1/runtimes-echo-succeeded.png), [running after reload](evidence/phase-1/runtimes-running-after-reload.png). Frontend tests cover rendering of output batches and lifecycle states.                                                                                                                           |
| A3  | Process state survives expected UI transitions.                   | **Pass** | View navigation: frontend test `keeps runtime state while navigating between views`. Webview reload: E2E test 4 reloads mid-run and verifies the same PID is still alive, heartbeats keep increasing, and the view/selection are restored. App quit + relaunch: E2E test 6 verifies history (succeeded, failed, cancelled-at-shutdown) is intact after restart. Crash of a previous session: `metadata_persists_and_restart_recovers_interrupted_runs` → `interrupted`. Close-to-tray: implemented, **manual check** (§6, M1). |
| A4  | The frontend cannot directly execute arbitrary OS commands.       | **Pass** | No command accepts an executable, args, env, or directory (ADR-005). E2E test 5 runs code **inside the webview** that tries: `start_execution` with `/bin/sh`, injected `executable`/`args`, `plugin:shell                                                                                                                                                                                                                                                                                                                     | execute`, `plugin:fs | read_text_file`, and an invented command — all refused (see §3). IPC boundary tests: unknown/malformed profile IDs, injected fields, ungranted windows, remote origins all rejected. ESLint forbids raw `invoke`/`listen` outside the typed clients. |

## 2. Required Phase 1 tests → evidence

| Test (ROLLOUT_PLAN.md)                           | Result   | Evidence                                                                                                                                                                                                                                                                                   |
| ------------------------------------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Launch a harmless local test process             | **Pass** | Built-in diagnostic profiles run Plenipo itself with `--plenipo-diagnostic=<scenario>`; `launches_process_…`, E2E 2                                                                                                                                                                        |
| Receive incremental stdout                       | **Pass** | ≥3 separate output batches during a ~2 s run (`launches_process_…`); E2E 2 early-line assertion                                                                                                                                                                                            |
| Receive stderr                                   | **Pass** | 3 stderr lines tagged `stderr`, seq-ordered with stdout; E2E 2 checks `data-stream="stderr"`                                                                                                                                                                                               |
| Cancel a long-running test process               | **Pass** | `cancels_long_running_process` (< 8 s, idempotent re-cancel), `cancel_terminates_the_whole_process_tree` (grandchild PID gone), E2E 4                                                                                                                                                      |
| Detect normal and abnormal exits                 | **Pass** | Exit 0 → `succeeded`; exit 3 → `failed · exit 3` (`detects_abnormal_exit`, E2E 3); signal → `Terminated by signal N`; spawn failure → `failed` with reason (`spawn_failure_becomes_failed_execution`)                                                                                      |
| Restart the UI without orphaning owned processes | **Pass** | Webview reload keeps the process (E2E 4). Quit/SIGTERM terminates the tree and records it (E2E 6, `shutdown_terminates_everything_…`). Owner **crash** on Windows: `children_do_not_outlive_a_crashed_owner` passed in Windows CI (Job Object kill-on-close). Linux hard-crash caveat: R1. |
| Reject executable paths outside configured rules | **Pass** | `rejects_executable_not_on_allowlist`, `rejects_relative_paths`, `traversal_resolves_before_comparison`, `symlink_to_disallowed_file_is_rejected` (Unix), `profiles_outside_executable_rules_are_rejected`, `executable_is_rechecked_at_spawn_time`                                        |

## 3. Failure paths and security checks

| Scenario                                                                          | Observed                                                                                                                                     |
| --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Webview calls `start_execution({profileId:"/bin/sh"})`                            | `invalidInput: invalid profile id: "/bin/sh"`                                                                                                |
| Webview injects `executable`/`args`                                               | `invalidInput: unknown launch profile: sh` (fields ignored)                                                                                  |
| Webview calls `plugin:shell\|execute`, `plugin:fs\|read_text_file`, `run_command` | `… not allowed by ACL`                                                                                                                       |
| Child environment                                                                 | Only OS baseline + declared vars; `CARGO_*` from the test process never reached the child (`child_environment_is_isolated`, Linux + Windows) |
| Max runtime exceeded                                                              | `timedOut`, tree killed                                                                                                                      |
| Invalid working directory / unknown profile / unknown execution                   | Rejected before spawn; nothing recorded                                                                                                      |
| 5,000-line burst + 100 KB line                                                    | All lines streamed in ≥25 batches; ring buffer capped at 1,000; long line cut to 8 KiB and flagged                                           |
| Invalid UTF-8 output                                                              | Replaced, not fatal                                                                                                                          |
| Corrupt / future-version history file                                             | Quarantined to `*.corrupt-<ts>.json` and reported as a notice                                                                                |
| Illegal state transitions                                                         | Rejected by the state machine (unit tests)                                                                                                   |
| Output events vs. snapshot after reload                                           | Deduplicated by `seq` (reducer tests + E2E 4)                                                                                                |
| Flakiness                                                                         | Supervisor suite run 5× locally, E2E 3× locally + CI, all green                                                                              |

## 4. Deliverables

| Deliverable                                                | Location                                                                                    |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Main desktop shell + navigation frame                      | `apps/desktop/src/App.tsx`, `components/Sidebar.tsx`                                        |
| Organization placeholder (data-driven, nothing hard-coded) | `views/OrganizationView.tsx` (test: `does not hard-code departments`)                       |
| Active runtimes / process status screen                    | `views/RuntimesView.tsx`, `components/OutputPanel.tsx`                                      |
| Task/activity panel                                        | `views/ActivityView.tsx` (derived from history, identical after reload)                     |
| Settings                                                   | `views/SettingsView.tsx` (read-only policy summary)                                         |
| Logs / developer diagnostics                               | `views/DiagnosticsView.tsx` (notices, recent raw events)                                    |
| System tray                                                | `src-tauri/src/tray.rs` (show, active count, stop all, quit)                                |
| Local runtime supervisor + lifecycle management            | `crates/runtime`                                                                            |
| Event streaming Rust → React                               | `runtime_host.rs` (`plenipo://runtime`), `src/api/events.ts`, `runtime/RuntimeProvider.tsx` |
| Graceful shutdown                                          | `lib.rs` `on_run_event`, SIGTERM/SIGINT handler, close-to-tray                              |
| Persisted execution metadata                               | `crates/runtime/src/store.rs` (interim JSON store)                                          |
| E2E harness                                                | `tests/e2e` + CI job                                                                        |
| Decision record                                            | [ADR-005](../adr/ADR-005-runtime-supervisor.md)                                             |

## 5. Deviations from the plan

| Deviation                                                                  | Why                                                              | Recorded             |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------- | -------------------- |
| Metadata persisted in a JSON file, not SQLite                              | Ledger schema/migrations belong to Phase 2                       | ADR-005 §7           |
| Only diagnostic profiles; no user-defined profiles                         | Keeps Phase 1 from granting any real program execution           | ADR-005 §9           |
| Cancellation is a hard tree kill (no graceful terminate first)             | Sufficient for diagnostics; revisit for real runtimes in Phase 3 | ADR-005 consequences |
| E2E runs on Linux only; Windows UI path covered by smoke test + Rust tests | tauri-driver on Windows needs a matching msedgedriver; deferred  | §6 R2                |

## 6. Residual risks, manual checks, owner items

| ID  | Item                                                                                                                                  | Recommendation                                                                                                                                                              |
| --- | ------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R1  | On Linux/macOS a **hard crash or SIGKILL** of Plenipo can leave children running (Windows is protected by the Job Object and tested). | Accept for now (Windows is the target); close in Phase 13 (daemon) or with `PR_SET_PDEATHSIG`.                                                                              |
| R2  | No automated UI test on Windows.                                                                                                      | Add Windows tauri-driver/msedgedriver job when convenient; meanwhile M1–M3.                                                                                                 |
| M1  | Close-to-tray and tray menu are not automated (WebDriver cannot drive the tray).                                                      | On Windows: start "Long-running process", close the window → it hides and the process keeps running; tray → Show; tray → Stop all; start again, tray → Quit → process ends. |
| M2  | Install the CI installer artifact on Windows 11 and run the three diagnostic profiles.                                                | ~5 minutes.                                                                                                                                                                 |
| M3  | Confirm no console window flashes when a profile starts (CREATE_NO_WINDOW).                                                           | Part of M2.                                                                                                                                                                 |
| O1  | Bundle identifier `com.eightwestventures.plenipo` still unconfirmed (from Phase 0). Runtime history now lives under it.               | Confirm before Phase 2 creates the Ledger.                                                                                                                                  |
| O2  | ADR-004 and ADR-005 are **Proposed**.                                                                                                 | Accept or amend.                                                                                                                                                            |
| O3  | Versioning convention bumps the minor version on phase acceptance.                                                                    | On acceptance, bump to `0.2.0`.                                                                                                                                             |

## 7. Owner sign-off (2026-09-26)

| Item                                                                                                        | Outcome                                                                                                       |
| ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| M1–M3 manual checks on Windows 11 (installer, close-to-tray, tray Show / Stop all / Quit, no console flash) | **Passed** — reported by owner                                                                                |
| O1 bundle identifier                                                                                        | Set to **`com.eightwest.plenipo`** by owner                                                                   |
| O3 version                                                                                                  | Bumped to **`0.2.0`**; owner set the convention of one minor bump per phase until the MVP (Phase 8) = `1.0.0` |
| O2 ADR-004 / ADR-005                                                                                        | Awaiting owner decision                                                                                       |

Phase 1 is **accepted**.

## 8. Phase boundary

Phase 1 is complete. Phase 2 (Ledger: SQLite, migrations, task/event schema) does not expand
privileges or add external integrations, so under ROLLOUT_PLAN §8.12 it does not require a
mandatory stop — but it will create the long-lived database under the bundle identifier (O1),
so owner confirmation of O1 is recommended before starting it.
