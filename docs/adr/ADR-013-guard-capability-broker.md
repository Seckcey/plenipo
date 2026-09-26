# ADR-013: Plenipo Guard, the capability broker, and human approval

- **Status:** Proposed
- **Date:** 2026-09-26
- **Phase:** 7

> **On screen** (ADR-010, plain words and rank names): capabilities are "permissions", a
> capability profile is a "permission set", a runtime grant is a worker's "permissions in use",
> and deny rules are "never run" and "never open". This ADR keeps the plan's words, which are
> also the code's.

## Context

Phase 7 lets agents use the local computer while making authority explicit, scoped, logged, and
revocable. The plan asks for a capability registry, capability profiles, per-role and
per-project permissions, runtime grants, an approval queue, deny rules, command and event
logging, Windows credential integration, and a secret-reference model. Every request is
evaluated against role policy, project policy, department policy, the target resource, the
action's risk class, and explicit user approval rules. A listed set of sensitive actions needs
approval by default. Credentials go through protected storage and are not shown to prompts. The
acceptance criteria are:

- a Development worker reads and writes only its authorized workspace and runs approved
  development commands;
- an unauthorized request is blocked and visible;
- a sensitive request pauses, shows a clear approval card, and proceeds only after approval.

The plan rules out blanket administrator access, silent elevation, plaintext secrets in SQLite,
and full enterprise RBAC.

Earlier decisions constrain the design:

- Workers run the official CLIs non-interactively, one supervised process per step, with no
  built-in tools (Claude Code) or a read-only sandbox (Codex). ADR-007 §5 anticipated that
  "Phase 7 replaces this posture with brokered, per-task capabilities".
- Every program runs through the supervisor from an allowlist, with a cleared environment
  (ADR-005).
- The Ledger is the system of record (ADR-006).
- Organization work carries the worker's position and project (ADR-009), and handoff requests
  for capabilities were recorded but never granted (ADR-008).

## Decision

1. **Two new crates, as ADR-004 planned.**
   - `crates/guard` (`plenipo-guard`) decides. It holds the registry, permission sets, the pure
     evaluation engine, folder confinement, command rules, the sensitive-action check, secret
     redaction, and its settings document.
   - `crates/capabilities` (`plenipo-capabilities`) carries out. It holds the broker, Plenipo's
     tool server and relay, the file, program, and git executors, the approval waits, runtime
     grants, and the Vault.
   - Plenipo Vault is a module of the broker rather than a third crate: it has one consumer.
2. **Plenipo's own tools, not the AI tools' built-in ones.** Workers keep the Phase 3 posture:
   Claude Code has `--tools ""` and `--strict-mcp-config`, and Codex keeps its read-only
   sandbox. A worker with permissions gets Plenipo's tools through the one add-on channel both
   CLIs support: an MCP server over stdio.
   - The tools are list, read, search, write, edit, move, and delete files; run a program; run
     a PowerShell script; and git status, diff, log, add, commit, branch, and push.
   - Claude Code gets `--mcp-config <file> --allowedTools mcp__plenipo` (no prompt from Claude
     Code itself) and `MCP_TOOL_TIMEOUT`, so a call can wait for an approval.
   - Codex gets `-c mcp_servers.plenipo.{command,args,startup_timeout_sec,tool_timeout_sec}`.
   - Every call is decided in Plenipo's code, so enforcement never depends on an AI tool's own
     permission system.
3. **The relay.** The AI tool starts Plenipo's own executable with `--plenipo-tools=<ticket
file>`; `main()` handles this before Tauri starts, like the diagnostic mode.
   - The relay reads its ticket (port and key), connects to the running Plenipo on
     `127.0.0.1`, presents the key, and then only passes lines through. It is standard
     library only.
   - The tool server admits a connection only with the key of an open grant: 244 random bits,
     valid while that step runs.
   - Ticket and configuration files live in Plenipo's private data folder: owner-only on Unix,
     the user profile on Windows. They are deleted when the step ends, and cleared at startup.
4. **Runtime grants.** The agent runtime gained a `ToolProvider` hook.
   - For each step of an organization worker's task, the broker opens a grant. The grant holds
     the worker's scope (role; the project its work belongs to; the department), a snapshot of
     its levels, its folder, and its ticket, and is recorded as `guard.grant_opened`.
   - The grant closes when the step's program ends, before the step's result is recorded
     (`guard.grant_closed`, with counts). Closing stops the grant's running programs and
     expires its pending approvals, so a paused task is never left waiting on a worker that is
     gone.
   - Workers the owner starts in Workers, and roles with nothing permitted, get no grant: they
     stay conversation only.
   - A worker with permissions but no usable folder gets no tools, and `guard.grant_skipped`
     says why.
5. **Levels and layers.** Each capability has a level: **allowed**, **ask** (ask me), or
   **blocked**. The effective level is the strictest of these:
   - the role's set (it grants; a role without a set has nothing);
   - the project's limit (the project's existing permission-set field; no limit by default; an
     unknown set fails closed, and Settings says so);
   - the department's limit.

   Each call is then checked against the target, the risk, and the rules, in the plan's order:
   - inside the folder;
   - not a blocked file;
   - not a `.git` internal for writes;
   - not a blocked command;
   - the sensitive-action check;
   - the always-ask list;
   - an Ask level;
   - for programs, an approved command (anything else asks).

   Each decision carries one plain sentence and every layer's note. It uses the snapshot and
   the settings as they are now, whichever is stricter: removing a permission applies to the
   next call, and a grant never widens.

6. **Folder confinement.**
   - A path resolves inside the project folder or is refused. Refused: `..` that climbs out,
     absolute paths elsewhere, symbolic links that lead out (checked on the canonical deepest
     existing part), and device paths. Also refused: Windows device names, and colons,
     wildcards, and trailing dots or spaces in names.
   - Blocked-file patterns are gitignore-style, including `!` exceptions, and match the
     canonical path.
7. **Commands.** A worker names a program and a list of arguments, never a shell string.
   - A bare name comes from PATH; on Windows the extensions Windows runs directly are tried,
     never `.ps1`. A `./path` must be a file inside the folder.
   - Rules match word by word, case-insensitively, with `*` as a wildcard.
   - The defaults:
     - approved: common build, test, and lint commands;
     - never run: deleting, formatting, downloading, remote shells, privilege tools, and the
       shells themselves (`cmd`, `powershell`, `bash`…), so no approved rule can be bypassed
       through a shell;
     - always ask: empty.
   - Programs, PowerShell scripts (read from stdin), and git (fixed options, never prompting
     for a password) run through the supervisor: their own process tree, cleared environment,
     time limit, and a recorded run (`capability.program`).
   - A short list of development variables that are not credentials is passed on, such as
     `CARGO_HOME` and `JAVA_HOME`.
8. **Sensitive actions** (the plan's list) are recognized in command lines and scripts:
   - production changes;
   - DNS changes;
   - credential changes;
   - destructive database operations;
   - cloud resource deletion;
   - payments;
   - outbound messages and publishing (including `git push`);
   - running as administrator;
   - destructive changes outside the workspace.

   Each asks by default, and the owner may set a kind to **blocked** but never to allowed
   without asking. The check errs on the side of asking.

9. **Approvals.** "Ask" pauses the tool call. In one transaction the Ledger records the approval
   (card data, free of secrets) and moves the task to `awaitingApproval`; the task returns to
   `running` when its last pending approval is settled.
   - The owner answers on the Approvals page. The page has a sidebar count and a banner on
     every page.
   - An unanswered request expires after the approval window (10 minutes by default, 1–60),
     and approvals left pending when Plenipo stopped are expired at startup.
   - The worker is told the outcome and asked not to work around a refusal.
   - Each approval is for that one action. There is no "remember this".
10. **Revocation.** Revoke marks a grant revoked: its running programs stop, its pending
    approvals are refused, and its later calls are blocked. A settings change narrows the next
    call the same way.
11. **Logging and redaction.**
    - Every call is recorded: `capability.used`, `guard.denied` with its reason and layer, or
      `approval.*`.
    - The recorded text is redacted, and so is every result returned to a worker.
    - An installed filter hides secrets in all AI tool activity and results before they are
      shown, recorded, or passed to Liaison.
    - Redaction covers the Vault's values, private keys, common API-key and token formats,
      passwords in URLs, and upper-case `…PASSWORD=`/`…TOKEN=` settings.
    - Content that contains a hidden-secret marker cannot be written back, so a redacted read
      never destroys the real value.
12. **Vault (the secret-reference model and Windows credential integration).**
    - A secret's value goes to Windows Credential Manager, the macOS Keychain, or the Linux
      kernel keyring (the `keyring` crate). Guard's settings keep only a reference: the name,
      the environment variable, and the programs that get it.
    - The value goes in through one command and never comes back out.
    - A secret is injected only into the programs the owner named, and hidden everywhere else.
13. **Configuration** is owner data in the Ledger's `guard` setting.
    - Every change is recorded as a `guard.*` or `vault.*` event.
    - Built-in permission sets: Read only, Developer, Reviewer, Tester, Writer, Researcher, and
      No access. Each built-in role template gets its starting set once.
    - Guard's input types reject unknown fields.
    - No schema migration was needed: the Phase 2 `approvals` table and task state machine
      already had what approvals need.
14. **Registered now, tools later.** github.\*, ssh.connect, browser.\*, computer.\*, mcp.invoke,
    network.local, and process.manage are in the registry, sets, and settings. Settings says
    which phase brings their tools.

## Consequences

- Authority is explicit and enforced in one place, Plenipo's code, for both AI tools, and every
  use is auditable in the trail.
- **Codex can still read outside the folder.** Codex's own read-only commands can read any file
  the owner's account can read, as since Phase 3. Its writes, programs, network, and git go
  through Guard (its sandbox blocks the rest). Claude Code workers are fully confined.
  Closing this needs Codex's shell turned off (a configuration key this phase does not assume)
  or its app-server with approval callbacks. The owner should confirm on their CLI version.
- **An approved program is trusted to behave.** Guard confines what a worker asks for, not what
  an approved program then does: `npm run build` runs the project's own scripts. Operating-system
  sandboxing of those programs (Windows AppContainer, job restrictions) is future work, so keep
  the approved list to commands you would run yourself.
- **The sensitive-action check is heuristic.** It catches common forms and errs toward asking;
  the approved list is the real gate.
- **Timing gaps.**
  - There is a narrow window between resolving a path and using it (a race only a program the
    worker is allowed to run could exploit).
  - Streamed text shown live can show part of a secret split across chunks before the complete
    message is filtered; what is recorded is always filtered.
- **Tool timeouts depend on the AI tools** honoring `MCP_TOOL_TIMEOUT` and `tool_timeout_sec`.
  A shorter one fails the call, never the approval. The turn's own 30-minute limit still
  applies.
- On Linux the kernel keyring does not survive a reboot. Linux is not a target platform.
- Adding a tool means adding one definition and one executor. Adding a capability's tools in a
  later phase needs no change to Guard's model.

## Alternatives considered

- **The AI tools' own permission systems** (Claude Code `--allowedTools` patterns and
  `--permission-prompt-tool`; Codex `workspace-write`). Rejected: enforcement would differ per
  tool and depend on vendor behavior, Codex has no approval hook in `exec`, and Codex's own
  writes would bypass Plenipo's log.
- **HTTP MCP in the main process** (no relay). One hop fewer, but support for remote MCP servers
  differs between the CLIs and versions; stdio is supported by both.
- **A separate relay binary.** One more file to ship, sign, and allowlist. The desktop
  executable already handles a special mode (diagnostics) before anything starts.
- **Codex app-server with approval callbacks** (ADR-007's revisit note). It gives richer
  in-turn control, but it is a long-lived protocol that would also have to replace the working
  `exec` path. Revisit if Codex's reads must be confined.
- **Suspending the turn on an approval**, as with handoff replies, instead of waiting inside
  the call. It is more robust to short tool timeouts, but it relies on the model ending its
  answer when told, and it complicates the handoff wait.
- **Secrets encrypted in SQLite.** The plan rules out plaintext secrets in SQLite; the
  operating system's store is the standard, audited place, and keeps keys out of Plenipo's own
  files.
- **"Allow without asking" for sensitive kinds.** Left out on purpose: the plan says they need
  approval by default, and the only relaxation offered is making them stricter.
