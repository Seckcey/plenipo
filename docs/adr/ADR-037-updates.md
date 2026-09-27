# ADR-037: Updates — from GitHub Releases, signed twice, installed only when you say so

- **Status:** Accepted (by the owner, 2026-09-27, as recommended, with one choice: checking for
  updates is always on, for Free and Pro alike; there is no switch to turn it off)
- **Date:** 2026-09-27
- **Phase:** 13
- **Number:** ADR-034 and ADR-035 are taken by pull request #69 (the security fixes), so this is
  ADR-037. The owner approved it as "ADR-036" before the renumbering.
- **Related:** ADR-021 (Free and Pro editions), ADR-022 (subscription and the weekly license
  check), ADR-036 (background work), and ADR-013 (Guard and the capability broker).

## Context

Phase 13 of `ROLLOUT_PLAN.md` asks for a "safe update mechanism" and a "version rollback
strategy". The owner set the rules:

- Plenipo checks for a new **signed** release, **tells you**, and installs **only when you say
  so**, and only a release that 8 West signed.
- **A failed update leaves the old version working.** The version stays shown.
- Updates must fit the Free and Pro editions and the weekly license check (ADR-021, ADR-022).
- Anything that reaches the network goes through Guard and the capability broker.

What exists in 1.8.0: the Release workflow builds the installer on GitHub, signs it as
**8 West Ventures, LLC** through Azure Artifact Signing (Windows' own "who made this program"
signature), checks that signature, and attaches the installer to a GitHub release. Plenipo
itself never contacts the internet; getting a new version means downloading the installer from
GitHub by hand. The version is shown in the top bar and in **Settings → About**.

Tauri offers an updater add-on (`tauri-plugin-updater`). It checks a small file, downloads, checks
a second signature made with an updater key, runs the installer, and then **ends Plenipo on the
spot** (`std::process::exit`), skipping Plenipo's careful shutdown that stops and records work.

## Decision

### 1. Where updates come from

- **GitHub Releases of `Seckcey/plenipo`, nothing else.** Each release carries three files: the
  installer (`Plenipo_X.Y.Z_x64-setup.exe`), its updater signature (`…-setup.exe.sig`), and a
  small `latest.json` saying the newest version, its release notes, where the installer is, and
  its signature (the same layout Tauri's updater uses).
- Plenipo reads `https://github.com/Seckcey/plenipo/releases/latest/download/latest.json`.
  "Latest" skips pre-releases, so a test build never reaches you.
- The request carries **nothing about you or your work**: no key, no license, no project, no
  ID. GitHub sees only what any download shows (the internet address of your connection).

### 2. How updates are signed and checked

Two signatures, both made only by the Release workflow on GitHub:

1. **Windows' signature (Authenticode)** by 8 West Ventures, LLC, as today. It is what Windows
   and your antivirus check.
2. **The updater signature**, made with a separate updater key (the private half lives only in
   GitHub secrets). The matching public half is built into Plenipo when the Release workflow
   builds it. Before running anything it downloaded, Plenipo checks:
   - the signature matches the public key built into it (so only 8 West's releases pass), and
   - the version written **inside** the signature equals the version `latest.json` announced
     (so nobody can dress an old release up as a new one to take you backwards).
     If either check fails, the download is thrown away and nothing changes.

Plenipo's own update code does this, not Tauri's add-on, so that:

- the check and the download go **through Guard**: Guard allows Plenipo's own update requests
  only over `https`, and only to GitHub's release
  addresses (`github.com/Seckcey/plenipo/releases/…` and GitHub's download servers it sends
  Plenipo to); any other address, or a redirect elsewhere, is refused and recorded;
- installing goes through Plenipo's normal shutdown, so work is stopped and recorded, never cut
  off;
- and every step can be tested on a PC with no internet (the tests use a local test server and a
  throwaway test key).

A copy of Plenipo built on a developer's PC has no public key built in, so it **cannot install
updates** and says so; it can still show you the newest version and a link to it. CI's test copies
(the Windows installer tests and the end-to-end tests) are built with a throwaway key made in each
run and look for updates on `127.0.0.1` only, so the tests can install an update the way Plenipo
does; those copies are never released.

### 3. When Plenipo checks, and what you see

- **Checking is always on, for Free and Pro alike** (the owner's choice, 2026-09-27): Plenipo
  checks a few minutes after it starts and then once a day. There is no switch to turn it off.
- **Settings → Updates** shows this version, when Plenipo last checked and what it found, and a
  **Check now** button.
- When a newer version exists: a pop-up notice once per version ("Plenipo 1.10.0 is ready to
  install"), a small **Update ready** mark on the version in the top bar, and in
  **Settings → Updates** its release notes with **Install now** and **Not now**.
- **Install now** while work is running asks first: "2 tasks are running. Installing stops them.
  Stop them and install?" **Not now** keeps everything as it is.
- Nothing is downloaded until you choose **Install now**.

### 4. How installing works, and the rollback plan

In order, stopping at the first problem and leaving the old version untouched:

1. **Download** the installer and **check both conditions of §2**. Any failure: "The update could
   not be checked, so it was not installed. Plenipo 1.9.0 is still installed." Nothing changed.
2. **Back up the Ledger** ("before updating to 1.10.0"). If the backup fails, stop there.
3. **Stop the work** the normal way (as **Quit** does), and record it.
4. **Run the installer** in its "progress bar only" mode, with the flag that makes it an update
   and reopen Plenipo afterwards. Plenipo then quits.
5. The new version starts, records "Updated from 1.9.0 to 1.10.0", and shows its version.

**Rollback plan** (going back to an older version):

- **Before the installer runs** (download, check, backup, or starting the installer fails): the
  old version keeps running; nothing to undo.
- **Your data is never in the program folder.** The installer replaces only the program
  (`%LOCALAPPDATA%\Plenipo`); your Ledger, backups, and settings live in
  `%LOCALAPPDATA%\com.eightwest.plenipo` and are not touched by installing.
- **To go back**, run the older version's installer from GitHub Releases (every release keeps
  its installer; installing an older version over a newer one is allowed). If the newer version
  changed the Ledger's layout, the older version refuses to open it (it never guesses) and
  **Settings → Diagnostics → Restore** offers the backup made before the update. 1.9.0 itself
  does not change the Ledger's layout, so going back from 1.9.0 to 1.8.0 needs no restore.
- **Plenipo never goes back, or forward, by itself.**

### 5. Free, Pro, and the weekly license check

- **Updates are the same for Free and Pro.** One installer for both (ADR-021), and every update
  goes to everyone. Fixes and safety never depend on paying (ADR-021, item 4).
- **The update check and the license check are separate and never combined.** The update check
  goes to GitHub and sends nothing about you. The weekly license check (ADR-022; Pro only, not
  built yet) goes to 8 West and sends the key's ID and the version, and nothing else. Neither
  one ever carries the other's information.
- **Updating never changes your edition** and never touches your license key. A Pro subscription
  that ends does not stop updates.
- **ADR-022's promise stands:** a Free install never contacts **8 West**. Every install, Free
  and Pro, contacts **GitHub** once a day to see whether a new version exists, which
  `docs/editions.md` and the README say plainly. That request carries nothing about the owner or
  their work.

### 6. What the owner does once (the updater key)

The updater key is separate from the Azure signing. The owner makes it on their own PC, and adds
it to GitHub themselves. It is never pasted into a chat and never committed.

1. In PowerShell, in the `plenipo` folder (after `pnpm install`):

   ```powershell
   pnpm --filter @plenipo/desktop tauri signer generate -w "$env:USERPROFILE\.tauri\plenipo-updater.key"
   ```

   It asks for a password (make a strong one and keep it in your password manager). It makes
   two files: `plenipo-updater.key` (the **private** key, secret) and `plenipo-updater.key.pub`
   (the **public** key, not secret).

2. On GitHub: **Seckcey/plenipo → Settings → Secrets and variables → Actions**:
   - **Secrets** tab → **New repository secret**, twice:
     - `TAURI_SIGNING_PRIVATE_KEY`: everything in `plenipo-updater.key` (open it in Notepad,
       select all, copy).
     - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the password from step 1.
   - **Variables** tab → **New repository variable**:
     - `PLENIPO_UPDATER_PUBLIC_KEY`: everything in `plenipo-updater.key.pub`.

3. Keep a backup of `plenipo-updater.key` and its password somewhere safe (for example your
   password manager). **If the key is lost**, copies already installed will refuse updates
   signed with a new key, and everyone must install the next version by hand once.

The Release workflow refuses to publish a release when any of the three is missing, and says
which.

## Consequences

- **You learn about new versions without looking**, and nothing changes until you say so.
- **A tampered or wrong download cannot be installed**: it must carry 8 West's updater signature
  and the version it claims. A copy of GitHub going bad, a changed `latest.json`, or a
  redirected download is refused.
- **Plenipo now talks to the internet by itself for the first time** (GitHub, once a day,
  always). The README, `docs/editions.md`, and `SECURITY.md` must say so. Someone who wants no
  internet traffic at all can block it with a firewall; Plenipo then says it could not check,
  and keeps working.
- **The owner holds one more secret** (the updater key). Losing it means one manual install for
  everyone; leaking it means someone could make an update Plenipo accepts, so it lives only in
  GitHub secrets and the owner's password manager.
- **Installing still stops running work.** The updated program replaces the one doing the work,
  so work cannot continue through an update; Plenipo asks first and records what it stopped.
- **The installer's own failure** (a crash halfway through copying files) is the one case the
  old version may not survive. Then you run the installer again, or the older one from GitHub
  Releases; your data is not in the program folder, so it is safe either way.

## Alternatives considered

- **Tauri's updater add-on as it is.** Least code, but it ends Plenipo without the careful
  shutdown, its requests cannot go through Guard, and its checks are hard to test without the
  internet. Its file layout and signature format are kept, so the Release workflow uses Tauri's
  own signing tool.
- **Installing updates automatically.** Rejected by the owner's rule: only when you say so.
- **An 8 West update server.** It would let 8 West count installs, which ADR-022 promises not to
  do for Free owners, and it is one more service to run. GitHub Releases already hosts every
  installer.
- **Only Windows' signature (Authenticode), no updater key.** Checking it from Plenipo needs
  low-level Windows code the project does not allow (`unsafe_code = "forbid"`), and the Azure
  certificate changes every few days, so Plenipo could not pin it. The updater key is one fixed
  key that Plenipo can check with ordinary code.
- **A switch to turn checking off.** Proposed with checking on by default; the owner chose
  checking always on, with no switch, so security fixes reach every owner, Free and Pro.
