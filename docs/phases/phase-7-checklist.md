# Phase 7 — Implementation Checklist

**Status:** implemented on `claude/phase-7` ([PR #14](https://github.com/Seckcey/plenipo/pull/14));
awaiting owner acceptance. Report: [phase-7-acceptance-report.md](phase-7-acceptance-report.md).

Source: `ROLLOUT_PLAN.md`, Phase 7 — Capability Broker, Guard, and Human Approval. Phase 6 is
accepted and released as v0.7.0 ([PR #9](https://github.com/Seckcey/plenipo/pull/9)); the owner
asked to begin Phase 7 on 2026-09-26. This checklist keeps the plan's words where it quotes the plan; the
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

- [x] Capability registry (the plan's 16 capabilities, with plain names and risk)
- [x] Capability profiles (permission sets)
- [x] Per-role permissions
- [x] Per-project permissions (folder and limit)
- [x] Per-department permissions (limit)
- [x] Runtime grants (per step, recorded, revocable)
- [x] Approval queue and approval cards
- [x] Deny rules (blocked commands and files; sensitive actions can be blocked)
- [x] Command/event logging (every call recorded, redacted)
- [x] Windows credential integration (Vault)
- [x] Secret-reference model
- [x] Plenipo's tools for workers (files, programs, git) through Guard
- [x] Settings → Permissions, the Approvals page, blocked requests visible
- [x] ADR-013; architecture, README, vocabulary, setup updated

## Phase 7 tests (from plan)

- [x] Allowed read
- [x] Denied write
- [x] Approval-required action
- [x] Approval accepted
- [x] Approval rejected
- [x] Expired approval
- [x] Path traversal attempt
- [x] Command allow/deny behavior
- [x] Secret redaction
- [x] Capability revocation during execution

## Acceptance criteria (from plan)

- [x] A Development worker can read/write only its authorized workspace and run approved
      development commands.
- [x] An unauthorized request is blocked and visible.
- [x] A sensitive request pauses, presents a clear approval card, and proceeds only after
      approval.

## Out of scope

Blanket unrestricted administrator access, silent elevation, storing plaintext secrets in
SQLite, full enterprise RBAC (plan). Also: GitHub, browser, computer-use, SSH, and MCP-server
tools (Phases 8, 10, 11), operating-system sandboxing of the programs an approved command starts,
isolated worktrees for concurrent workers (Phase 8).

## Owner check on Windows (~25 minutes)

Uses the organization from the Phase 5 and 6 checks (Development → Website, with its Senior
Developer). Use a scratch copy of a small git repository as the project folder — not real work —
for example:

```powershell
git clone https://github.com/Seckcey/plenipo.git "$env:USERPROFILE\plenipo-guard-test"
```

1. Install this version and start Plenipo. **AI tools** → **Re-check**: Claude Code and Codex
   both **Ready**.
2. **Settings** → **Permissions**. _Who may do what_: Senior Developer → **Developer**, Code
   Reviewer → **Reviewer**, Supervisor → **Read only**; the top says **Tools ready**, and
   _Secrets_ says they are kept in Windows Credential Manager. If a project is listed as a
   problem (its permission limit is not a permission set), fix it in step 3.
3. **Organization** → select _Website Supervisor_ → **Edit project** → **Project
   folder**: `%USERPROFILE%\plenipo-guard-test` (the full path) → **Permission limit**: _No
   limit_ → **Save**. The details panel shows the folder and "No limit".
4. **Settings** → **Permissions** → _Programs workers may run_ → add `git status *` to
   **Approved** → **Save command lists**.
5. Give _Website Supervisor_ the objective: _"Ask the Senior Developer to read README.md, create
   notes/guard-test.txt with one line saying hello, run git status, and then try to read the
   file outside.txt in the folder above the project."_ Expected: the file appears in the
   folder; **Approvals** → _Recently blocked_ shows "Senior Developer tried to read …
   outside.txt", and the worker says it was blocked (acceptance 1 and 2).
6. Objective: _"Ask the Senior Developer to push the current branch with git_push."_ A banner
   appears on every page ("Senior Developer is waiting for your approval — git push origin").
   **Review** → the card says exactly what will run and why it needs you. Wait two minutes
   (the AI tool must keep waiting), then **Deny**: the worker reports it was not approved and
   nothing was pushed (acceptance 3). Repeat and **Approve**: the push runs (it may fail if
   you cannot push to that repository — that is fine; it ran only after approval).
7. **Revoke**: objective _"Ask the Senior Developer to run ping -n 30 127.0.0.1."_ Approve the
   card, then under _Workers using permissions now_ choose **Revoke** → **Revoke now** while it
   runs: the ping stops, and the worker reports that its permissions were revoked.
8. Expiry: **Settings** → **Permissions** → _How long workers wait_ → **1** → **Save**. Repeat
   step 6 and do not answer: after a minute the card moves to _Recent answers_ as **Expired**. Set it
   back to 10.
9. **Vault**: **Settings** → **Permissions** → **Secrets** → **Add a secret** → Name _Test
   secret_, Value `plenipo-test-secret-8w` (a made-up value — never a real one for this test)
   → **Store secret**. Windows **Credential Manager** → **Windows Credentials** lists an entry
   for `com.eightwest.plenipo`. Then confirm Plenipo's own files do not contain it (no output
   expected):

   ```powershell
   Get-ChildItem "$env:LOCALAPPDATA\com.eightwest.plenipo\ledger\plenipo.db*" |
     Select-String -Pattern "plenipo-test-secret-8w" -SimpleMatch
   ```

   Remove the test secret afterwards.

10. **Activity** → the objective's task → the Senior Developer's task: "Permissions given to
    Senior Developer", each use, "Blocked: …", "Waiting for your approval: git push origin",
    the answer, and "Permissions ended for Senior Developer".
11. Optional, with Codex: **Edit title, AI tool, or model** on the Senior Developer → AI tool
    _Codex_, then repeat step 5.

Things only real CLIs can confirm (report anything odd):

- Claude Code offers Plenipo's tools with `--tools ""` (built-in tools off) plus
  `--mcp-config` and `--allowedTools mcp__plenipo`, never asks in the terminal, and waits for an
  approval beyond its usual tool timeout (`MCP_TOOL_TIMEOUT`).
- Codex accepts `-c mcp_servers.plenipo.*` in `codex exec`, runs Plenipo's tools without its own
  prompt, and waits for an approval (`tool_timeout_sec`).
- Plenipo's own program, started by the AI tool as the relay (`--plenipo-tools=…`), talks over
  its standard input and output on Windows (the release build has no console window).
- Codex's own read-only commands can still read files outside the folder (ADR-013,
  Consequences); Claude Code workers cannot.
