# Phase 7 — Acceptance Report

|              |                                                                                                                                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 7 — Capability Broker, Guard, and Human Approval                                                                                                                                                              |
| **Branch**   | `claude/phase-7` ([PR #14](https://github.com/Seckcey/plenipo/pull/14), merged)                                                                                                                               |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, full `pnpm e2e`. GitHub CI: Rust, Frontend, E2E (Linux), Windows — see PR #14.                                                                       |
| **Date**     | 2026-09-26                                                                                                                                                                                                    |
| **Result**   | **Accepted by the owner on 2026-09-26** and released as **v0.8.0**. All three acceptance criteria pass end to end against fake CLIs in CI; the owner's Windows check with the real CLIs is deferred (§7, O2). |

Screenshots: [Settings → Permissions: who may do what](evidence/phase-7/permissions-settings.png)
· [programs workers may run](evidence/phase-7/permissions-commands.png) ·
[a push waiting for approval: the banner](evidence/phase-7/approval-banner.png) ·
[the approval card, the worker's permissions in use, and a blocked request](evidence/phase-7/approval-card.png)
· [after approving](evidence/phase-7/approvals-answered.png) ·
[Guard's record in the activity trail](evidence/phase-7/guard-trail.png).

Test totals: **638 Rust** (Linux) · **138 frontend** · **39 end-to-end** against the real
release binary (6 Phase 1 + 6 Phase 2 + 8 Phase 3 + 5 Phase 4 + 5 Phase 5 + 5 Phase 6 + 4
Phase 7).

On screen, capabilities are "permissions", capability profiles "permission sets", runtime grants
a worker's "permissions in use", deny rules "Never run" and "files workers may never open", and
the Vault "Secrets" ([word list](../design/vocabulary.md)). Quotes from the plan below keep the
plan's words.

CI has no AI tool accounts, so every automated test drives `plenipo-fake-agent`, installed as
`claude` and `codex`. Given `<<tool:NAME {json}>>` in its objective, the fake worker calls
Plenipo's tools exactly as a real one would: it reads the MCP configuration Plenipo passed on its
command line (Claude Code's `--mcp-config` or Codex's `-c mcp_servers.plenipo.*`), starts the
relay it names (Plenipo's own program with `--plenipo-tools=…`), and speaks MCP through it. The
end-to-end test uses the real desktop executable as that relay.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                              | Result (fake CLIs) | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --- | -------------------------------------------------------------------------------------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | A Development worker can read/write only its authorized workspace and run approved development commands. | **Pass**           | Integration `acceptance_a_development_worker_works_only_in_its_workspace` (a Codex Senior Developer reads, writes, edits, and runs an approved command in its project folder; `../outside.txt`, an absolute path elsewhere, `.env`, `curl`, and `.git/config` are refused and nothing is written outside). E2E `acceptance: the worker works only in its folder…` (the real app: the file is written in the folder, `git --version` runs after being added to the approved list, nothing outside the folder). |
| 2   | An unauthorized request is blocked and visible.                                                          | **Pass**           | Every refusal is a `guard.denied` event with its reason and layer, is told to the worker, and is listed under **Approvals → Recently blocked** and in the task's trail ("Blocked: Senior Developer tried to read ../outside.txt"). Integration `plan_allowed_read_and_denied_write`; E2E acceptance and trail tests; [screenshot](evidence/phase-7/approval-card.png).                                                                                                                                        |
| 3   | A sensitive request pauses, presents a clear approval card, and proceeds only after approval.            | **Pass**           | Integration `acceptance_a_sensitive_request_waits_for_approval_even_when_allowed` (`git push` asks even though the Developer set allows saving to git; nothing ran before approval; one `capability.used` carrying the approval's ID after). E2E: banner on every page, sidebar count, a card with the worker, project, folder, exact command, reason, and time left; the push runs only after **Approve**; [banner](evidence/phase-7/approval-banner.png), [card](evidence/phase-7/approval-card.png).       |

## 2. Required Phase 7 tests → evidence

Unit tests in `crates/guard` test the engine (pure), paths, command rules, sensitive actions, and
redaction; `crates/ledger/src/guard.rs` tests approvals and task states; `crates/capabilities/tests/broker.rs`
runs the real broker, tool server, relay, Workforce, Router, Liaison, agent runtime, supervisor,
adapters, and a file-backed Ledger against the fake CLIs, with Claude Code and Codex workers.

| Test (ROLLOUT_PLAN.md)                 | Result   | Evidence                                                                                                                                                                                                                                                                                                                               |
| -------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Allowed read                           | **Pass** | `plan_allowed_read_and_denied_write` (a Read-only Supervisor reads README.md; only the tools it may use are offered); engine `allowed_read_and_denied_write`                                                                                                                                                                           |
| Denied write                           | **Pass** | Same test: "Blocked: the Read only set of the Supervisor role does not allow changing files", layer `role`, nothing written, listed as blocked                                                                                                                                                                                         |
| Approval-required action               | **Pass** | `plan_approval_required_accepted_rejected_and_expired` (a command not on the approved list: the call pauses, the task is **Waiting for your approval**, the card explains it); Ledger `an_approval_pauses_its_task_until_the_last_one_is_settled`                                                                                      |
| Approval accepted                      | **Pass** | Same test: it runs once, recorded with the approval's ID; E2E approve                                                                                                                                                                                                                                                                  |
| Approval rejected                      | **Pass** | Same test: it does not run; the worker is told and asked not to work around it                                                                                                                                                                                                                                                         |
| Expired approval                       | **Pass** | Same test (no answer within the window); `approvals_left_waiting_expire_when_plenipo_starts_again_and_tickets_are_single_use`; Ledger `expiry_and_recovery`                                                                                                                                                                            |
| Path traversal attempt                 | **Pass** | `plan_path_traversal_attempts_are_refused` (`..`, absolute paths, a symbolic link that leads out, Windows device names and alternate streams, `.git` internals); paths `traversal_is_refused`, `links_that_leave_are_refused`                                                                                                          |
| Command allow/deny behavior            | **Pass** | `plan_command_allow_and_deny_behavior` (approved runs at once; not listed asks; blocked never runs, even when installed or not; a shell is blocked; `./script` only inside the folder); engine `command_allow_ask_and_deny`; commands `rules_match_word_by_word`                                                                       |
| Secret redaction                       | **Pass** | `plan_secret_redaction` (a Vault secret and a recognizable token are hidden in tool results, in the Ledger, and in the worker's answer; the secret reaches only the program it was given to; a file with a hidden-secret marker cannot be written back); redact tests; `changes_are_validated_recorded_and_secret_values_never_stored` |
| Capability revocation during execution | **Pass** | `plan_capability_revocation_during_execution` (revoke while a program runs: it stops, its waiting approval is refused, later calls are blocked); engine `revoked_and_narrowed_grants`                                                                                                                                                  |

Also: a settings change applies to the next call (`a_settings_change_applies_to_the_next_call`),
project and department limits narrow a role and an unknown limit fails closed
(`project_and_department_limits_narrow_a_role`), a project without a folder gives no tools and
says why (`a_project_without_a_folder_gives_no_file_tools_and_says_so`), tickets are single-use
and useless after the step, the registry covers the plan's 16 capabilities once, starting
permission sets are stored and given to template roles once, both adapters pass Plenipo's tools
(`plenipo_tools_come_only_from_plenipo`, `plenipo_tools_are_passed_as_codex_settings`), IPC
boundary tests for the 13 new commands (configuration and approvals through IPC, project limits
must be permission sets, input validation, refused extra fields, denial for ungranted windows and
remote origins), and frontend tests for Settings → Permissions, the Approvals page and card, the
banner, and the trail text. All earlier phases' tests pass.

## 3. Defects found and fixed during Phase 7

| Found by          | Problem                                                                                                                           | Fix                                                                                         |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Unit tests        | Redaction hid ordinary code such as `password = input()`.                                                                         | Only upper-case settings (`DB_PASSWORD=…`), JSON secret keys, and known formats are hidden. |
| Integration tests | A blocked program that was not installed was reported as "not found" rather than blocked, so the refusal was not recorded as one. | Blocked rules are checked before the program is looked up.                                  |
| Unit tests        | A Windows program path with a drive letter (`C:\…\git.exe`) did not match its rule (`git`).                                       | Rules compare the program's file name without its extension.                                |
| Code review       | Search skipped folders named `bin`, `obj`, and `build`, which can hold real source.                                               | Only clearly generated folders are skipped (`.git`, `node_modules`, `target`, `dist`, …).   |
| E2E (real app)    | Guard's refusals on screen began with "invalid input:".                                                                           | Refusals keep their plain message (tested).                                                 |
| Screenshot review | The approval card's title was in capitals, like a section heading.                                                                | Normal case, full contrast.                                                                 |

## 4. Deliverables

| Deliverable (plan)             | Location                                                                                                                                                                                                                                                                                                           |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Capability registry            | `crates/guard/src/registry.rs` (16 capabilities, plain names, which have tools now and which phase brings the rest)                                                                                                                                                                                                |
| Capability profiles            | Permission sets: `crates/guard/src/dto.rs`, `config.rs`; 7 built in (`defaults.rs`); Settings → Permissions → _Permission sets_                                                                                                                                                                                    |
| Per-role permissions           | Each role's set (it grants); template roles get their starting set once                                                                                                                                                                                                                                            |
| Per-project permissions        | The project's folder (its workspace) and permission limit (it narrows); unknown limits fail closed                                                                                                                                                                                                                 |
| Per-department permissions     | A department's limit (it narrows)                                                                                                                                                                                                                                                                                  |
| Runtime grants                 | `crates/capabilities/src/broker.rs` (per step; `guard.grant_opened` / `guard.grant_closed`; revocable); Approvals → _Workers using permissions now_                                                                                                                                                                |
| Approval queue                 | Ledger `approvals` (Phase 2 table), `crates/ledger/src/guard.rs`; Approvals page, card, banner, sidebar count                                                                                                                                                                                                      |
| Deny rules                     | Never-run commands, files workers may never open, sensitive actions set to Blocked                                                                                                                                                                                                                                 |
| Command/event logging          | `capability.used`, `capability.program` runs, `guard.denied`, `approval.*`, `guard.*`, `vault.*` events, all redacted                                                                                                                                                                                              |
| Windows credential integration | `crates/capabilities/src/vault.rs` (`keyring`, Windows Credential Manager; Keychain and kernel keyring elsewhere)                                                                                                                                                                                                  |
| Secret-reference model         | Guard settings keep a secret's name, variable, and programs; the value goes in once and never back out                                                                                                                                                                                                             |
| Plenipo's tools                | `crates/capabilities/src/{tools,files,programs,server,relay,mcp}.rs`; the runtime's `ToolProvider` hook; adapters pass the MCP server                                                                                                                                                                              |
| Commands                       | `get_permissions`, `save_permission_set`, `remove_permission_set`, `assign_permissions`, `set_command_rules`, `set_blocked_files`, `set_sensitive_rule`, `set_guard_options`, `save_secret`, `remove_secret`, `get_approvals`, `resolve_approval`, `revoke_grant` (architecture overview §3), each granted by name |
| Decision record                | [ADR-013 (how Plenipo lets workers use your computer safely)](../adr/ADR-013-guard-capability-broker.md)                                                                                                                                                                                                           |

## 5. Security notes

- **One door, checked in Plenipo's code.** Workers reach files, programs, and git only through
  Plenipo's tools; every call is decided by Guard and carried out by Plenipo. The AI tools' own
  tools stay off (Claude Code) or read-only (Codex). The UI configures Guard and answers
  approvals; it can never run a tool.
- **The tool server** listens on `127.0.0.1` only and admits a connection only with the key of a
  step running now (244 random bits); keys and configuration files are private to the user and
  deleted when the step ends.
- **No blanket administrator access, no silent elevation.** Privilege tools and shells are on the
  never-run list; running as administrator is a sensitive action (ask or block); nothing Plenipo
  runs is elevated.
- **No plaintext secrets in SQLite.** Values live in Windows Credential Manager; a test asserts
  the value appears nowhere in the Ledger's settings or events. Secrets are hidden wherever they
  would appear, including the AI tools' own activity.
- **Sensitive actions** can be made stricter (Blocked), never looser. Each approval covers one
  action; there is no "remember this".
- **Known limits** (ADR-013, Consequences): Codex's own read-only commands can still read outside
  the folder; approved programs are trusted to behave (no operating-system sandbox yet); the
  sensitive-action check is a safety net, so the approved list is the real gate; live streamed
  text can briefly show part of a secret split across chunks (what is recorded is filtered).

## 6. Deviations from the plan

| Deviation                                                                                              | Why                                                                          | Record      |
| ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------- | ----------- |
| Workers use Plenipo's own tools (an MCP server) rather than the AI tools' built-in tools               | One enforcement point for both AI tools; Codex's `exec` has no approval hook | ADR-013 §2  |
| The Vault is a module of the capabilities crate, not its own crate                                     | It has one consumer                                                          | ADR-013 §1  |
| Configuration is one Ledger setting document; approvals use the Phase 2 table; no migration            | Owner configuration, validated in Rust, changed atomically with events       | ADR-013 §13 |
| Only organization workers get permissions; workers started in Workers stay conversation only           | Permissions come from a role and a project folder                            | ADR-013 §4  |
| GitHub, SSH, browser, computer, MCP-server, local-network, and process tools are registered, not built | Their phases (8, 10, 11) bring them                                          | ADR-013 §14 |
| A tool call waits for its approval (the step keeps running)                                            | Simplest reliable flow; the AI tools' tool timeouts are raised to match      | ADR-013 §9  |

## 7. Owner items

| ID  | Item                                                                                                                                                                                                                                                                                                | Recommendation                                                         |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| O1  | ADR-013 (how Plenipo lets workers use your computer safely) — **Accepted** by the owner on 2026-09-26: workers use only Plenipo's own tools, inside their project's folder; sensitive actions always ask (or are blocked); secrets live in Windows Credential Manager, never in Plenipo's files.    | Done.                                                                  |
| O2  | Windows check with the **real** CLIs (~25 min): the steps in [phase-7-checklist.md](phase-7-checklist.md#owner-check-on-windows-25-minutes). Use a scratch folder, and a made-up secret. The owner accepted Phase 7 before running it (away from the PC); anything odd is fixed in a patch release. | Recommended with v0.8.0.                                               |
| O3  | Codex workers can still read (not change) files outside their folder through Codex's own read-only commands. Claude Code workers cannot.                                                                                                                                                            | Accept for now, or use Claude Code for positions near sensitive files. |
| O4  | The approved-command list is the main safeguard: `npm run *` and similar run the project's own scripts without asking.                                                                                                                                                                              | Trim it to the commands you use.                                       |
| O5  | Version **0.8.0** (Phase 7 accepted).                                                                                                                                                                                                                                                               | Done.                                                                  |
| O6  | Phase 8 (Development Department MVP) builds on these permissions.                                                                                                                                                                                                                                   | The owner starts it in a new branch.                                   |

## 8. Verification

| Check                                                                     | Result                                         |
| ------------------------------------------------------------------------- | ---------------------------------------------- |
| `pnpm check` (versions, format, lint, typecheck, tests)                   | Pass — 138 frontend tests                      |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` | Pass                                           |
| `cargo test --workspace`                                                  | Pass — 638 tests                               |
| `pnpm e2e` against the release build (Linux, Xvfb)                        | Pass — 39 of 39, including the 4 Phase 7 tests |
| Generated TypeScript bindings                                             | Up to date (`pnpm bindings` leaves no diff)    |
| GitHub CI on the PR                                                       | Linked from the PR                             |

## 9. Phase boundary

Phase 7 was accepted by the owner on 2026-09-26 and released as v0.8.0; the Windows check with
the real CLIs (§7, O2) is recommended with this release. Phase 8 has not been started.
