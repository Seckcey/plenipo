# ADR-023: Servers over SSH, through Guard

- **Status:** Proposed
- **Date:** 2026-09-27
- **Phase:** 11

## Context

Phase 11 of the rollout plan adds work on authorized remote hosts. Its deliverables are an SSH
capability, a host registry, host fingerprints, credential references, a command policy, a
remote working-directory policy, port forwarding where justified, an audit trail, and
infrastructure approval rules. Each host records:

- a friendly name, host name, and port;
- a credential reference and the expected host key;
- its environment, and production and development hosts must be distinguishable;
- the roles that may use it, the command classes it permits, and an approval policy.

The acceptance criterion: "An authorized development or operations worker can use SSH against an
explicitly configured host without receiving the raw private key in its prompt. Unexpected host
identity changes block execution." Out of scope: network-wide credential discovery,
uncontrolled lateral movement, and unattended destructive production commands.

The owner added these rules:

- Keys and passwords are entered in Plenipo's Settings and kept in Windows Credential Manager
  (the Vault), or the owner's SSH agent is used. They are never in a prompt, output, or the
  Ledger, and are hidden if they appear.
- On production, every command waits for the owner, and destructive commands are blocked by
  default.
- No hopping from one server to another (no agent forwarding by default), and no network-wide
  credential discovery.
- Show on screen when a worker is connected to a server, with Stop and Take over where it makes
  sense.

Workers already use Plenipo's tools only through Guard and the capability broker (ADR-013), with
per-step grants, approvals, the Ledger, and (Phase 10) the control center's sign, Stop, and Take
over (ADR-020). The `ssh.connect` capability ("Connect to servers") was registered in Phase 7 for
this phase.

## Decision

### 1. The capability and its tools

`ssh.connect` ("Connect to servers") gets four tools:

| Tool             | What it does                                                                            |
| ---------------- | --------------------------------------------------------------------------------------- |
| `ssh_servers`    | Lists the servers the worker's role may use, with their environment, folders, and rules |
| `ssh_run`        | Runs a program with arguments on a named server (the output comes back when it ends)    |
| `ssh_forward`    | Forwards a port on this computer to a destination the server allows (always asks)       |
| `ssh_disconnect` | Closes the worker's connection to one server or all of them                             |

- **The Servers permission set** (`servers`, new, built in) allows Connect to servers.
- **The Operations Engineer role** (new, built in, on call) starts with the Servers set. Its
  working instructions (ADR-019) say to look before changing, never to connect from a server to
  another computer, never to look for secrets, and to stop and ask when an identity changed or a
  command is blocked.
- No other built-in role gets the permission.
- A worker names a server by its friendly name. It can never give an address.

### 2. The server list (the plan's host registry): Settings → Servers

Each server is a record in Guard's settings (the Ledger's `guard` setting, no new table and no
migration):

- **Name, address, port, and "sign in as"** (the user name).
- **Environment:** development, staging, or production. On screen, production is red and in
  capitals everywhere: the server's card, the approval card, the sign, the footer, the tray,
  and the Activity trail. Staging is amber and development green.
- **How Plenipo signs in** (the credential reference): a private key (with its passphrase, if
  any), a password, or the owner's SSH agent. The record says which; the values are only in the
  Vault (§4).
- **The pinned identity** (host key type and SHA-256 fingerprint, with when it was pinned).
- **Who may use it:** role IDs. None means no worker may.
- **The kinds of commands it allows** (the plan's command classes, §5), and **the folders**
  commands run in and may change (the remote working-directory policy, §6).
- **When to ask:** every command; anything but looking around (the default); or not for the kinds
  allowed. Production is always "every command", whatever is chosen.
- **Ports that may be forwarded** (`host:port`). None means forwarding is off.

Every change is a `guard.server_added`, `guard.server_changed` (with the old and new identity
when it was pinned again), or `guard.server_removed` event. None of them holds a secret.

### 3. Identity (host key) pinning

- **Setting up:** **Check the server's identity** connects and reads the host key, without
  signing in (`ssh.identity_checked`). The owner compares the fingerprint with the one their
  hosting provider shows, or with `ssh-keygen -lf` on the server. They may type the one they
  expect: a different one is refused. Only then does **This is my server: pin this identity**
  pin it. A server without a pinned identity cannot be used.
- **Every connection** compares the server's host key with the pinned one during the SSH
  handshake, before signing in. If they differ, Plenipo leaves at once, having sent nothing
  about the key or password.
  - The worker is told plainly and told not to retry.
  - `ssh.host_key_changed` records the expected and the seen fingerprints.
  - Settings shows **This server's identity changed** on the server's card.
  - Workers stay blocked until the owner checks and pins the new identity.
- **Before asking the owner:** when a command needs approval, Plenipo connects first, so the owner
  is never asked about a command on a server whose identity changed.
- **Test the connection** (Settings) connects with the pinned identity, signs in, and leaves
  (`ssh.tested`).

### 4. Sign-ins stay in the Vault

- Keys, passphrases, and passwords are typed (or a key file chosen) in Settings → Servers.
  They are sent once to the app's Rust side, checked (a key must be readable with its
  passphrase), and stored in the Vault: Windows Credential Manager, or the macOS Keychain or
  Linux keyring.
- **Never returned:** the Settings screen learns only whether each value is stored.
  `vault.server_sign_in_stored` records which values were stored, never the values.
- **Long values in pieces:** Windows Credential Manager holds at most 1,280 characters per entry,
  and an RSA private key is longer. The Vault now keeps any value longer than 1,000 characters in
  numbered pieces, and joins them when it reads. This also fixes the Phase 7 secrets list, which
  accepted values Windows would refuse.
- **Hidden everywhere:** each server's key (whole, and each line of it), passphrase, and password
  join the redactor's known secrets. They are hidden in tool results, the Activity trail,
  approval cards, and all AI tool activity.
- **The SSH agent:** Windows' OpenSSH Authentication Agent (`\\.\pipe\openssh-ssh-agent`), then
  Pageant, or `SSH_AUTH_SOCK` elsewhere. It only signs the sign-in. **It is never forwarded.**
- **Never asked of a server:** agent forwarding, X11, a terminal, a shell, or environment
  variables. The tests prove the server never sees such a request.
- A worker gets only server names. Keys and passwords are read only inside the broker, only to
  sign in.

### 5. Commands: a program and its arguments, in kinds

- **No shell lines.** A worker gives a program and a list of arguments. Plenipo sends
  `cd '<folder>' && exec '<program>' '<arg>' …`, each word in single quotes. The server's shell
  reads `;`, `|`, `&&`, redirects, and `$(…)` as plain text, never as more commands.
- **Six kinds of commands** (the plan's command classes). Plenipo decides the kind from the
  program and its words:
  - **Look around** — status, logs, listings, disk space (`ls`, `tail`, `df`,
    `systemctl status`, `journalctl`, `docker ps`, `git status`, `wp plugin list`).
  - **Start, stop, and restart services** — `systemctl restart`, `service … reload`,
    `docker restart`, `pm2 restart`, `kill`.
  - **Install, deploy, and change files** — `git pull`, `apt install`, `npm ci`,
    `docker compose up`, `mkdir`, `cp`, `sed -i`, `wp plugin update`.
  - **Delete, wipe, or shut down** — `rm`, `docker rm` or `prune`, `git reset --hard`,
    `apt purge`, `wp db reset`, `DROP TABLE`, `reboot`.
  - **Run as administrator** — `sudo`, `doas`. Plenipo looks through them, so
    `sudo systemctl restart nginx` is both "Run as administrator" and "Start, stop, and restart
    services".
  - **Other commands** — scripts, programming languages, database clients, `curl`, and anything
    not recognized.
- **Kinds a new server starts with:** development and staging get Look around, services, and
  changes. Production gets Look around and services only.
- **Never, on any server:** programs that reach another computer, scan or watch the network, or
  crack passwords. That includes `ssh`, `scp`, `sftp`, `rsync` to another host, `nc`, `socat`,
  `telnet`, `nmap`, `tcpdump`, `hydra`, `john`, and similar. This is the plan's "no network-wide
  credential discovery" and "no uncontrolled lateral movement".
- **Blocked files** (the owner's Phase 7 patterns: `.env`, `*.pem`, `id_ed25519*`, …) apply to
  the arguments of commands on servers too.
- **Tuned for this owner's servers:** WP-CLI (WordPress and WooCommerce), `pm2`, Docker, and
  `systemd` commands are classified in detail.

### 6. Folders (the remote working-directory policy)

- A command runs in the server's first allowed folder, or in a folder the worker names inside an
  allowed one. With none listed, it runs in the home folder of the user Plenipo signs in as.
- `..` is refused.
- Commands that can change things (Install and change, Delete, and Other) may name only paths
  inside the allowed folders (or, with none, relative paths in the home folder). Looking may read
  anywhere the user can, except blocked files.

### 7. When the owner is asked

Guard's usual layers apply first:

1. the role's permission set, and the project and department limits;
2. the server: its roles, its pinned identity, its kinds of commands, its folders, and blocked
   files;
3. the owner's sensitive-action rules, as in Phase 7. `sudo` counts as running as administrator,
   `DROP TABLE` as wiping database data, and so on. "Blocked" in those rules blocks on servers
   too.

Then:

- **Production: every command waits for the owner.** That includes looking around.
- **Deleting, wiping, or shutting down: blocked on production by default.** If the owner turns
  it on, each such command still asks. On every server, these commands always ask (never
  unattended).
- **Running as administrator always asks.**
- On development and staging, the server's "when to ask" setting applies.
- **The approval card** shows the server's name, its environment (a red **PRODUCTION**), its
  address, the folder, the exact command, and its kind.

### 8. Port forwarding (only where justified)

- It is off unless the server lists a destination (`localhost:5432`).
- A worker asks with its reason, and **the owner is always asked**.
- The forward listens on `127.0.0.1` only, on a random port. It closes when the worker's step
  ends, or when it disconnects.
- Only local forwarding (SSH's `direct-tcpip`) exists: no remote or dynamic (SOCKS) forwarding,
  and no jump hosts. Events: `ssh.forward_opened` and `ssh.forward_closed`.

### 9. The audit trail and output streaming

Every connection, command, output, and approval is in the Activity trail:

- `ssh.connected` (the identity it showed, and how Plenipo signed in), `ssh.connect_failed`,
  `ssh.host_key_changed`, and `ssh.disconnected` (and why);
- `ssh.command_started` (the server, its environment, the command, the folder, and its kinds);
- `ssh.output`: **output as it arrives**, in batches a few times a second. Lines are hidden for
  secrets first, and the Activity trail shows them as lines. The first 2,000 lines of a command
  are kept; the rest are counted.
- `ssh.command_finished`: exit code, signal, stopped, time limit, connection lost, or refused,
  with the time and number of lines;
- `capability.used`, `guard.denied`, and `approval.*`, as for every tool.

The worker gets the end of the output (up to 300 lines) when the command ends. It is marked as
the server's information, never as instructions.

### 10. The sign, Disconnect, Stop all, and time limits

- **The sign:** while a worker holds a connection, the banner on every page, the footer, and the
  tray say "{worker} is connected to {server} ({environment})", with the latest output line.
  Production shows **PRODUCTION** in red.
- **Disconnect** (the Phase 10 Take over, for servers):
  - The worker's running command is sent TERM, then KILL.
  - Its connections and forwards close.
  - What it was waiting for is refused, and its next server call is refused for the rest of its
    step.
- **Stop all** now covers servers too, in the app, the tray ("Stop all browser, desktop, and
  server work"), and the desktop window. Nothing connects again until **Allow again**.
- **Other stops:** when a worker's step ends, is revoked, or runs past a command's time limit
  (5 minutes by default; the worker may set up to 30 minutes), its commands are stopped the same
  way. The worker is told which, and told to check before running it again.
- **Connection loss:** a quiet connection is checked every 15 seconds and counts as lost after
  three unanswered checks. A command that loses its connection ends as "connection lost". The
  worker is told whether it finished on the server is unknown, and to check before running it
  again. The next command connects again.

### 11. Deviations from the plan

- **Linux and Unix servers only.** Plenipo quotes commands for a POSIX shell (Linux, macOS, BSD).
  Windows servers with OpenSSH use cmd.exe or PowerShell, which read the quoting differently, so
  they are not supported in this phase.
- **No shell lines.** Pipes, `&&`, and redirects cannot be used. They are text, by design (§5).
- **Take over becomes Disconnect for servers.** A command running over SSH has no screen to hand
  over, so the worker stops and disconnects. The owner continues in their own SSH program.
- **No terminal for the owner** inside Plenipo, and no file copy tools (SFTP) in this phase.

## Consequences

- An Operations Engineer can check status and logs, restart services, and deploy on servers the
  owner set up. Every step is visible, recorded, and stoppable, and nothing it holds can reveal a
  key or password.
- The owner answers every production command. That is deliberate, and it can be many cards
  during a busy task.

**Known limits** (said plainly to the owner):

- **Old servers may keep running a stopped command.** Stopping a command sends it TERM, then KILL,
  through SSH. Servers with OpenSSH older than 7.9 (2018) ignore those signals, so a stopped
  command may keep running there even though the connection closes.
- **The kinds of commands are a careful list, not a guarantee.** A program Plenipo does not
  recognize is "Other commands", off by default. A recognized program can still do surprising
  things (a service's own restart script, for example).
- **Looking can read anything the user can read,** except blocked file names. Sign in as a user
  with only the access the work needs.
- **Output is hidden only for known secrets:** the Vault's values and recognizable formats (keys,
  tokens, `PASSWORD=` settings). An unusual secret printed by a server cannot be recognized.
- **The first identity is the owner's to check.** Pinning trusts what the owner confirms when
  setting up (as SSH does the first time). Check the fingerprint with the hosting provider.
- **A server's own access is its own.** `git pull` on a server uses the server's deploy key to
  reach GitHub. That is the server's access, not Plenipo's, and it is allowed as a change.
- **The SSH agent offers all its keys** to the server, one at a time, as `ssh` does.
- **One connection per worker step and server.** Commands in the same step share it. Other
  workers and later steps connect again.

## Alternatives considered

- **Windows' own `ssh.exe`.** Rejected: host key checks would depend on `known_hosts` files, the
  key would have to be a file on disk, agent forwarding would depend on the owner's SSH
  settings, and stopping a command and reading its output live are unreliable.
- **libssh2 or OpenSSH libraries.** Rejected: C libraries with their own Windows build, for
  nothing a Rust SSH library cannot do.
- **The `aws-lc-rs` cryptography in the Rust SSH library.** Rejected in favor of `ring`, which
  builds with the Windows toolchain alone (no CMake or NASM). The only cipher lost is
  ChaCha20-Poly1305; AES-GCM and AES-CTR remain, which every OpenSSH server offers.
- **Shell lines with a blocklist.** Rejected: a shell line can hide any command behind quoting,
  variables, or `$(…)`. A program and its arguments can be checked word by word.
- **Trust on first use without the owner.** Rejected: the owner pins the identity knowingly, once,
  and every change after that blocks.
- **Asking the owner only for changes on production.** Rejected: the owner's rule is that every
  production command waits for approval.
