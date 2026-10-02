# ADR-151: One repository for Windows, Mac, and Linux

- **Status:** Accepted (by the owner, 2026-10-02, as recommended)
- **Date:** 2026-10-02
- **Phase:** 23
- **Part of:** [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)
- **Builds on:** [ADR-004 (a small monorepo that grows only when needed)](ADR-004-repository-layout.md)
  and [ADR-101 (the account service's own repository)](ADR-101-account-service-repository-name.md)

> **On screen:** nothing changes.

## In short

The Mac and Linux versions of Plenipo live in the same repository as Windows, `Seckcey/plenipo`.
They are one program, built three ways.

## Context

The owner asked whether the Mac and Linux versions should have repositories of their own.

- **It is one program.** Tauri builds Windows, Mac, and Linux apps from one copy of the code. The
  screens, Guard, the Ledger, the workforce, routing, the Liaison, licensing, and the phone's line
  are the same on every system. Only 162 lines in 53 Rust files choose by system (ADR-150).
- **Safety must not drift.** A Guard fix has to reach every system in the same release. Separate
  repositories would hold separate copies of Guard.
- **Releases are built for one repository.** Tauri's update file (`latest.json`) lists every
  system's download in one file, from one release.
- **GitHub's test machines are free here.** The repository is public, so GitHub's Mac and Linux
  machines cost nothing. A private copy would pay for every Mac minute, the most expensive kind.
- **Our own rule:** a separate repository is for a separate product that talks to Plenipo through a
  written contract, such as the account service (ADR-101). A Mac version is not a separate product.

## Decision

1. **One repository.** All Mac and Linux code, tests, packaging, and release steps live in
   `Seckcey/plenipo`.
2. **One version and one release.** A version number means the same on every system. Each release
   carries every system's download and one `latest.json` listing them all.
3. **One door per system-specific job.** Each job that differs by system (ending a program tree,
   finding which program holds a connection, the password store, the terminal's shell, starting
   at sign-in, installing an update, screen capture) gets a `platform` module in the crate that
   owns it, with a file per system (`windows.rs`, `unix.rs`, `macos.rs`, `linux.rs`). The rest of
   the code calls the door and never asks which system it is on. If two crates need the same door,
   Wave 1 may add one small crate for it, and says why (ADR-004).
4. **Moving first, changing second.** Wave 1 first moves today's Windows code behind its doors
   **without changing what it does**, and the Windows tests prove nothing moved. Mac and Linux code
   is added after.
5. **Every change is tested on all three systems** in CI once Wave 1 makes the Mac job pass.
6. **Two optional extras later, holding no code:**
   - `Seckcey/homebrew-plenipo`, a public repository with one file that tells Homebrew (the Mac's
     popular installer) where the Mac download is (Wave 4). Homebrew requires that name pattern.
   - A list of Linux packages for `apt` on getplenipo.com, if owners ask for it. It is files on a
     web server, not a repository.

## Consequences

- One fix, one review, one release for all three systems.
- CI takes longer, because each change is also built and tested on a Mac.
- A contributor needs only one repository.

## Alternatives considered

- **A repository for each system.** Three copies of Guard, the Ledger, and the screens to keep in
  step; a safety fix could reach one system and miss another.
- **A private repository for the Mac.** Pays for every Mac test minute and hides the code that runs
  on owners' Macs, which the Elastic License (ADR-021) keeps readable.
- **A shared core repository with three app repositories.** More moving parts and version pinning
  between them, for no gain: Tauri already builds all three from one.
