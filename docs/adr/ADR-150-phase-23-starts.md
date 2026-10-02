# ADR-150: Phase 23 starts — its numbers, what the check found, its five waves, and the owner's answers

- **Status:** Accepted (by the owner, 2026-10-02: all eleven questions "as recommended"). The
  builder then found that its list of Linux systems left out Ubuntu 26.04 LTS (April 2026), whose
  desktop runs only Wayland; it added 26.04 (ADR-152), and the owner chose to build Wayland computer
  use in Wave 4 (ADR-154). The owner already has a D-U-N-S number for 8 West Ventures, LLC and a
  MacBook Pro to test on.
- **Date:** 2026-10-02
- **Phase:** 23 (Mac and Linux)
- **Carries out:** [ADR-132 (the final push)](ADR-132-the-final-push.md) §1: Phase 23 is second,
  after Phase 14, which is delivered (v1.19.0 to v1.19.3).
- **Number:** on 2026-10-02, `main` and every branch on GitHub stopped at ADR-149 (Phase 14 used
  ADR-140 to ADR-149). So that no two sessions pick the same number, **Phase 23 uses ADR-150 to
  ADR-159**, starting with this one.

> **On screen** (ADR-010, plain words and rank names): nothing yet. The words for each system are
> set by ADR-155 and go into `docs/design/vocabulary.md` in Wave 0.

## In short

Phase 23 makes Plenipo run on a Mac and on a Linux PC, with the same safety as on Windows. Before
building, Plenipo's builder read the plan, the records it points at, and the code, and wrote one
record for each big choice. **Accepting this record means** Phase 23 uses ADR-150 to ADR-159, is
built in **five waves**, and starts with the records below:

| Record | What it settles                                                                         |
| ------ | --------------------------------------------------------------------------------------- |
| 151    | One repository for Windows, Mac, and Linux                                              |
| 152    | Which systems, and in what order: Linux first, then every Mac from macOS 13             |
| 153    | Saved keys on Linux live in the system's password store, never in one that forgets them |
| 154    | Computer use on Linux: X11 in Wave 2, Wayland in Wave 4                                 |
| 155    | Each system's own words on screen                                                       |
| 156    | When Plenipo cannot tell which program sent a tool call, it refuses (changes ADR-034)   |

The full list of what must change, part by part, is in
[the Phase 23 checklist](../phases/phase-23-checklist.md).

## Context

What the check found (the code first read at v1.18.1, then again at `42e9b42` on `main`, v1.19.3,
2026-10-02; Phase 14 added almost no system-specific code):

1. **One code base.** The screens (React), Guard, the Ledger, the workforce, routing, the Liaison,
   licensing, and the phone's line are the same on every system. **162 lines in 53 Rust files**
   choose by system.
2. **Linux is built and tested on every change.** CI runs clippy, `cargo test --workspace`, and the
   end-to-end tests that drive the real app through `tauri-driver`, all on Ubuntu.
3. **The Mac has never been built.** No CI job, no Mac installer settings, no Mac signing.
4. **Releases are Windows only.** One Windows job builds and signs the installer (ADR-052) and
   writes a `latest.json` that lists only Windows. The update check always asks for
   `windows-x86_64`, and installing an update says "Updates are installed on Windows only."
5. **The Vault forgets every key on Linux at each restart.** It uses the kernel keyring, which is
   kept in memory only (ADR-153).
6. **On a Mac, every tool call would be served unchecked.** ADR-034 serves a connection when the
   system "offers no way to tell", and the Mac has no lookup yet (ADR-156).
7. **A worker's programs outlive a crash on Mac and Linux.** A normal stop ends the whole process
   group, but if Plenipo itself crashes, its programs keep running; closing a terminal only signals
   its shell. On Windows a job object ends them all.
8. **Two more Guard gaps on Mac and Linux:** Plenipo never checks whether it runs as root, and
   Guard's program names ignore upper and lower case, which is wrong on Linux.
9. **Computer use:** nothing for the Mac; X11 only on Linux (ADR-154).
10. **What people see:** about 46 lines of screen text say Windows, 37 say "this PC", keyboard
    labels say Ctrl and Alt only, and `docs/design/vocabulary.md` requires Windows words in nine
    places (ADR-155). The website offers only the Windows download.
11. **The Mac's end-to-end tests need another driver:** `tauri-driver` does not support Macs.
12. **Licensing is the same everywhere.** A Pro license already covers any of a person's computers
    (ADR-110).

## Decision

1. **Numbers.** Phase 23 uses ADR-150 to ADR-159.
2. **Five waves**, each one or more pull requests:

   | Wave  | What                                                                                                                                                     | Release                                             |
   | ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
   | **0** | Get ready: these records, the word table for each system, a Mac job in CI, and unsigned trial builds for Mac and Linux                                   | none (documents and CI)                             |
   | **1** | The shared base Mac and Linux both need, with every Guard gap above closed, ending with a Guard safety review                                            | a Windows release as usual; Windows does not change |
   | **2** | Linux, first look: `.deb` and AppImage, updates, tray, browser, computer use under X11, "Delete my Plenipo data", the owner's check                      | the first Linux download, marked "first look"       |
   | **3** | Mac, first look: signed and notarized as 8 West Ventures, LLC, one download for every Mac, computer use with Apple's permission steps, the owner's check | the first Mac download, marked "first look"         |
   | **4** | For everyone: Wayland computer use, the website's downloads, documents, Homebrew, every AI tool checked on each system                                   | Phase 23 delivered                                  |

   Waves 2 and 3 may run side by side once Wave 1 is merged and the Apple account is ready.

3. **When:** now, from `main` (the owner, 2026-10-02), as ADR-132 already orders.
4. **Pro does not wait for the Mac.** Phase 22 is live; Mac and Linux join Pro as each is ready.
5. **The plan's rules hold on every system:** every request goes through Guard; nothing loads code
   at run time (ADR-014); new desktop commands are the main window's alone; and **no Mac or Linux
   download ships until tool tickets and program trees hold on that system.**
6. **The owner's part:** join the Apple Developer Program as an organization with the D-U-N-S
   number (the owner does this, because it accepts Apple's agreement and pays US$99 a year), test on
   the MacBook Pro in Wave 3, and test on a Linux PC at the end of Wave 2. Signing secrets are typed
   only into GitHub's own Environment page (ADR-052), never into a chat.
7. **Order of work.** Phase 23 shows as **In progress** until Wave 4 is merged; Phase 24 (Community)
   comes after, as ADR-132 says.

## Consequences

- The Linux first look can ship before Apple's paperwork is done, and proves the shared base.
- Wave 1 changes nothing Windows owners see; the Windows tests prove it.
- The builder's computer runs Windows, so every Mac change is tested on GitHub's Mac machines and on
  the owner's MacBook Pro; that loop is slower than for Windows or Linux.
- Every release from Wave 2 on builds for three systems, so releases take longer.

## Alternatives considered

- **One pull request for the whole phase.** One huge, slow review; nothing to try until the end.
- **Mac first.** It waits on Apple's paperwork; Linux proves the same shared base sooner and cheaper.
- **A repository for each system.** Not chosen: ADR-151.
