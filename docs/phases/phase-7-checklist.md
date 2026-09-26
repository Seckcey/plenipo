# Phase 7 — Implementation Checklist

**Status:** in progress on `claude/phase-7`.

Source: `ROLLOUT_PLAN.md`, Phase 7 — Capability Broker, Guard, and Human Approval. Phase 6 is
implemented and merged ([PR #9](https://github.com/Seckcey/plenipo/pull/9)); the owner asked to
begin Phase 7 on 2026-09-26. This checklist keeps the plan's words where it quotes the plan; the
app uses the plain words in [`docs/design/vocabulary.md`](../design/vocabulary.md) ("permissions",
not "capabilities").

**Goal:** allow agents to use the local computer while making authority explicit, scoped,
logged, and revocable.

## Design decisions (details in ADR-013, how Plenipo lets workers use your computer safely)

- **Two new crates, as planned by ADR-004.** `crates/guard` (`plenipo-guard`): the capability
  registry, permission sets, the evaluation engine (a pure function), folder confinement, command
  rules, the sensitive-action check, and secret redaction. `crates/capabilities`
  (`plenipo-capabilities`): the capability broker — Plenipo's own tools for workers, the
  approval queue, runtime grants, and the Vault (secrets in the operating system's protected
  storage).
- **Plenipo's own tools, not the AI tools' built-in ones.** Workers still have no built-in tools
  (Claude Code `--tools ""`; Codex read-only sandbox). A worker with permissions gets Plenipo's
  tools through the standard add-on channel both AI tools support (an MCP server over stdio):
  list, read, search, write, edit, move, and delete files; run a program; run a PowerShell
  script; git status, diff, log, add, commit, branch, and push. Every call is checked by Guard in
  Plenipo's own code, carried out by Plenipo, logged in the Ledger, and redacted.
- **How the AI tool reaches Plenipo.** The AI tool starts Plenipo's own program with
  `--plenipo-tools=<ticket file>`; that small relay connects to the running Plenipo over the
  local loopback address and passes messages through. The ticket holds a random 244-bit key
  that is valid only while that step of that task runs.
- **Permission levels:** Allowed, Ask me, Blocked.
- **Layers (the plan's order):** the role's permission set grants; the project's and the
  department's permission sets can only narrow it ("no limit" by default); the target (inside
  the project folder, not a blocked file, not a blocked command); the action's risk (the
  sensitive-action check); your explicit rules (approved, always-ask, and blocked commands;
  blocked files). The strictest answer wins. A position outside any project, or a project
  without a folder, has no folder to work in, so file, program, and git tools are not offered.
- **Runtime grants.** Each step of an organization worker's task gets a grant: a snapshot of its
  permissions, its folder, and its ticket, recorded (`guard.grant_opened`) and closed when the
  step's program ends (`guard.grant_closed`). Every call is checked against the grant **and**
  the settings as they are now, so taking a permission away in Settings applies to the next
  call. **Revoke** ends a grant at once: its running commands stop, its pending approvals are
  refused, and later calls are blocked.
- **Approvals.** "Ask me" pauses the tool call: an approval record (Ledger `approvals`), the
  task shows **Waiting for your approval**, and an approval card appears (Approvals page, with a
  count in the sidebar and a banner on every page). Approve → Plenipo carries out that one
  action; Deny or no answer within the approval window (10 minutes by default) → the worker is
  told it was not approved. Approvals left pending when Plenipo stopped are marked expired.
- **Sensitive actions ask by default** (plan list): production deployment, DNS change,
  credential change, destructive database operation, cloud resource deletion, payments,
  outbound messages and publishing (including `git push`), running as administrator, and
  destructive changes outside the workspace. Each can be set to Ask me or Blocked, never to
  Allowed. Detection is a safety net over command lines and scripts; the approved-command list
  and "unknown commands ask" are the main control.
- **Folder confinement.** Paths resolve inside the project folder: `..` escapes, absolute paths
  elsewhere, symbolic links that lead out, Windows device and alternate-stream names are
  refused; `.git` internals cannot be written (git goes through the git tools). Blocked files
  (default: `.env` files, private keys, certificates, credential files) cannot be read or
  written.
- **Commands** run through the supervisor (own process tree, cleared environment, time limit,
  recorded as a run) with a program name and an argument list — never a shell string.
  Approved commands (default: common build, test, and lint commands) run without asking when
  the permission is Allowed; anything not on the list asks; blocked commands (default: deleting,
  formatting, downloading, remote shells, privilege tools) never run.
- **Secret redaction.** Known secrets from the Vault and recognizable secrets (private keys,
  common API-key and token formats, `PASSWORD=`-style lines) are hidden in tool results, in
  everything recorded in the Ledger, and in AI tool activity. Content with a hidden-secret marker
  cannot be written back.
- **Vault (secret-reference model, Windows credential integration).** A secret's value goes to
  Windows Credential Manager (Keychain on macOS, the kernel keyring on Linux); the Ledger keeps
  only its name and where it may be used. A secret can be given to named programs as an
  environment variable (for example a GitHub token as `GH_TOKEN` for `gh`); the worker never
  sees the value.
- **Configuration** is owner data in the Ledger (the `guard` setting, changed in one transaction
  per edit, each change recorded as a `guard.*` event). Project limits use the project's existing
  permission-set field. No schema migration.
- **Starting permissions** (from the role templates): Supervisor — Read only; Senior Developer —
  Developer; Code Reviewer and Security Auditor — Reviewer; QA Engineer — Tester; Documentation
  Writer and Designer — Writer; Researcher — Researcher (browsing arrives in Phase 10). Given
  once per template role; you can change them.
- **Registered now, tools later.** github.\*, ssh.connect, browser.\*, computer.\*, mcp.invoke,
  network.local, and process.manage are in the registry, permission sets, and settings; their
  tools arrive with the phases that need them (8, 10, 11).

## Deliverables

- [ ] Capability registry (the plan's 16 capabilities, with plain names and risk)
- [ ] Capability profiles (permission sets)
- [ ] Per-role permissions
- [ ] Per-project permissions (folder and limit)
- [ ] Per-department permissions (limit)
- [ ] Runtime grants (per step, recorded, revocable)
- [ ] Approval queue and approval cards
- [ ] Deny rules (blocked commands and files; sensitive actions can be blocked)
- [ ] Command/event logging (every call recorded, redacted)
- [ ] Windows credential integration (Vault)
- [ ] Secret-reference model
- [ ] Plenipo's tools for workers (files, programs, git) through Guard
- [ ] Settings → Permissions, the Approvals page, blocked requests visible
- [ ] ADR-013; architecture, README, vocabulary, setup updated

## Phase 7 tests (from plan)

- [ ] Allowed read
- [ ] Denied write
- [ ] Approval-required action
- [ ] Approval accepted
- [ ] Approval rejected
- [ ] Expired approval
- [ ] Path traversal attempt
- [ ] Command allow/deny behavior
- [ ] Secret redaction
- [ ] Capability revocation during execution

## Acceptance criteria (from plan)

- [ ] A Development worker can read/write only its authorized workspace and run approved
      development commands.
- [ ] An unauthorized request is blocked and visible.
- [ ] A sensitive request pauses, presents a clear approval card, and proceeds only after
      approval.

## Out of scope

Blanket unrestricted administrator access, silent elevation, storing plaintext secrets in
SQLite, full enterprise RBAC (plan). Also: GitHub, browser, computer-use, SSH, and MCP-server
tools (Phases 8, 10, 11), operating-system sandboxing of the programs an approved command starts,
isolated worktrees for concurrent workers (Phase 8).
