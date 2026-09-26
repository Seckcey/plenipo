# Phase 1 — Implementation Checklist

**Status:** complete — see [phase-1-acceptance-report.md](phase-1-acceptance-report.md).

Source: `ROLLOUT_PLAN.md`, Phase 1 — Desktop Shell and Local Runtime Supervisor.
Phase 0 accepted (see `phase-0-acceptance-report.md`); owner approved starting Phase 1.

**Goal:** prove Plenipo can safely supervise local child processes and stream their events to
the desktop UI.

## Design decisions (details in ADR-005)

- The UI never names an executable, arguments, environment, or working directory. It asks
  Core to start a **launch profile by ID**. Profiles are defined in Rust.
- Every profile's executable is checked against an **executable allowlist** (canonical,
  absolute paths) at registration **and** again at spawn.
- Children get a **cleared environment**: a small OS baseline (PATH, SystemRoot, TEMP, …)
  plus only the variables the profile declares. Parent secrets never leak.
- Children run in a **Windows Job Object with kill-on-close** (Unix: own process group), so
  cancel kills the whole tree and a Plenipo crash on Windows cannot orphan them.
- Phase 1 ships only **built-in diagnostic profiles**, which run Plenipo's own executable in a
  harmless diagnostic mode (`--plenipo-diagnostic=<scenario>`). No arbitrary programs.
- Execution metadata persists to a JSON file (atomic writes) until the Ledger (Phase 2)
  replaces it. Records left "running" by a previous session become `interrupted` on startup.

## Deliverables

- [x] `crates/runtime`: supervisor, execution IDs, lifecycle state machine, executable and
      environment policy, output capture (bounded), cancellation, timeouts, metadata store
- [x] Diagnostic child scenarios (echo, stderr, failure exit, long-running, env dump,
      process tree) + `plenipo-diag` test binary
- [x] Typed commands: overview, start, cancel, output — each granted in capabilities
- [x] Event streaming Rust → React (batched output + lifecycle events)
- [x] Main desktop shell with left navigation: Organization (placeholder, data-driven),
      Runtimes, Activity, Settings, Diagnostics
- [x] Process status screen: profiles, executions, live output, cancel
- [x] System tray: show window, active count, stop all, quit
- [x] Graceful shutdown: quit terminates owned processes and persists final state;
      closing the window while work is active hides to tray instead of killing it
- [x] ADR-005 (runtime supervisor boundary); architecture doc update
- [x] E2E harness (tauri-driver) exercising the real UI → Core → process → UI path

## Phase 1 tests (from plan)

- [x] Launch a harmless local test process
- [x] Receive incremental stdout
- [x] Receive stderr
- [x] Cancel a long-running test process
- [x] Detect normal and abnormal exits
- [x] Restart the UI without orphaning owned processes
- [x] Reject executable paths outside configured rules

Additional failure paths: timeout, spawn failure, invalid working directory, unknown profile,
duplicate cancel, env isolation, process-tree kill, invalid state transitions, oversized
output lines, corrupt metadata file, crash of the owning process (Windows).

## Acceptance criteria (from plan)

- [x] Plenipo can launch, observe, and terminate a local child process
- [x] Live process activity is visible in the UI
- [x] Process state survives expected UI transitions
- [x] The frontend cannot directly execute arbitrary OS commands

## Out of scope

Codex, Claude Code, shell capability granted to agents, model routing, autonomous
delegation, user-defined launch profiles, SQLite Ledger (Phase 2).
