# Phase 1 — Implementation Checklist

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

- [ ] `crates/runtime`: supervisor, execution IDs, lifecycle state machine, executable and
      environment policy, output capture (bounded), cancellation, timeouts, metadata store
- [ ] Diagnostic child scenarios (echo, stderr, failure exit, long-running, env dump,
      process tree) + `plenipo-diag` test binary
- [ ] Typed commands: overview, start, cancel, output — each granted in capabilities
- [ ] Event streaming Rust → React (batched output + lifecycle events)
- [ ] Main desktop shell with left navigation: Organization (placeholder, data-driven),
      Runtimes, Activity, Settings, Diagnostics
- [ ] Process status screen: profiles, executions, live output, cancel
- [ ] System tray: show window, active count, stop all, quit
- [ ] Graceful shutdown: quit terminates owned processes and persists final state;
      closing the window while work is active hides to tray instead of killing it
- [ ] ADR-005 (runtime supervisor boundary); architecture doc update
- [ ] E2E harness (tauri-driver) exercising the real UI → Core → process → UI path

## Phase 1 tests (from plan)

- [ ] Launch a harmless local test process
- [ ] Receive incremental stdout
- [ ] Receive stderr
- [ ] Cancel a long-running test process
- [ ] Detect normal and abnormal exits
- [ ] Restart the UI without orphaning owned processes
- [ ] Reject executable paths outside configured rules

Additional failure paths: timeout, spawn failure, invalid working directory, unknown profile,
duplicate cancel, env isolation, process-tree kill, invalid state transitions, oversized
output lines, corrupt metadata file, crash of the owning process (Windows).

## Acceptance criteria (from plan)

- [ ] Plenipo can launch, observe, and terminate a local child process
- [ ] Live process activity is visible in the UI
- [ ] Process state survives expected UI transitions
- [ ] The frontend cannot directly execute arbitrary OS commands

## Out of scope

Codex, Claude Code, shell capability granted to agents, model routing, autonomous
delegation, user-defined launch profiles, SQLite Ledger (Phase 2).
