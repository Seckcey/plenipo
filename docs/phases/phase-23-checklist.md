# Phase 23 — Implementation Checklist

**Status: Wave 0 delivered (2026-10-02); Wave 1 in v1.20.0 and v1.21.0, its Guard safety review done (2026-10-03); Wave 2, Linux first look, in v1.23.0 (2026-10-03)** (started 2026-10-02). Builds on v1.19.3. Below, "[x]" is done.
Plenipo is made by 8 West Ventures, LLC.

Source: `ROLLOUT_PLAN.md`, Phase 23 — Mac and Linux, and the records written for it:

- [ADR-150 (Phase 23 starts: numbers 150 to 159, what the check found, five waves)](../adr/ADR-150-phase-23-starts.md)
- [ADR-151 (one repository for Windows, Mac, and Linux)](../adr/ADR-151-one-repository-for-every-system.md)
- [ADR-152 (which systems, and in what order: Linux first, then every Mac from macOS 13)](../adr/ADR-152-which-systems-and-in-what-order.md)
- [ADR-153 (saved keys on Linux live in the system's password store)](../adr/ADR-153-saved-keys-on-linux.md)
- [ADR-154 (computer use on Linux: X11 in Wave 2, Wayland in Wave 4)](../adr/ADR-154-computer-use-on-linux.md)
- [ADR-155 (each system's own words on screen)](../adr/ADR-155-each-systems-own-words.md)
- [ADR-156 (when Plenipo cannot tell which program sent a tool call, it refuses)](../adr/ADR-156-refuse-unchecked-tool-calls.md)

**Numbers:** ADR-150 to ADR-159. Dates are Pacific time. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md), including its table "Words that change with
the system".

**Goal (plan):** "Plenipo runs on macOS and Linux as well as on Windows, with the same safety."

## In short, for the owner

- **One repository** for all three systems (ADR-151). Mac, Linux, and Windows Plenipo are the same
  program; Tauri, the tool Plenipo is built with, makes all three from one copy of the code.
- **Linux is close.** GitHub already builds and tests Plenipo on Linux for every change, including
  the full test that drives the real app. What is missing is mostly the installer, updates, and the
  parts that touch the desktop.
- **The Mac has never been built.** It needs the Apple Developer Program for 8 West Ventures, LLC
  (the D-U-N-S number is in hand) and the owner's MacBook Pro for checks.
- **Problems found, all fixed before any Mac or Linux download:** Linux forgets saved keys at each
  restart (ADR-153); a Mac would let every tool call through unchecked (ADR-156); a worker's
  programs outlive a Plenipo crash; nothing checks for root; and Guard ignores upper and lower case
  in program names, which is wrong on Linux.
- **Five waves:** get ready → the shared base → Linux first look → Mac first look → for everyone.

## Where we start from (v1.19.3, 2026-10-02)

- **One code base.** 12 Rust crates and one Tauri app in one Cargo workspace; one React front end.
- **162 lines in 53 Rust files choose by system** (the plan's 2026-09-28 sketch said about 110).
- **Linux is built and tested on every change** (`.github/workflows/ci.yml`): `cargo clippy`,
  `cargo test --workspace`, and the end-to-end suite that drives the real app through
  `tauri-driver` all run on Ubuntu. So the Linux branches compile and the shared tests pass.
- **The Mac is never built.** No CI job, no Mac installer settings, no signing.
- **Releases are Windows only** (`.github/workflows/release.yml`): one Windows job builds the NSIS
  installer, signs it as 8 West Ventures, LLC through Azure Artifact Signing (ADR-052), signs it
  again with the updater key (ADR-038), and writes a `latest.json` that lists only Windows.
- **The installer settings are Windows only:** `tauri.conf.json` builds `"targets": ["nsis"]`.
- **Licensing and the phone's line are the same everywhere.** A Pro license already covers any of a
  person's computers (ADR-110).

## What the check found

Checked against the code on 2026-10-02 (v1.18.1, then v1.19.3). "Works" means a branch for that system exists and runs in
CI; "never run" means the code is shared with Linux but no Mac has ever run it. Sizes: S small,
M medium, L large.

### The parts that keep work safe (Guard and the supervisor)

| Part                                                                                         | Windows                                   | Linux                                                                                                        | Mac                                                                                           | Size |
| -------------------------------------------------------------------------------------------- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- | ---- |
| A worker's programs all end together (`crates/runtime/src/supervisor.rs`)                    | Job object, even after a crash            | Process group: a normal stop works; **after a Plenipo crash they keep running**                              | Same as Linux, never run                                                                      | M    |
| Private pipes to the AI tools (`crates/runtime/src/pipes`)                                   | Works                                     | Works and tested                                                                                             | Same code as Linux, never run                                                                 | S    |
| Tool tickets bound to the AI tool's programs (ADR-034; `crates/capabilities/src/process.rs`) | Works                                     | Works and tested (reads `/proc`)                                                                             | **Cannot tell, so every call is allowed** with a notice (`broker.rs`, `Admission::Unchecked`) | M    |
| The settings a worker's programs get (`crates/runtime/src/policy.rs`)                        | Works                                     | Works, but leaves out a few that AI tools need to sign in                                                    | Same as Linux                                                                                 | S–M  |
| Refusing to run as administrator (`terminal.rs`, `runs_as_administrator`)                    | Works                                     | **Never checks for root**                                                                                    | **Never checks for root**                                                                     | S    |
| Program names in Guard's rules (`crates/guard/src/commands.rs`, `program_key`)               | Ignores upper/lower case, as Windows does | **Also ignores case**, so `./Deploy` and `./deploy` share one approval though Linux treats them as two files | Ignores case, which matches the Mac's usual disk                                              | S    |
| Guard's folder rules (`crates/guard/src/paths.rs`)                                           | Works                                     | Works, tested with links                                                                                     | Case mismatches refuse too much (safe); `/var` is really `/private/var`                       | S–M  |
| Lists of risky programs (Guard's defaults and add-on lists)                                  | Windows entries                           | Some Unix entries                                                                                            | **None for the Mac** (`osascript`, `security`, `launchctl`, `defaults`)                       | S    |
| Owner-only data folder (`crates/ledger/src/owner_only.rs`)                                   | Windows folder rights                     | Works (`0700`); the browser profile and screenshots rely on it                                               | Same code, never run                                                                          | S    |

### The Vault, the AI tools, and the owner's tools

| Part                                                                            | Windows                                       | Linux                                                                                                                            | Mac                                                                        | Size |
| ------------------------------------------------------------------------------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ---- |
| The Vault (`crates/capabilities/src/vault.rs`)                                  | Credential Manager                            | Kernel keyring: **forgets every key at each restart**                                                                            | Keychain, never run                                                        | M    |
| Finding each AI tool (`crates/runtime/src/agent/discovery.rs` and each adapter) | Works                                         | Works; misses some install places (Ollama, npm's global folder, nvm)                                                             | Same, and an app opened from the Dock gets a short list of program folders | M    |
| Sign-in checks for each AI tool                                                 | Works                                         | Same code; test samples were recorded on Windows                                                                                 | Same; check the own-home trick and the Keychain                            | M    |
| Install and update hints for AI tools                                           | `winget`, PowerShell                          | Shown the Windows hint                                                                                                           | Shown the Windows hint                                                     | S    |
| The owner's terminal (`crates/capabilities/src/terminal.rs`)                    | Three Windows shells; closing ends everything | The owner's own shell works, but closing only signals the shell, so its programs can linger; Settings offers Windows shells only | Same as Linux, never run                                                   | S–M  |
| "Run PowerShell scripts" (`programs.rs`)                                        | Works                                         | Needs PowerShell 7 installed; no bash or sh choice                                                                               | Same                                                                       | S    |
| The browser workers use (`crates/capabilities/src/browser/mod.rs`)              | Edge or Chrome                                | Works (CI drives Chrome); no snap or flatpak handling                                                                            | Finds `/Applications`; may use the owner's Keychain; no Brave              | S–M  |
| Signing in to a server with the SSH agent (`ssh.rs`)                            | OpenSSH agent or Pageant                      | `SSH_AUTH_SOCK`, never tested                                                                                                    | Same                                                                       | S    |
| Opening a Connection's sign-in page (`connections/mod.rs`)                      | Works                                         | Probably fails: started without the screen's settings                                                                            | `open`, never run                                                          | S    |
| Computer use (`crates/capabilities/src/desktop.rs`)                             | Works (`xcap` and Windows input)              | X11 only (`enigo`, `x11rb`); **nothing under Wayland**, Ubuntu's default                                                         | **Nothing:** no screen or mouse code                                       | L    |

### The app around the work (`apps/desktop/src-tauri`)

| Part                                                     | Windows                             | Linux                                             | Mac                                                        | Size |
| -------------------------------------------------------- | ----------------------------------- | ------------------------------------------------- | ---------------------------------------------------------- | ---- |
| Tray, and closing hides the window (ADR-037)             | Works                               | Works; menu only (Linux sends no tray clicks)     | Works through Tauri; needs a menu bar icon and Dock reopen | S    |
| Start when you sign in                                   | Autostart plugin (Run key)          | Same plugin writes the autostart file; words only | Same plugin writes a launch agent; words only              | S    |
| One Plenipo at a time                                    | Works                               | **Turned off** (test runs start several copies)   | **Turned off**                                             | S    |
| Finding an update (`crates/capabilities/src/updates.rs`) | Works                               | **Always asks for `windows-x86_64`**              | **Always asks for `windows-x86_64`**                       | S    |
| Installing an update (`upkeep_commands.rs`)              | Works                               | **"Updates are installed on Windows only."**      | **Same**                                                   | L    |
| Installer and uninstall                                  | NSIS, with quit and keep-data hooks | None                                              | None; the uninstall code looks in the Linux data folder    | L    |
| Signing                                                  | Azure Artifact Signing (ADR-052)    | Not required                                      | Developer ID and notarization, none yet                    | M    |
| Data folder, backups, logs, diagnostics, recovery        | Works                               | Works (owner-only folder)                         | Same code, never run                                       | S    |
| "Show in folder"                                         | File Explorer                       | Opens the parent folder                           | **Fails** (it calls Linux's `xdg-open`)                    | S    |

### What people see and read

| Part                                                                                                                                                                                                        | Size |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| About 46 lines of screen text say Windows (Credential Manager, "Start Plenipo with Windows", Windows Settings pages, File Explorer, Edge, PowerShell, a `D:\` example), plus 37 "this PC" and 10 tray lines | S–M  |
| Keyboard labels say Ctrl and Alt only; a Mac says Cmd and Option. The editor's Save already uses Cmd on a Mac                                                                                               | S    |
| `docs/design/vocabulary.md` **requires** Windows words in nine places, so it changes first                                                                                                                  | S    |
| The website: two "Download for Windows" buttons, "Windows 11 · x64", its build script, tests, and `auto-release.sh` expect one Windows file                                                                 | M    |
| README, FAQ, CONTRIBUTING, SECURITY, SUPPORT, the privacy notice, and `docs/development/setup.md` say Windows only                                                                                          | M    |

### Building, testing, and releasing

| Part                                                              | Windows                                      | Linux                                            | Mac                                                        | Size |
| ----------------------------------------------------------------- | -------------------------------------------- | ------------------------------------------------ | ---------------------------------------------------------- | ---- |
| CI checks on every change                                         | `cargo test`, installer tests, launch test   | clippy, `cargo test`, end-to-end on the real app | **None**                                                   | S    |
| End-to-end tests of the real app                                  | —                                            | `tauri-driver`                                   | `tauri-driver` does not support Macs; needs another driver | M    |
| Installer tests (`scripts/windows/installer-tests.ps1`)           | 374 lines of PowerShell                      | None                                             | None                                                       | M    |
| The release job and `latest.json` (`scripts/update-manifest.mjs`) | One Windows job, one system in `latest.json` | None                                             | None                                                       | M    |

## The waves (ADR-150)

Waves 2 to 4 each end with a release; Wave 4 also ends with the Phase 23 acceptance report. Waves 2
and 3 can run side by side once Wave 1 is merged, if the Apple account is ready.

| Wave | Name              | Size         | Who waits on whom                                                    |
| ---- | ----------------- | ------------ | -------------------------------------------------------------------- |
| 0    | Get ready         | small        | The records and the word table (done); a Mac job in CI; trial builds |
| 1    | The shared base   | large        | Builder. Ends with a Guard safety review on Mac and Linux            |
| 2    | Linux, first look | medium       | Builder, then the owner's check on a Linux PC                        |
| 3    | Mac, first look   | large        | Needs the Apple account. Builder, then the owner's check on a Mac    |
| 4    | For everyone      | small–medium | Builder. Wayland computer use, website, documents, Homebrew          |

"Size" compares with earlier phases: large is about Phase 13 (installer, updates, and recovery);
medium is about one wave of Phase 16. The owner's paperwork and hands-on checks set the calendar
more than the coding does.

### Wave 0 — Get ready

- [x] The records for the owner's answers (ADR-150 to ADR-156) and the plan's Phase 23 section
- [x] The word table for each system in `docs/design/vocabulary.md` (ADR-155), with a note above
      "Say this, not that" for the nine Windows-only rows
- [x] The plan's order-of-work row and summary, and the roadmap, say Phase 23 is in progress
      (with Wave 1, after the v1.19.4 pull request, which edited the same lines)
- [x] A Mac job in CI that runs clippy and `cargo test`, reported but not yet required, so every
      later change shows what still fails on a Mac. Every test group runs, and the summary lists
      each failing test. Its first run: everything compiled on a Mac except the desktop app's
      tests, which built the app's settings twice (fixed in Wave 1)
- [x] The release workflow's dry run also builds unsigned Mac and Linux files. The first ones:
      `Plenipo_1.19.3_universal.dmg` (40 MB, Apple's chips and Intel, macOS 13 or newer), and
      `Plenipo_1.19.3_amd64.deb` (23 MB; needs only the tray, web view, and window libraries)
      and `Plenipo_1.19.3_amd64.AppImage` (100 MB), built on Ubuntu 22.04

### Wave 1 — The shared base Mac and Linux both need

This is the safety wave. No Mac or Linux download comes from it, and Windows owners see no change.

- [ ] **Doors first.** Move each Windows-only job behind a `platform` door, with no change in what it
      does. The Windows tests prove nothing moved.
- [x] **Program trees that never outlive Plenipo.** On Windows a job object ends every program a
      worker started, even if Plenipo crashes. On Mac and Linux today, a normal stop works (a process
      group), but if Plenipo itself crashes, the programs keep running. A small keeper does it on
      both (ADR-157, a keeper ends programs after a crash): Linux's `PR_SET_PDEATHSIG` turned out
      to fire when a worker thread retires, not when Plenipo ends. Stop gently first, then for
      certain. The test that only ran on Windows (`children_do_not_outlive_a_crashed_owner`) runs
      on all three, and passes on GitHub's Linux and Mac machines. The Guard safety review found
      that a program can leave the group (a build tool's server, `setsid`, `tmux`) and outlive
      both; every program now carries a mark that its own programs inherit, and Stop, the end of a
      run, and the keeper end whatever still carries it (ADR-158, a program that leaves its group
      still ends with its work), tested on GitHub's Mac and Linux machines.
- [x] **Tool tickets on the Mac (ADR-034, approved programs run as the owner).** Plenipo checks that a
      tool call comes from the AI tool's own program tree. On Linux this works. On the Mac the check
      cannot tell today (`crates/capabilities/src/process.rs`), and when it cannot tell, the rule is
      "allow, and say so" (`Admission::Unchecked`). On a Mac that would be every call. Build the Mac
      lookup, and until it works, **refuse** on a Mac instead of allowing (ADR-156). Done: a call
      the check cannot run on is refused everywhere; the Mac asks its own `/usr/sbin/lsof` (full
      path, 5-second limit; any failure refuses) and follows parents with `sysinfo`; its tests
      pass on GitHub's Mac
- [x] **The terminal's programs end with it.** Closing a terminal on Mac and Linux only signals the
      shell, so programs started in it can keep running. End its whole group, as Windows does.
      Done: the shell leads its own session, so when it ends Plenipo asks every program still in
      that session to stop, waits a second, then ends the rest. A test starts a program with
      `nohup` (told to ignore the hang-up), closes the terminal, and checks the program is gone.
- [x] **Never as root.** On Windows, Plenipo refuses to open a terminal while it runs as
      administrator. On Mac and Linux it never checks for root; add that check, for the terminal and
      for the browser (which runs without its sandbox as root on Linux).
- [x] **Program names keep their case on a Mac and Linux.** Guard treated `./Deploy` and
      `./deploy` as one program because Windows does. On Linux they are two files, and a Mac disk
      can be set up the same way, so a rule that allows (and a secret's program) names a program
      exactly there; it costs at most one more question. Rules that block or always ask still
      catch any spelling and Windows' run extensions on every system. The folder check no longer
      lower-cases paths off Windows (`sensitive.rs`), and a program found through PATH counts as
      the project's own file when its real path, after links, is inside the project.
- [x] **Risky-program lists for the Mac and Linux:** new installs block `osascript`,
      `security`, `launchctl`, `defaults`, `diskutil`, `csrutil`, `tccutil`, `spctl`,
      `systemsetup`, `systemctl`, `pkexec`, and `fish`; `open` and `xdg-open` always ask.
      Add-ons refuse `osascript`, `open`, `xdg-open`, `launchctl`, and interpreters named with a
      version (`python3.12 -c`).
- [x] **Found by the first Mac and Linux runs:** the desktop app built its settings twice, which a
      Mac does not allow; and a run whose output closed slowly waited twice on a finished output
      reader, which tokio does not allow (possible on any system; a test repeats it).
- [x] **Programs Plenipo uses itself** (git, cargo, PowerShell, the browser) are found the same careful
      way as AI tools, since a Mac app opened from the Dock gets a short list of program folders.
      Done with the next item: Plenipo's own PATH gets the known folders.
- [x] **Saved keys on Linux.** Today's kernel keyring forgets every key at each restart: server
      sign-ins, Connection sign-ins, paid AI keys, and the license key. Use the Secret Service (GNOME
      Keyring or KWallet), which keeps them (ADR-153), and say plainly when a PC has none or it is
      locked. Add tests for the real password store on each system (today only the in-memory test
      store is tested). How: `keyring`'s Secret Service store alone, in pure Rust (`zbus`, so nothing
      to build against); the kernel keyring and its every-thread workaround go. GitHub's Linux
      machine starts GNOME Keyring for the real-store test and the end-to-end tests; Windows and
      the Mac test their own stores. It passes on Windows and the Mac (GitHub) and on Linux (Coastline,
      against GNOME Keyring).
- [x] **What AI tools get to see.** The list of settings passed to a worker's program is right for
      Windows; on Mac and Linux it leaves out a few that AI tools need to sign in or open a browser
      (`XDG_*`, `DBUS_SESSION_BUS_ADDRESS`, `DISPLAY`, `WAYLAND_DISPLAY`, `SHELL`, `LOGNAME`). Add only
      the ones a tool proves it needs, each with a test. Done: `SHELL` (Claude Code runs its commands
      in it), `LOGNAME`, and the `XDG_*` folders (where the owner keeps programs' settings and
      sign-ins). Never the screen, the session bus, or the owner's SSH agent, and a test says so. A
      tool that proves it needs the session bus (to read its sign-in from the password store) is
      checked in Wave 4.
- [x] **Finding AI tools.** A Mac app opened from the Dock does not get the owner's usual list of
      program folders, so Plenipo looks in the known places itself: Homebrew, npm's global folder,
      nvm and Volta, `/Applications/Ollama.app`, `/usr/bin`. It never takes the list blindly from a
      login shell, because Guard must know exactly which program it approved. Done: as Plenipo
      starts, the folders of a fixed list that exist go after its own PATH (Homebrew, `/usr/local`,
      the system's own, `/snap/bin`, `~/.local/bin`, `~/bin`, `~/.cargo/bin`, Volta, bun, npm's
      global folder, fnm's and nvm's default Node.js), so AI tools are found, and tools installed
      with npm find `node` to start. Ollama is also looked for in the Mac's `Ollama.app`.
- [ ] **Sign-in checks.** Record each AI tool's real answers on Mac and Linux (today's samples were
      recorded on Windows) and check that giving a tool its own home folder (Antigravity, Copilot)
      does not hide the Mac Keychain from it. **Moved by the owner (2026-10-03)** to Wave 2 for Linux
      and Wave 3 for the Mac: they need each AI tool signed in on a real Linux PC and a real Mac.
- [x] **Guard's path rules** on the Mac's file system, which ignores upper and lower case, and on
      Linux's, which does not; and on Mac folders that are really links (`/tmp` is `/private/tmp`).
      Checked: the project folder is kept as where it really is (after links), so paths written
      through `/tmp` or `/private/tmp` both resolve inside it, and one through a link that leaves it is
      refused. A name in other letters stays inside: the same file on a Mac's disk, a new name on
      Linux's. Blocked-file patterns catch any spelling. Tests for each, run on GitHub's Mac and
      Linux machines.
- [x] **The terminal's shells.** Today the choice is Windows PowerShell, PowerShell 7, or Command
      Prompt. Add the owner's own shell on Mac and Linux (zsh, bash, fish from `/etc/shells`).
      Done: a Mac and Linux offer "Your shell" (the owner's own, named: "Your shell (zsh)"), zsh,
      bash, and fish, each looked for in fixed places (Homebrew's first for bash and fish) rather
      than read from `/etc/shells`, and only one this PC has can be picked. A Mac starts them as
      Terminal does, as a login shell. Settings says who the terminal runs as in the system's own
      words ("as yourself — never as root"), and a choice made on another system falls back to
      this system's first one.
- [x] **One Plenipo at a time** on every system (the single-instance switch is Windows only today
      because Linux test runs start several copies; give tests their own switch instead). Done: on
      every system. The end-to-end tests start one copy after another, so they need no switch. On
      Linux the copies find each other over the desktop's session bus, and the plugin stops
      Plenipo when there is none, so it is used only where one is there.
- [x] **Updates know which system they are on** (today the update check always asks for
      `windows-x86_64`). Installing an update on each system comes in Waves 2 and 3. Done: each copy
      asks for its own (`darwin-aarch64`, `linux-x86_64`, and so on), and a release with no download
      for it is simply not offered, instead of the check failing every day.
- [x] **Screen words** come from the new vocabulary table: the system's own name for the password
      store, "Start Plenipo when you sign in", Cmd and Option on a Mac, no "Windows" where it does not
      apply. Done: one list of each system's words in Rust (`crates/core/src/words.rs`), sent to the
      screens with the app's information before the first paint; Plenipo's own messages use the same
      list. "this PC" becomes "this Mac" or "this computer", the tray becomes the menu bar on a Mac,
      "Windows closed Plenipo" becomes "Your Mac closed Plenipo", and shortcut labels read ⌘S and ⌃⇧E
      on a Mac. `pnpm bindings` writes every system's words for the screens' tests, which check the Mac
      and Linux words too. Names in the code and the Ledger (`windowsRestart`) stay.
- [x] **Files that run when opened** (found 2026-10-02): "Open in another program" refuses a
      program or a script by the end of its name, which is Windows' rule. On a Mac and Linux a file
      with no ending can run if it is marked as a program, and some kinds run or open something else
      (`.command`, `.app`, `.terminal`, `.scpt`, `.workflow`, `.fileloc`, `.webloc` on a Mac;
      `.desktop`, `.AppImage`, `.run` on Linux). Refuse those too, before Waves 2 and 3 let the
      button open files with Finder's `open` or `xdg-open`.
      Done: on a Mac and Linux a file marked as a program counts as one whatever its name, and the
      Mac's and Linux's kinds (`.command`, `.pkg`, `.dmg`, `.webloc`, `.fileloc`, `.scpt`, `.workflow`,
      `.AppImage`, `.desktop`, `.run`, and others) join the list. "Open in another program" refuses
      them, and the Files list marks them as programs; tests on GitHub's Mac and Linux machines.
- [ ] **Done when:** every test suite passes on Windows, Linux, and Mac in CI, and a Guard safety
      review of Wave 1 finds nothing open.
      The review is [its own record](phase-23-wave-1-guard-review.md): it found six things (three
      Medium, three Low), all fixed in the same pull request, with ADR-158 (a program that leaves
      its group still ends with its work). GitHub's Mac check still only reports: the owner chose
      (2026-10-03) to make it required once the flaky browser tests are fixed, so it never blocks a
      merge for a test that fails now and then on every system.

### Wave 2 — Linux, first look

- [x] **Downloads:** a `.deb` (Ubuntu 22.04, 24.04, and 26.04 LTS, and Debian 12 or newer) and an
      AppImage (most other Linux). Built on Ubuntu 22.04 so it runs on all of them (ADR-152).
      Done: the Release workflow's Linux job builds both. The `.deb` names 8 West Ventures, LLC as
      its maker and needs WebKitGTK and the tray library.
- [x] **Updates:** the AppImage updates itself, signed with the same updater key as Windows (ADR-038).
      The `.deb` shows "A new version is ready" with a download button (an `apt` list can come later).
      Done: the AppImage writes the new version next to itself, swaps it in, and starts it again,
      which waits for the old one to finish. A `.deb` copy (and the Mac's until Wave 3) says "ready
      to download" and opens GitHub's page. Opening files and pages for the owner now uses the
      desktop's own opener, with the owner's session and none of the AppImage's own settings.
- [x] **The release job** becomes one job per system and one final job that writes a single
      `latest.json` listing every system, and publishes once.
      Done: a Linux job with no secret, and the Windows job, which holds the owner's one approval
      (ADR-052), is the final one. It signs the AppImage too, writes one `latest.json`, and publishes
      everything at once. Wave 3 adds the Mac's job; a separate final job comes then if the Mac's
      signing needs one.
- [x] **Tray:** works through AppIndicator. On Linux it is menu only (no click, no tooltip). Where a
      desktop has no tray (plain Fedora GNOME), closing the window quits, as the code already does.
      Done: Plenipo asks the desktop whether it shows tray icons (a status notifier host). Where it
      does not, Plenipo makes no tray, so the window shows even at sign-in and closing it quits. A
      missing tray library no longer stops Plenipo.
- [x] **Start when you sign in:** already written by the autostart plugin; only the words change.
      Done, with one change: on Linux Plenipo writes the sign-in entry itself, in the owner's
      settings folder. The plugin's does not quote the program, so an AppImage in a folder with a
      space in its name would never start, and it cannot make a missing folder.
- [x] **The `.deb` recommends a password store** (`gnome-keyring`), since the Vault needs one
      (ADR-153); the AppImage explains it on first use.
      Done: the `.deb` recommends `gnome-keyring`. With no password store, the Vault says so in
      plain words (Wave 1).
- [x] **Browser choice:** find Chrome, Chromium, Edge, or Brave on Linux, and say plainly when a
      "snap" Chromium cannot be used.
      Done: each one's own folder first, then by name. Brave counts for Automatic and gets its own
      profile. A Chromium from the Snap Store is never used, and Settings says why.
- [x] **Computer use:** works on X11. Under Wayland (Ubuntu's default, and the only desktop on
      26.04) Plenipo refuses and says plainly that it is not ready yet (ADR-154).
      Done: under Wayland, the screen and the mouse and keyboard both refuse with the same plain
      words, which say how to choose "Ubuntu on Xorg" when you sign in.
- [x] **Delete my data:** `apt remove` never touches a person's home folder, so Plenipo gets a
      "Delete my Plenipo data" button, like the Windows uninstaller's tick box.
      Done: Settings → Info, on a Mac and Linux. It asks first (and again if work is running). It
      removes the keys Plenipo saved, and if the password store says no, nothing is deleted. Then
      it stops the work, turns off starting at sign-in, deletes only folders named
      `com.eightwest.plenipo`, and quits.
- [ ] **Installer tests** in bash on GitHub's Ubuntu: install, upgrade, update, remove, what is left.
      Install, remove, and what is left are done (`scripts/linux-package-check.sh`, in every release
      and dry run). The installed copy and the AppImage each start and keep running on a desktop.
      Upgrade and update need an earlier Linux release to start from, so they come with the
      release after the first one. Until then the AppImage's swap has its own test, and the
      owner's check covers the rest.
- [ ] **Sign-in checks on Linux** (moved from Wave 1): record each AI tool's real answers on a
      Linux PC; today's samples were recorded on Windows.
- [ ] **The owner's check on a Linux PC:** install, sign in to Claude Code, run a Development objective
      end to end with the same approvals as Windows, restart the PC, and the Vault still has its keys.
- [x] **Release** as "Linux (first look)".
      Done: v1.23.0. The owner chose (2026-10-03) to release it now as a first look, before the
      Linux PC check; what that check finds goes into the next versions.

### Wave 3 — Mac, first look

- [ ] **Signing as 8 West Ventures, LLC:** Apple's Developer ID certificate and notarization (Apple's
      malware check), with the secrets in the GitHub Environment `release` behind the owner's approval,
      like Windows (ADR-052). The release job checks the signature, the notarization, and that macOS
      will open it, just as it checks the Windows signature today.
      **Waiting (2026-10-03):** 8 West Ventures, LLC has a free Apple account, not the paid Apple
      Developer Program. A free account cannot make a Developer ID certificate or notarize, so
      this item and the Mac download wait until the owner enrolls (an organization, $99 a year,
      with the D-U-N-S number; Apple checks the company first, which can take days).
      **The other choice: an unsigned test build, for the owner's own MacBook only** (never a
      public download, never in a release). **Building it:** nothing new. The Release workflow's dry run already builds the Mac app,
      unsigned, for Apple's chips and Intel. It is kept as a download for 7 days and published
      nowhere. Signing it with no name ("ad hoc", `signingIdentity: "-"` in
      `tauri.macos.conf.json`) is worth adding, because an Apple-chip Mac may call an app with
      no signature at all "damaged". **What the owner sees:** macOS says Apple could not check it for malware and will not open
      it. On macOS 13 and 14: right-click Plenipo → Open → Open. On macOS 15 and later: try to
      open it once, then System Settings → Privacy & Security → "Open Anyway". If it says
      "damaged", the app has no valid signature: in Terminal,
      `xattr -dr com.apple.quarantine /Applications/Plenipo.app` lets it open. **What does not work the same:** every new test build looks like a different app to
      macOS, so the Keychain asks again before Plenipo can use its saved keys. Accessibility and
      Screen Recording (computer use, Wave 3B) must be allowed again for each build. Plenipo
      does not update itself (its update list has no Mac entry); each test build is downloaded
      by hand. **So:** the test build is good for checking that Plenipo looks and works right on a Mac
      (the menu bar, the Dock, copy and paste, the browser, the terminal). It is not for daily
      use and not for anyone else. Signing waits for the enrollment.
- [ ] **One download for every Mac** (Apple's chips and Intel), as a `.dmg`.
- [ ] **Fit in on a Mac:** a menu bar icon, clicking the Dock icon brings the window back, the system's
      Edit menu so Cmd+C and Cmd+V work, Cmd and Option in labels, "Show in Finder" (`open -R`; today
      it would fail on a Mac).
- [ ] **The right data folder:** `~/Library/Application Support/com.eightwest.plenipo` (the uninstall
      code looks in the Linux folder on a Mac today). "Delete my Plenipo data" (Wave 2) also
      deletes the Mac's web-page folders (`~/Library/WebKit` and `~/Library/HTTPStorages`); it
      still leaves `~/Library/Preferences/com.eightwest.plenipo.plist` and the saved window state,
      which are not folders named for Plenipo. Add them here.
- [ ] **Computer use on a Mac** is new work: there is no screen or mouse code for the Mac today. macOS
      makes the owner allow "Accessibility" and "Screen Recording" in System Settings. Plenipo
      explains why, opens the right page, and never works around it. Each step is still asked
      (ADR-049, computer use asks every step). A Mac's sharp screen has two pixels for each point,
      so clicks must be scaled, and the allowed keys are checked against the Mac's own shortcuts.
- [ ] **The workers' browser** stays out of the owner's Keychain (Chrome's own switch for that), as it
      already stays out of the Linux password store.
- [ ] **Screen permissions stay with computer use.** A Mac counts the app that started a program as
      responsible for it, so once the owner allows Plenipo Accessibility and Screen Recording, the
      programs a worker runs could use them too, without computer use's questions (ADR-049). Start
      workers' programs so the Mac treats them as their own, or keep those permissions in a small
      helper that only computer use starts. A test proves a worker's program cannot take a
      screenshot. (Found while choosing what AI tools get to see, Wave 1.)
- [ ] **Keychain:** the Vault already uses it. A signed Plenipo keeps access after updates, so the
      signing identity must never change.
- [ ] **Updates:** Plenipo swaps in the new signed app and restarts.
- [ ] **End-to-end tests on a Mac:** Tauri's own test driver does not support Macs. Use WebdriverIO's
      driver built into **test copies only**; a release check proves it is not inside the real app
      (it would let any program on the Mac drive Plenipo). A paid driver (CrabNebula) is the backup.
- [ ] **Sign-in checks on the Mac** (moved from Wave 1): record each AI tool's real answers on a
      Mac, and check that giving a tool its own home folder (Antigravity, Copilot) does not hide the
      Mac's Keychain from it.
- [ ] **The owner's check on the MacBook Pro** (GitHub's Mac machines test the other kind of chip): download from the website, it opens with no warning, sign
      in to Claude Code, run a Development objective end to end, try computer use and see the
      permission steps.
- [ ] **Release** as "Mac (first look)".

### Wave 4 — For everyone

- [ ] **Computer use under Wayland,** through the desktop's own portals (screen sharing and
      remote control): explained first, every step still asked, and working on Ubuntu 26.04 (ADR-154).
- [ ] **Website:** a download for each system (showing the visitor's own first), system requirements,
      and the build script, its tests, and `auto-release.sh` expecting every system's file.
- [ ] **Documents:** README, FAQ, SUPPORT, SECURITY, the privacy notice (where keys are kept on each
      system, and how to remove Plenipo), and `docs/development/setup.md` sections for Mac and Linux
      contributors.
- [ ] **Each AI tool on each system:** install hints for Mac and Linux (Homebrew, curl, npm), not
      `winget`.
- [ ] **Homebrew:** the optional `homebrew-plenipo` repository.
- [ ] **Out of "first look",** with the Phase 23 acceptance report and the roadmap row.

## What the owner needs to do or buy

| What                                                                                                                     | Cost                            | Needed by                  | State                                                               |
| ------------------------------------------------------------------------------------------------------------------------ | ------------------------------- | -------------------------- | ------------------------------------------------------------------- |
| A D-U-N-S number for 8 West Ventures, LLC                                                                                | free                            | —                          | [x] in hand                                                         |
| A Mac to test on                                                                                                         | —                               | Wave 3                     | [x] the owner's MacBook Pro (its chip: Apple menu → About This Mac) |
| The Apple Developer Program, as an organization. The owner enrolls, because it accepts Apple's agreement and pays        | US$99 a year                    | start of Wave 3            | [ ] the owner checks whether 8 West is already enrolled             |
| A Linux PC (a spare PC or old laptop): Ubuntu 24.04 LTS for Wave 2, which has both desktops; Ubuntu 26.04 LTS for Wave 4 | free                            | end of Wave 2              | [ ]                                                                 |
| The Apple signing secrets, typed into GitHub's own Environment page                                                      | —                               | start of Wave 3            | [ ]                                                                 |
| GitHub's Mac and Linux test machines                                                                                     | free (the repository is public) | Wave 0                     | —                                                                   |
| Optional: a paid Mac test driver (CrabNebula)                                                                            | paid                            | only if the free way fails | —                                                                   |

The Apple secrets (the certificate, its password, and an App Store Connect key) go into the GitHub
Environment `release` the same way the Windows signing secrets did: the owner types them into
GitHub's own page, never into a chat (the owner's standing rule).

## The owner's answers (2026-10-02)

| #   | Question                             | Answer                                                                                                                                                   | Record  |
| --- | ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| 1   | One repository or several?           | One                                                                                                                                                      | ADR-151 |
| 2   | When does Phase 23 start?            | Now, from `main`; ADR-132 already puts it next                                                                                                           | ADR-150 |
| 3   | Does Pro wait for the Mac?           | No; Mac and Linux join Pro as each is ready                                                                                                              | ADR-150 |
| 4   | Linux first or Mac first?            | Linux first look, then the Mac right behind                                                                                                              | ADR-152 |
| 5   | Which Linux?                         | Ubuntu 22.04, 24.04, and 26.04 LTS, and Debian 12 or newer; `.deb` and AppImage; `x86_64` (26.04 added by the builder: the recommendation had missed it) | ADR-152 |
| 6   | Which Macs?                          | macOS 13 or newer, Apple's chips and Intel, one download                                                                                                 | ADR-152 |
| 7   | A Linux PC without a password store? | Use the Secret Service; with none, say so and do not save keys                                                                                           | ADR-153 |
| 8   | Computer use under Wayland?          | X11 in Wave 2, Wayland in Wave 4 (chosen after the 26.04 finding)                                                                                        | ADR-154 |
| 9   | The Mac App Store?                   | No                                                                                                                                                       | ADR-152 |
| 10  | "This PC" on screen?                 | Each system's own word                                                                                                                                   | ADR-155 |
| 11  | When the ticket check cannot run?    | Refuse                                                                                                                                                   | ADR-156 |

## Risks to watch

| Risk                                                                                                                                                                                                          | What we do about it                                                                                                                                                                        |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **A careless port quietly weakens Guard.** Programs that outlive a crash, a tool ticket that says "unavailable", a path rule fooled by upper case or a link, a setting passed to a worker that should not be. | Wave 1 is a safety wave with a Guard review at the end. No Mac or Linux release until tool tickets and program trees hold on that system (the plan already says so).                       |
| **The Mac test driver ships by mistake.** It would let any program on the Mac drive Plenipo.                                                                                                                  | It is built into test copies only, and the release job proves it is absent.                                                                                                                |
| **Linux desktops differ** (GNOME or KDE, X11 or Wayland, tray or none, password store or none).                                                                                                               | Promise Ubuntu LTS and Debian; elsewhere Plenipo says plainly what is missing instead of failing silently.                                                                                 |
| **Apple's paperwork is slow, or notarization fails.**                                                                                                                                                         | The D-U-N-S number is in hand; the owner enrolls during Wave 1. Wave 2 does not need it. A dry run proves the notarization steps before the first real release.                            |
| **The Mac's Keychain and permissions are tied to the signature.** Changing it later would make every Mac forget its keys and permissions.                                                                     | One signing identity, 8 West Ventures, LLC, from the first Mac release, never changed.                                                                                                     |
| **The Mac's web view is Safari's engine**, not Edge's, so a screen may look or act differently.                                                                                                               | Linux CI already uses a WebKit engine close to Safari's; the owner's Mac check walks every page once.                                                                                      |
| **The builder cannot run a Mac here** (this PC is Windows).                                                                                                                                                   | Every Mac change is tested on GitHub's Mac machine and checked on the owner's MacBook Pro, so the loop is slower. Linux builds and tests can also run on Coastline, as the host rules say. |
| **Three systems mean more support questions.**                                                                                                                                                                | Diagnostics already names the system; the support page gets a section per system.                                                                                                          |
| **Newer Linux desktops drop X11** (Ubuntu 26.04 already has).                                                                                                                                                 | Wayland computer use in Wave 4; Phase 23 is not delivered without it (ADR-154).                                                                                                            |
