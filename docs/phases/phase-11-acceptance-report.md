# Phase 11 — Acceptance Report

|              |                                                                                                                                                                                                                                                 |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 11 — SSH, Remote Infrastructure, and Operations Capabilities                                                                                                                                                                                    |
| **Branch**   | `claude/phase-11` ([PR #28](https://github.com/Seckcey/plenipo/pull/28))                                                                                                                                                                        |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings` (no diff), and the full `pnpm e2e` against the release build. GitHub CI: Rust, Frontend, E2E (Linux), Windows — see the PR.                                            |
| **Date**     | 2026-09-27                                                                                                                                                                                                                                      |
| **Result**   | Both acceptance criteria and all nine Phase 11 tests pass against a synthetic SSH server on `127.0.0.1`, stand-ins for the AI tools, and no internet; the owner's rules hold. Version **1.6.0**. The owner's Windows check is in the checklist. |

Screenshots (from the end-to-end run in the real app):

- [Settings → Servers, with a production server](evidence/phase-11/servers-settings.png)
- [adding a server: its server ID checked and pinned](evidence/phase-11/server-form.png)
- [Settings → Switches → Remote computers (SSH)](evidence/phase-11/server-switch.png)
- [a production command waits on a red PRODUCTION card](evidence/phase-11/server-approval-card.png)
- [the sign while a worker is connected, with Disconnect](evidence/phase-11/server-sign.png)
- [after Disconnect](evidence/phase-11/server-disconnected.png)
- [the Activity trail: commands and their output, with a secret hidden](evidence/phase-11/server-trail.png)
- [a changed server ID blocks the work, and Settings says so](evidence/phase-11/server-id-changed.png)

Test totals: **718 Rust** (Linux, including 13 server integration tests against the synthetic
SSH server and 16 browser tests against a real headless Chromium) · **168 frontend** · **58
end-to-end** against the real release binary (6 Phase 1 + 6 Phase 2 + 9 Phase 3 + 5 Phase 4 + 5
Phase 5 + 5 Phase 6 + 4 Phase 7 + 4 Phase 8 + 6 Phase 10 + 3 switches and learning + 5 Phase
11).

The screenshots were taken from the release build made just before the version number was set,
so the corner shows v1.4.0; nothing else changed between them.

On screen the plan's hosts are **servers** (or **remote computers**), the host registry is
**Settings → Servers**, a host key fingerprint is the **server ID**, environments are **Test**,
**Staging**, and **PRODUCTION**, command classes are **the kinds of commands**, and "user takes
control" is **Disconnect** for servers ([word list](../design/vocabulary.md)). Quotes from the
plan keep the plan's words.

CI has no AI tool accounts, and tests never touch the internet or a real server.

- **The AI tools** are `plenipo-fake-agent`, installed as `claude`. It follows a script and calls
  Plenipo's server tools through the real tool relay, like a real AI tool would.
- **The servers** are `plenipo-test-sshd` (`crates/capabilities/tests/support/sshd.rs`): a real
  SSH server built with the same library, on `127.0.0.1`, with its own server ID. It accepts a
  key, a password, or an agent's key, runs a few synthetic commands (`uptime`, `whoami`, `cat`,
  `systemctl`, `count`, `sleep`, a program that ignores TERM, one that fails, and one that drops
  the connection), records every request it gets, and can forward a port to a local service.
  It never runs anything on this computer.
- **The SSH agent** in the end-to-end test is the real OpenSSH `ssh-agent`, holding a new key.

## 1. Acceptance criteria → evidence

| #   | Criterion (ROLLOUT_PLAN.md)                                                                                                                           | Result (stand-ins) | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | An authorized development or operations worker can use SSH against an explicitly configured host without receiving the raw private key in its prompt. | **Pass**           | Integration `plan_connect_to_a_synthetic_ssh_target`: an Operations Engineer runs a command on a server set up in Settings, signing in with a key from the Vault. The test then searches every prompt, tool result, answer, file Plenipo wrote, and Ledger entry: the key appears in none of them. The server saw no request for a terminal, environment variables, agent forwarding, or X11. E2E `acceptance: every production command waits…` does the same in the real app with the SSH agent: [card](evidence/phase-11/server-approval-card.png), [trail](evidence/phase-11/server-trail.png). |
| 2   | Unexpected host identity changes block execution.                                                                                                     | **Pass**           | Integration `plan_changed_host_key`: the server is replaced by one with another server ID at the same address. The worker's command is blocked before Plenipo signs in or sends anything; on production, before any approval card. The worker and the owner are told plainly, and it stays blocked until the owner pins the new ID. E2E `a server showing another server ID is blocked…`: [screenshot](evidence/phase-11/server-id-changed.png).                                                                                                                                                   |

## 2. Required Phase 11 tests → evidence

`crates/capabilities/tests/ssh.rs` runs the whole stack against the synthetic SSH server:

- Workforce, Router, and Liaison;
- the agent runtime and supervisor;
- Guard, and the broker with its tool server and relay;
- a file-backed Ledger, and a Vault that holds at most 1,280 characters per entry, as Windows
  Credential Manager does.

| Plan test                             | Test                                     | What it shows                                                                                                                                                                                                                                                        |
| ------------------------------------- | ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| connect to synthetic/local SSH target | `plan_connect_to_a_synthetic_ssh_target` | Key sign-in from the Vault; the connection, command, and output are recorded; the key is nowhere a worker or the Ledger could see it; nothing but `exec` is asked of the server.                                                                                     |
| valid host key                        | `plan_valid_host_key`                    | **Check the server ID** reads it without signing in; **Test the connection** signs in with the pinned ID; a worker's connection is accepted.                                                                                                                         |
| changed host key                      | `plan_changed_host_key`                  | Blocked before signing in, on test and production servers alike, and before any card; `ssh.host_key_changed` has both fingerprints; Settings shows **This server's ID changed** until the owner pins the new one or a test succeeds with the old one.                |
| denied role                           | `plan_denied_role`                       | A role the server does not list is blocked before connecting; a role without the permission gets no server tools; a kind of command the server does not allow, and `ssh` from the server, are blocked without reaching it.                                           |
| command execution                     | `plan_command_execution`                 | A program and its arguments; shell characters stay text; a failing command reports its exit code and error output; a change asks first; a secret the server prints is hidden.                                                                                        |
| output streaming                      | `plan_output_streaming`                  | Output reaches the Activity trail in several entries while the command runs, and the sign shows its latest line; the worker gets all of it at the end.                                                                                                               |
| cancellation                          | `plan_cancellation`                      | **Disconnect** stops the command (TERM on the server) and closes the connection; the worker is told and may not use servers again in that step. A program that ignores TERM gets KILL. **Stop all** does the same for every worker, and holds until **Allow again**. |
| connection loss                       | `plan_connection_loss`                   | The connection drops while a command runs: the worker is told the outcome is unknown and to check before running it again; the loss is recorded; the next command connects again.                                                                                    |
| production approval gate              | `plan_production_approval_gate`          | Every command on production waits, with the server's name, PRODUCTION, its address, and exactly what will run; a refused command never reaches the server; deleting, wiping, and shutting down are blocked there, and still ask when the owner turns them on.        |

Also in `ssh.rs`: `port_forwarding_is_listed_asked_and_closed`,
`settings_keep_sign_ins_in_the_vault_only` (a long RSA key in pieces; removing a server removes
its sign-in), `switching_remote_computers_off_disconnects_and_blocks`, and
`lessons_from_servers_always_wait_for_the_owner`. Guard's unit tests cover the kinds of
commands, the never list, folders, validation, and the `Production` kind (the stricter of the
owner's rules wins).

**End to end** (`tests/e2e/specs/servers.e2e.mjs`, the real app, the real `ssh-agent`):

1. Settings → Servers says remote computers are switched off; the owner adds a production
   server, checks and pins its server ID, tests the connection, and turns the switch on.
2. An Operations team is built on the Organization map.
3. Every production command waits on a red PRODUCTION card; the sign names the worker and the
   server; **Disconnect** stops the running command.
4. The Activity trail shows each command and its output, with a password in a file hidden.
5. The server comes back with another server ID: the work is blocked before signing in, and
   Settings says so.

## 3. The owner's rules → evidence

| Rule                                                                                                          | Evidence                                                                                                                                                                                |
| ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Keys and passwords only in Settings, kept in Credential Manager or the SSH agent; never in a prompt or output | `plan_connect_to_a_synthetic_ssh_target`, `settings_keep_sign_ins_in_the_vault_only`; the Tauri test `servers_are_configuration_only`; Settings shows only whether a sign-in is stored. |
| Production: every command waits; destructive commands blocked by default                                      | `plan_production_approval_gate`; Guard `production_asks_for_every_command_and_blocks_destroying` and `servers_are_checked_with_their_settings`.                                         |
| No credential discovery, no hopping between servers, no agent forwarding, no unattended destruction           | The never list (`plan_denied_role`), the server's own record of requests (no agent forwarding), blocked files on servers, and "Delete, wipe, or shut down" always asking.               |
| The sign shows a connected worker, with Stop all and Disconnect                                               | `plan_output_streaming`, `plan_cancellation`, and the e2e [sign](evidence/phase-11/server-sign.png).                                                                                    |
| Everything through Guard and the broker                                                                       | The four tools exist only as broker tools; every call opens no connection before Guard's decision (and the server ID check).                                                            |
| The switch starts off, and off means off (ADR-023)                                                            | `switching_remote_computers_off_disconnects_and_blocks`; Guard `servers_are_checked_with_their_settings`; the e2e [switch](evidence/phase-11/server-switch.png).                        |
| Lessons from servers wait for the owner (ADR-024)                                                             | `lessons_from_servers_always_wait_for_the_owner`; Ledger `web_screen_or_server_use_anywhere_below_a_task_counts`.                                                                       |

## 4. Defects found and fixed during Phase 11

- **Asking before checking the server ID.** A production command's card appeared before Plenipo
  had connected, so the owner could be asked about a server that was no longer theirs. Plenipo
  now connects (and checks the server ID) before it asks.
- **The Vault and Windows Credential Manager.** Windows holds at most 1,280 characters per entry,
  and an RSA key is longer. The Vault now keeps long values in numbered pieces. This also fixes
  the Phase 7 secrets list, which accepted values Windows would refuse.
- **Stop all's reason** was overwritten when the worker's permissions were revoked a moment
  later; the first reason now wins.
- **A leftover tool relay** (found while testing, not changed here): if Plenipo closes while an
  AI tool waits on a tool call, the relay and the AI tool can wait forever on Linux. On Windows
  the Job Object ends them. Suggested as a separate fix.

## 5. Deliverables

See the [checklist](phase-11-checklist.md): every plan deliverable and host setting is done.

## 6. Security notes

- **Looking can read anything the signed-in user can.** Blocked file names are refused, but a
  server can hold secrets anywhere. Sign in as a limited user.
- **Secrets in output** are hidden when Plenipo can recognize them: the Vault's values (each
  line of a key, too), and common formats. An unusual secret can show.
- **Server output can try to steer a worker.** It is marked as the server's information, never
  instructions, and production asks for every command.
- **Pinning trusts the first ID the owner confirms**, as SSH itself does. Compare it with the
  hosting provider's.
- **Old servers (OpenSSH before 7.9) ignore signals,** so a stopped command may keep running
  there after its connection closes.
- **russh** (ADR-026) is a smaller project than OpenSSH; its security fixes must be taken
  promptly.

## 7. Deviations from the plan

ADR-025 §11: Linux and Unix servers only; a program and its arguments, not shell lines;
**Disconnect** in place of Take over for servers; no owner terminal or file copying. ADR-026: SSH
built into Plenipo rather than Windows' `ssh.exe`.

## 8. Owner items

- **O1.** Accept or change ADR-025 (servers over SSH, through Guard).
- **O2.** Accept or change ADR-026 (SSH built into Plenipo, not Windows' ssh.exe).
- **O3.** Run the Windows check against a real server of yours (checklist, ~45 minutes).
- **O4.** Servers are recorded as Free (your answer, 2026-09-27).

## 9. Verification

Run on Linux, on the final branch after merging `main` (v1.4.0):

- `pnpm check` (versions, format, lint, typecheck, 168 frontend tests)
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked` (718 tests)
- `pnpm bindings`, with no diff in `packages/types/src/generated`
- `pnpm e2e` against the release build (58 tests)

GitHub CI runs the same, plus the Windows job (tests, installer, and the launch smoke test).

## 10. Phase boundary

Phase 11 adds no Ledger migration (the latest is still 7). The next phase starts from v1.6.0.
