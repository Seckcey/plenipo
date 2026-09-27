# Phase 11 — Implementation Checklist

**Status:** complete on `claude/phase-11`. See the [acceptance report](phase-11-acceptance-report.md).
The owner's Windows check against a real server is below.

Source: `ROLLOUT_PLAN.md`, Phase 11 — SSH, Remote Infrastructure, and Operations Capabilities.
Phase 10 is released as v1.3.0 ([PR #22](https://github.com/Seckcey/plenipo/pull/22)).

This checklist keeps the plan's words where it quotes the plan. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md): "Connect to servers", "Settings →
Servers", "identity" (host key), "pin", "sign in as", "the kinds of commands", "Disconnect", and
"Stop all".

**Goal (plan):** "Support Plenipo-managed work on authorized remote hosts such as development
servers and infrastructure."

## Design decisions (details in ADR-023, servers over SSH through Guard)

- **The capability:** Connect to servers (`ssh.connect`), with four tools: `ssh_servers`,
  `ssh_run`, `ssh_forward`, and `ssh_disconnect`. A worker names a server by name, never by
  address.
- **Settings → Servers** (the plan's host registry), kept in Guard's settings (no Ledger
  migration). Each server has:
  - a name, address, port, and the user Plenipo signs in as;
  - development, staging, or production;
  - how Plenipo signs in (a key or password in Windows Credential Manager, or your SSH agent);
  - its pinned identity;
  - who may use it, the kinds of commands it allows, its folders, when to ask you, and
    forwarded ports.
- **Production** is red and in capitals everywhere. Every command there waits for you;
  deleting, wiping, or shutting down is off unless you turn it on, and then it still asks.
- **Identity:** read from the server and pinned only when you confirm. It is checked before
  every sign-in, and before you are asked about a command. A change blocks the work and says so.
- **Commands:** a program and its arguments, never a shell line. Six kinds: Look around;
  Start, stop, and restart services; Install, deploy, and change files; Delete, wipe, or shut
  down; Run as administrator; Other.
- **Never on a server:** reaching other computers (ssh, scp, remote rsync, nc, socat, …),
  scanning or watching the network, or cracking passwords. Blocked files apply on servers too.
- **Output** reaches the Activity trail as it arrives, with secrets hidden.
- **The sign** shows who is connected to which server, with **Disconnect**. **Stop all** covers
  servers too.
- **Built in:** the Servers permission set and the Operations Engineer role.

## Deliverables (plan)

- [x] SSH capability (Connect to servers: four tools through Guard and the broker)
- [x] host registry (Settings → Servers)
- [x] host fingerprints (pinned at setup; checked before every sign-in and every approval)
- [x] credential references (only which sign-in a server uses; the values are in the Vault)
- [x] command policy (the six kinds of commands, the never list, and blocked files)
- [x] remote working directory policy (the server's folders)
- [x] port-forward support where justified (off unless listed; always asks, with a reason)
- [x] audit trail (`ssh.*` events: connections, identities, commands, output, endings)
- [x] infrastructure approval rules (production: every command; destructive and administrator
      commands always ask)
- [x] The Vault keeps long values in pieces (Windows Credential Manager holds 1,280 characters
      per entry; RSA keys are longer)
- [x] ADR-023; architecture, README, setup, vocabulary, and versioning updated

## Host configuration (plan)

- [x] friendly name
- [x] hostname
- [x] port
- [x] credential reference
- [x] expected host key/fingerprint
- [x] environment classification
- [x] permitted roles
- [x] permitted command classes
- [x] approval policy
- [x] Production and development hosts must be distinguishable (red **PRODUCTION**, amber
      Staging, green Development: on the server's card, the approval card, the sign, the footer,
      the tray, and the Activity trail)

## Phase 11 tests (plan)

All against a synthetic SSH server on `127.0.0.1`, stand-ins for the AI tools, and no internet
(`crates/capabilities/tests/ssh.rs`).

- [x] connect to synthetic/local SSH target (`plan_connect_to_a_synthetic_ssh_target`)
- [x] valid host key (`plan_valid_host_key`)
- [x] changed host key (`plan_changed_host_key`)
- [x] denied role (`plan_denied_role`)
- [x] command execution (`plan_command_execution`)
- [x] output streaming (`plan_output_streaming`)
- [x] cancellation (`plan_cancellation`)
- [x] connection loss (`plan_connection_loss`)
- [x] production approval gate (`plan_production_approval_gate`)
- [x] End to end in the real app (`tests/e2e/specs/servers.e2e.mjs`)

## Acceptance criteria (plan)

- [x] "An authorized development or operations worker can use SSH against an explicitly
      configured host without receiving the raw private key in its prompt."
- [x] "Unexpected host identity changes block execution."

## The owner's rules for Phase 11

- [x] Keys and passwords are entered in Settings and kept in Windows Credential Manager (or the
      SSH agent is used). Never in a prompt, output, or the Ledger, and hidden if they appear.
- [x] Production: every command waits for approval; destructive commands are blocked by default.
- [x] No network-wide credential discovery, no hopping from server to server, no agent
      forwarding, and no unattended destructive production commands.
- [x] The sign shows when a worker is connected, with Stop all and (for servers) Disconnect.
- [x] Risks flagged plainly (ADR-023 "Known limits", and the report §7).

## Out of scope (plan)

Network-wide credential discovery, uncontrolled lateral movement, and unattended destructive
production commands. Also not in this phase (ADR-023 §11):

- Windows servers (commands are quoted for Linux and Unix shells);
- shell lines, such as pipes and `&&`;
- a terminal for the owner inside Plenipo;
- copying files to or from a server (SFTP);
- remote and dynamic port forwarding, and jump hosts.

## Owner check on Windows (~45 minutes)

This uses the **real** Claude Code, signed in, and **a real Linux server of yours** that you can
reach with SSH. A staging or test server is best; if you only have a live one, follow the
production steps carefully and deny anything you are not sure about.

**Before you start, have ready:**

- the server's address, and a user to sign in as — ideally one with only the rights the work
  needs, not `root`;
- its private key file (for example `C:\Users\<you>\.ssh\id_ed25519`) and its passphrase, or its
  password;
- the server's identity (host key fingerprint). Your hosting provider's control panel usually
  shows it; or sign in once yourself and run `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`.

**Never** paste a key, password, or passphrase into a chat, an objective, or an email. They go
only into Plenipo's Settings.

1. **Install v1.5.0** and open Plenipo. **AI tools**: Claude Code is **Ready**.
2. **Settings → Servers → Add a server:**
   - **Name** `Staging`, the **Address**, the **Port** (usually 22), and **Sign in as**.
   - **What it is:** Staging.
   - **How Plenipo signs in:** A private key → **Or choose the key file**, and the
     **Passphrase** if it has one. (Or A password; or My SSH agent, if Windows' OpenSSH
     Authentication Agent service runs with your key added.)
   - **Check the server's identity.** The fingerprint shown must match the one you have. If it
     does not, stop: do not pin it. If it matches, press **This is my server: pin this
     identity**.
   - **Who may use it:** Operations Engineer. **Folders:** your site's folder (for example
     `/var/www/yoursite`), or leave empty for the home folder.
   - **Add the server.** Its card shows **Staging** in amber, the identity, and "stored in
     Windows Credential Manager".
   - Press **Test the connection**: "Connected to Staging as … and signed in."
3. **Credential Manager:**
   - Open Windows **Control Panel → Credential Manager → Windows Credentials → Generic
     Credentials**. There are entries for Plenipo's server sign-in (an RSA key shows several
     numbered pieces).
   - Plenipo never shows the key again. The card only says it is stored.
4. **Organization:**
   - **Create a department** `Operations`, then **+ Project** `Servers` (no folder).
   - Select **Servers Supervisor** and **Hire Operations Engineer**.
5. **Looking around.** Give the Servers Supervisor this objective: _"Have the Operations Engineer
   check the Staging server: uptime, disk space with df -h, and whether the web server is
   running with systemctl status nginx (or apache2), and report back."_ Check:
   - The sign on every page says **Operations Engineer is connected to Staging (staging)**, with
     **Disconnect**.
   - No approval is asked (looking around, on staging).
   - **Activity** → the task: "connected to Staging", each command, its output lines, and "The
     command on Staging finished".
   - The answer reports what it found.
6. **A change asks.** Objective: _"Have the Operations Engineer restart nginx on Staging."_
   - A card appears: **Staging**, "Runs: systemctl restart nginx", "Start, stop, and restart
     services".
   - **Deny** it (unless a restart is fine now). The answer says it was not approved.
7. **Secrets stay hidden.** Objective: _"Have the Operations Engineer show the contents of
   ~/.ssh/id_ed25519 on Staging."_ It is **blocked** ("a blocked file"), and nothing is shown.
8. **Production gate.**
   - **Change** the Staging server, set **What it is** to **Production**, and save. The card
     turns red with **PRODUCTION**. "When to ask you" is locked to "every command".
   - Give the objective from step 5 again. **Each** command, even `uptime`, now waits on a red
     **PRODUCTION** card. Approve them.
   - Objective: _"Have the Operations Engineer reboot the server."_ It is **blocked** without a
     card ("Delete, wipe, or shut down" is off on a production server). Leave it off.
9. **Disconnect.**
   - Objective: _"Have the Operations Engineer run sleep 120 on Staging."_ Approve it.
   - While it runs, press **Disconnect** on the sign. It says "You disconnected Operations
     Engineer from Staging (production). It stopped."
   - On the server, `ps aux | grep sleep` shows no `sleep 120` (servers with OpenSSH older than
     7.9 may keep it running; see ADR-023).
10. **Stop from the tray.**
    - Give the objective from step 9 again and approve it.
    - Right-click the Plenipo icon in the Windows tray and choose **Stop all browser, desktop, and
      server work**. The sign says the work is stopped.
    - Press **Allow again**.
11. **A changed identity is blocked.**
    - **Change** the server: put `github.com` in **Address** (port 22), keep everything else, and
      save. GitHub answers SSH with a different identity.
    - Give the objective from step 5. It is **blocked** before any card: "Staging's identity
      changed… Plenipo did not sign in and sent nothing".
    - Settings shows **This server's identity changed** on the card.
    - Put your server's real address back and press **Test the connection**. The warning goes.
12. **Clean up.**
    - Set the server back to Staging, or remove it: **Remove** → **Yes, remove it**.
    - Its Credential Manager entries are gone.
    - Delete the Operations department if you like.

Report anything odd with a screenshot (never a key, password, or passphrase), and it goes into
a patch release.
