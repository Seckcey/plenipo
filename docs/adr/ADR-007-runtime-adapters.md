# ADR-007: Provider runtime adapters (Codex, Claude Code)

- **Status:** Accepted (owner, 2026-09-26)
- **Date:** 2026-09-26
- **Phase:** 3

## Context

Phase 3 runs real OpenAI Codex and Anthropic Claude Code workers under Plenipo, without their
desktop apps. It is the first phase that starts third-party AI tools on the owner's machine,
with the owner's sign-ins. The plan requires a provider-neutral `RuntimeAdapter` contract,
detection of installation and sign-in, session creation and resume, streamed output,
cancellation, normalized results, and executions preserved in the Ledger. It forbids collecting
provider passwords and silently converting subscription use into API-billed use (ADR-003, plan
§1.4). ADR-005 limits launches to approved profiles; ADR-006 makes the Ledger the system of
record.

## Decision

1. **Surface: the official non-interactive CLIs, one supervised process per turn.**
   - Claude Code: `claude -p --output-format stream-json --verbose --include-partial-messages`.
     A new session is started with `--session-id <UUID chosen by Plenipo>`; a follow-up uses
     `--resume <provider session ID>`.
   - Codex: `codex exec --json --sandbox read-only --skip-git-repo-check`; a follow-up appends
     `resume <thread ID>`. This is the invocation OpenAI's own Codex SDK uses.
   - The objective is written to the child's **stdin**, never placed in argv.
   - Each turn is an ordinary supervised execution (ADR-005): own process tree, cleared
     environment, timeout (30 minutes), cancel, shutdown handling, persisted metadata.
2. **Contract.** `RuntimeAdapter` (in `crates/runtime`) is provider-neutral. Plan verbs map to:
   detect installation → executable discovery + version probe; detect authentication → sign-in
   probe + classification; list capabilities → static `RuntimeCapabilities`; start session,
   resume session, submit task → one `TurnRequest` (new or existing provider session, objective,
   optional model) turned into launch arguments; stream events → a per-turn parser producing
   normalized `AgentEvent`s; cancel execution → the supervisor's tree kill (common to all
   adapters); close session → Plenipo marks the session closed (provider transcripts stay in
   the provider's own store); normalize result → the parser's final `TurnResult`. Provider
   names occur only in the two adapter modules and in data values.
3. **Launch path (extends ADR-005).** The UI supplies only: a runtime ID from a fixed set, an
   objective (1–10,000 characters, sent on stdin), an optional model name matching
   `[A-Za-z0-9][A-Za-z0-9._:\[\]-]{0,63}`, and a Plenipo session ID (UUID). Core builds every
   argument. Executables are found only by detection rules (PATH and well-known install
   locations for the exact names `claude` / `codex`), added to the allowlist by Core, and
   re-checked at spawn. If a CLI updates itself (symlink retargeted), the next turn re-resolves
   it by the same rules. On Windows only real `.exe` files are accepted: npm `.cmd` shims run
   through `cmd.exe`, whose argument parsing is unsafe for general input; for Codex the native
   binary vendored inside the npm package is used (as the Codex SDK does), and for Claude Code
   the owner is guided to the native installer.
4. **Credentials and billing.** Plenipo never asks for, reads, or stores provider credentials,
   and never stores account identifiers (email, organization) from sign-in probes — only the
   method class. Children receive the OS baseline plus a short per-adapter pass-through list
   (`CLAUDE_CONFIG_DIR`, `CLAUDE_CODE_GIT_BASH_PATH`, `CODEX_HOME`, proxy and CA variables).
   API-key and cloud-provider variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
   `CODEX_API_KEY`, `CLAUDE_CODE_USE_BEDROCK`, …) are never passed. Before every turn Plenipo runs
   the CLI's own status command (`claude auth status`, `codex login status`) with the same
   environment the turn will get:
   - signed out → the turn is refused with the official login command to run;
   - signed in with an API key or a third-party cloud → refused as `billingNotAllowed`
     (API fallback is disabled until explicitly configured in a later phase);
   - subscription sign-in → allowed;
   - signed in but unrecognized, or status unavailable → allowed **only** for a runtime that
     proves its credential during the turn (Claude Code), labelled "billing unverified";
     otherwise refused (Codex must show a ChatGPT sign-in).
     Claude Code reports its credential source at the start of every stream; any source other
     than a subscription sign-in terminates the turn immediately as `billingNotAllowed`. If the
     sign-in was not confirmed up front, a stream that does not report its source (or produces
     output before reporting it) is stopped the same way.
5. **Least privilege until Guard (Phase 7).** No capabilities are granted in Phase 3. Claude
   Code runs with no built-in tools (`--tools ""`) and no MCP servers (`--strict-mcp-config`);
   Codex runs in its read-only sandbox (no writes, no network) with approvals disabled. Each
   session works in its own empty directory, `<local app data>/runtime/agent-workspaces/<id>`.
   The auto-updater is disabled for Plenipo-launched Claude Code processes so a binary is not
   replaced mid-turn.
6. **Sessions and Ledger.** Migration 0002 adds `runtime_sessions` (Plenipo session ID, runtime,
   provider, provider session ID, model, working directory, `open`/`closed`). A turn is a
   Ledger **task** (objective = the prompt; metadata carries the session), plus an
   **execution** carrying runtime, provider, model, session, and usage, plus normalized
   activity events (`agent.message`, `agent.tool_use`, `agent.notice`, `agent.session_bound`)
   and one final `agent.result` event holding the normalized result. Text in events is capped
   (4 KiB per activity event, 64 KiB for the result). Raw provider output stays in the
   supervisor's in-memory buffer for diagnostics and is not stored, as in ADR-005/006. Turns
   left running when Plenipo stopped become `interrupted`.
7. **Normalized outcomes.** `completed | failed | cancelled | timedOut | usageLimited |
authRequired | billingNotAllowed | providerUnavailable | malformedOutput | crashed |
interrupted`. `completed` → task `succeeded`, `cancelled` → `cancelled`, everything else →
   `failed` with the reason. A usage limit never switches provider; the session stays open and
   can be resumed later. Unknown stream events are ignored (and counted), never fatal; a turn
   that ends without a parseable result is `malformedOutput` or `crashed`.
8. **Resume semantics.** A provider session ID counts as confirmed once the provider reports it
   (Claude Code `init`, Codex `thread.started`). Resume uses the confirmed ID and the session's
   fixed working directory (Claude Code finds sessions per directory). If the first turn failed
   before the provider confirmed a session, the next turn starts the provider session again and
   the Ledger records it. One active turn per session; at most four active turns overall.

## Consequences

- Real-CLI behavior (flags, sign-in status output, stream fields) can change between CLI
  releases. Adapters parse defensively, record the detected version, and are tested against
  fake CLIs that speak the documented formats. Owner verification with real CLIs is part of
  Phase 3 acceptance; a format change is fixed in one adapter module.
- Agents can do little in Phase 3 by design: Claude Code can only converse; Codex can run
  read-only commands inside its own sandbox (which can read files the owner's account can
  read). Phase 7 (Guard) replaces this posture with brokered, per-task capabilities.
- Cancellation is a hard tree kill; the provider's transcript up to that point remains and the
  session can be resumed. A cooperative "interrupt, then kill" can be added if a provider needs
  it.
- Activity text may contain whatever the agent wrote. Secret redaction arrives in Phase 7;
  Phase 3 limits exposure by granting no tools beyond Codex's read-only sandbox.
- Owners who installed Claude Code only through npm on Windows must install the native build.

## Alternatives considered

- **Codex app-server (JSON-RPC over stdio)** — richer (in-turn approvals, live thread
  management), but a long-lived bidirectional protocol that Phase 3 does not need. Revisit in
  Phase 7, when Guard must answer approval requests mid-turn.
- **Provider HTTP APIs** — would bill API usage instead of the owner's subscriptions, which the
  plan forbids without explicit configuration.
- **Automating the desktop apps** — explicitly rejected by the plan; brittle.
- **Prompt as a command-line argument** — exposes the prompt in process listings, is limited in
  length on Windows, and invites quoting bugs. Stdin avoids all three.
- **Reusing `agent_instances` for sessions** — an agent instance is an organizational worker
  filling a role (Phase 5); a runtime session is provider mechanics. Keeping them separate lets
  an agent instance use several sessions over time (for example after a provider change).
