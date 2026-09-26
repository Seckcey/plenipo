# ADR-005: Runtime supervisor boundary

- **Status:** Accepted (owner, 2026-09-26)
- **Date:** 2026-09-26
- **Phase:** 1

## Context

Phase 1 gives Plenipo its first privileged capability: starting local processes. Later
phases will use the same supervisor to run provider CLIs (Codex, Claude Code) and, behind
Guard, development tools. The boundary set now determines how safe those phases can be.
ROLLOUT_PLAN §3.1 requires least privilege, validated IPC input, and capabilities enforced in
code; Phase 1 requires that the frontend cannot execute arbitrary OS commands.

## Decision

1. **Launch profiles, not commands.** The UI can only ask Core to start a _profile ID_. A
   `LaunchProfile` (Rust only) fixes the executable, arguments, working directory, declared
   environment variables, and maximum runtime. No command accepts a path, argument, or
   environment value from the UI, and profile DTOs sent to the UI omit them.
2. **Executable allowlist.** `ExecutablePolicy` holds canonical absolute paths. A candidate
   must be absolute, exist, be a file, and canonicalize (symlinks and `..` resolved) to an
   allowlisted path. It is checked when a profile is registered **and** again at spawn.
3. **Environment isolation.** Children start from an empty environment plus a small OS
   baseline (`PATH`, `SystemRoot`, `TEMP`, …) and the profile's declared variables. Anything
   else in Plenipo's environment — including credentials — is withheld.
4. **Process-tree ownership.** Each launch runs in its own tree: a Windows Job Object with
   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (via `process-wrap`), or a Unix process group.
   Cancel, timeout, and shutdown terminate the whole tree. If the top-level process exits while
   descendants still hold its output pipes open, the rest of the tree is terminated too.
5. **Lifecycle.** Every launch gets a UUID v4 execution ID and a validated state machine
   (`starting → running → succeeded | failed | cancelled | timedOut`, plus `interrupted` for
   recovery). Every profile has a hard maximum runtime.
6. **Output.** stdout/stderr are read line by line with an 8 KiB per-line cap, sequenced
   across both streams, kept in a 1,000-line ring buffer, and emitted to the UI in batches
   (every 50 ms or 200 lines).
7. **Persistence (interim).** Execution metadata — never output or environment values — is
   written atomically to `<app local data>/runtime/executions.json` (last 200 records).
   Records left active by a previous session become `interrupted` at startup. A corrupt file
   is quarantined and reported. The Ledger (Phase 2) replaces this store.
8. **Shutdown.** Quit (tray), last-window close, SIGTERM and SIGINT all run one graceful
   shutdown that terminates owned processes and records them as cancelled. Closing the window
   while work is active hides it to the tray instead of killing the work.
9. **Phase 1 profiles.** Only Plenipo's own executable is allowlisted, running harmless
   built-in diagnostic scenarios (`--plenipo-diagnostic=<scenario>`, handled before Tauri
   starts). No arbitrary programs can be launched in this phase.

## Consequences

- Adding a new runnable program later means adding a reviewed profile in Rust (and, from
  Phase 7, a Guard policy) — it cannot be done from the UI or from agent output.
- Windows guarantees no orphaned children even if Plenipo crashes (tested in CI). On Linux
  and macOS the guarantee covers cancel, timeout, quit, SIGTERM, and SIGINT, but **not** a
  hard crash or SIGKILL of Plenipo itself; closing that gap needs `PR_SET_PDEATHSIG`
  (unsafe `pre_exec`) or a separate daemon (Phase 13). Windows is the primary target.
- Cancellation is a hard tree kill. A cooperative "terminate, then kill" sequence can be added
  when real runtimes (Phase 3) need it.
- Output of executions from a previous session is not retained; the UI says so.

## Alternatives considered

- **Pass commands from the UI and validate them in Rust** — a much larger attack surface;
  validation of free-form commands is error-prone and contradicts ADR-001's trust boundary.
- **Tauri shell plugin** — designed for UI-initiated commands; it would put the allowlist in
  capability JSON instead of Core, and it lacks process-tree ownership.
- **SQLite now** — Phase 2 owns the Ledger schema and migrations; a JSON file keeps Phase 1
  within scope while still being durable and atomic.
