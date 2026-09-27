# ADR-026: SSH built into Plenipo (the russh library), not Windows' ssh.exe

- **Status:** Accepted (by the owner, 2026-09-27: "I don't mind maintaining it")
- **Date:** 2026-09-27
- **Phase:** 11
- **Goes with:** [ADR-025 (servers over SSH, through Guard)](ADR-025-servers-over-ssh.md), which
  decides what workers may do on servers. This record decides which SSH program does it.

## Context

Phase 11 needs an SSH client. Plenipo is Windows first, installs per user with no admin rights
(ADR-002, local-first architecture), and must keep these under its own control, not the owner's
SSH settings:

- **The server ID check.** Each server's host key fingerprint is pinned in Settings → Servers.
  Plenipo must refuse a different one before it sends anything, and tell the owner in plain
  words.
- **Sign-ins from the Vault.** A key or password comes from Windows Credential Manager, or the
  owner's SSH agent signs. A key must never be written to disk, and never forwarded.
- **Stopping and watching.** A command's output must reach the Activity trail as it arrives.
  **Disconnect**, **Stop all**, and time limits must stop the command on the server, not only the
  connection.
- **Tests without a real server**, on Linux and Windows CI.

There are two realistic choices. One is a Rust SSH library compiled into Plenipo. The other is
the `ssh.exe` that Windows 10 and 11 install by default (Microsoft's build of OpenSSH).

## Decision

Plenipo speaks SSH itself, with **russh** (`russh` 0.63, pure Rust, Apache-2.0), using the
**`ring`** cryptography backend. No other SSH program runs.

- **The server ID is checked inside the handshake** (`check_server_key`), against the pinned
  fingerprint, before any sign-in is sent. A mismatch is a typed error ("its server ID
  changed…"), not text to parse. No `known_hosts` file is read or written.
- **Sign-ins never touch the disk.** Keys (OpenSSH, PKCS#1, or PKCS#8, encrypted or not) are
  read from the Vault into memory. Passwords are sent through SSH's password sign-in. The owner's
  agent is reached directly: Windows' OpenSSH Authentication Agent (`\\.\pipe\openssh-ssh-agent`),
  then Pageant, or `SSH_AUTH_SOCK` on macOS and Linux.
- **Only what Plenipo asks for happens.** Plenipo requests one `exec` per command, and
  `direct-tcpip` for a forwarded port the owner listed. It never requests agent forwarding, X11,
  a terminal, a shell, or environment variables. The owner's `~/.ssh/config` is never read, so
  nothing there (a `ProxyJump`, `ForwardAgent yes`) can widen what a worker does.
- **Stopping** sends the command TERM, then KILL, through SSH, then closes the channel.
  Keepalives notice a lost connection.
- **Tests** run an SSH server built with the same library, inside the test on `127.0.0.1`. It
  covers key, password, and agent sign-in, a changed server ID, output, cancellation, and a dropped
  connection. The same tests run on Linux and Windows CI, with no internet and nothing installed.
- **`ring`, not `aws-lc-rs`:** `ring` builds with the Windows toolchain alone (no CMake or NASM).
  The only cipher left out is ChaCha20-Poly1305. AES-GCM and AES-CTR remain, and every OpenSSH
  server offers them.

## Consequences

**Good:**

- **Windows first, no admin rights.** Nothing to install or turn on. Key and password sign-in
  work on a fresh Windows account. (The SSH agent option needs Windows' OpenSSH Authentication
  Agent service running, which an administrator turns on once, or Pageant, which needs no admin.)
- **The server ID check is Plenipo's**, with one place that decides, one plain message, and an
  event (`ssh.host_key_changed`) with both fingerprints.
- **Nothing the owner configured for their own SSH use leaks into workers' use**, and the
  reverse: Plenipo never adds to the owner's `known_hosts`.
- **One code path on every platform**, tested end to end in CI.

**Costs and risks (said plainly):**

- **A smaller project than OpenSSH.** russh is widely used, but it has fewer reviewers than
  OpenSSH, and Plenipo must take its security fixes itself. (Terrapin, CVE-2023-48795, affected
  both, and both fixed it.) Plenipo updates russh like any other dependency, and a security fix
  is a patch release.
- **Larger app:** about 80 more Rust crates in the build (602 → 686), and a few megabytes in the
  installer.
- **Fewer SSH features than OpenSSH:** no `~/.ssh/config`, no jump hosts (`ProxyJump`), no
  Kerberos (GSSAPI), and no trusting a certificate authority for server IDs (Plenipo pins each
  server's own key). Hardware
  security keys (FIDO, `sk-ssh-ed25519`) work only through the SSH agent. None of these is needed
  for Phase 11. Each would be a later decision.
- **No ChaCha20-Poly1305** (see above).

## Alternatives considered

- **Windows' own `ssh.exe` (OpenSSH).** The most reviewed SSH there is, updated by Windows
  Update, with every feature above. Rejected, because each of Plenipo's rules would be a
  workaround around a program built for a person at a terminal:
  - **Server ID:** possible only through a `known_hosts` file Plenipo writes, plus
    `StrictHostKeyChecking=yes`. A mismatch is English text on standard error, which must be
    recognized in whatever words that version prints.
  - **Keys** must be files on disk (a Vault key would be written out, however briefly).
  - **Passwords:** `ssh.exe` reads them from the console only, so it needs an `SSH_ASKPASS`
    helper program.
  - **Options:** the owner's `~/.ssh/config` applies unless every option is overridden on the
    command line (`-F`, `-a`, `-o …`), and a mistake there fails open.
  - **Stopping:** ending `ssh.exe` closes the connection, but without a terminal the command on
    the server can keep running, and the `ssh` program has no way to send it a signal.
  - **Version:** it depends on the Windows version (the OpenSSH Client feature can be removed,
    and older builds differ).
  - **Tests** would need a real SSH server on each CI machine, and Windows runners do not have
    one.
- **libssh2** (the `ssh2` crate). A C library with its own Windows build (and OpenSSL, or
  Windows' crypto). Its host key and agent handling are older, and it is blocking, not async.
  Nothing it does that russh cannot.
- **The `openssh` crate** (drives the `ssh` program through multiplexing). Unix only.
- **A wrapper around russh** (for example `async-ssh2-tokio`). Adds a layer and hides the
  handshake check Plenipo needs.

## What accepting it means

Plenipo keeps its own SSH built in. It stays responsible for keeping that part up to date, and it
does not use the owner's OpenSSH setup or the servers already in their `known_hosts`. Choosing
`ssh.exe` later would be a replacement of `crates/capabilities/src/ssh.rs` behind the same broker
tools, and a new ADR.
