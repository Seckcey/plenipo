# ADR-152: Which systems, and in what order — Linux first, then every Mac from macOS 13

- **Status:** Accepted (by the owner, 2026-10-02, as recommended). The recommendation listed Ubuntu
  22.04 and 24.04 LTS but missed **26.04 LTS** (released April 2026); the builder added it the same
  day, since supporting the newest Ubuntu LTS was the point of the answer.
- **Date:** 2026-10-02
- **Phase:** 23
- **Part of:** [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)

> **On screen:** the website's system requirements and download buttons (Wave 4), and the Linux and
> Mac downloads marked "first look" until then.

## In short

Linux comes first, then the Mac right behind it. On Linux, Plenipo supports Ubuntu LTS and Debian
through a `.deb`, and most other Linux through an AppImage. On a Mac, one download works on every
Mac from macOS 13 (Ventura), with Apple's chips or Intel. Plenipo is not sold in the Mac App Store.

## Context

- Linux needs no paperwork and is already tested in CI (ADR-150). The Mac needs the Apple Developer
  Program, and Apple's checks take time.
- Tauri builds a `.deb`, an `.rpm`, and an AppImage for Linux, and a `.dmg` for the Mac. It can build
  one Mac app for both Apple's chips and Intel (`universal-apple-darwin`).
- A Linux app built on an older Ubuntu runs on newer ones; one built on a newer Ubuntu may not run
  on older ones.
- Tauri's own updater can replace an AppImage or a Mac app, but not a `.deb`, which belongs to the
  system's package manager.
- The Mac App Store requires Apple's sandbox, which stops an app from starting other programs. Plenipo
  starts AI tools and approved programs, so it cannot live inside that sandbox.

## Decision

1. **Order:** Linux first look (Wave 2), then Mac first look (Wave 3). If the Apple account is ready
   before Wave 2 is done, the two waves run side by side.
2. **Linux:**
   - **Supported:** Ubuntu 22.04, 24.04, and 26.04 LTS, and Debian 12 or newer, on Intel or AMD
     (`x86_64`) PCs.
   - **Downloads:** a `.deb` for Ubuntu and Debian, and an AppImage for other Linux. Both are built
     on Ubuntu 22.04, so they run on every supported system.
   - **Updates:** the AppImage updates itself, signed with the same updater key as Windows
     (ADR-038). The `.deb` shows **A new version is ready** with a download button.
   - **If asked later:** Fedora's `.rpm`, ARM Linux, and an `apt` list (ADR-151).
   - **Other desktops** may use the AppImage; where a part is missing (no tray, no password store),
     Plenipo says so plainly instead of failing quietly.
3. **Mac:**
   - **Supported:** macOS 13 (Ventura) or newer, with Apple's chips or Intel.
   - **Download:** one `.dmg` for every Mac, signed as **8 West Ventures, LLC** with Apple's
     Developer ID and notarized (Apple's malware check), so it opens without a warning.
   - **Updates:** Plenipo replaces itself with the new signed app.
   - **The signing identity never changes** after the first Mac release: the Mac ties the Keychain
     and the screen permissions to it, and a new one would make every Mac forget them.
4. **Not the Mac App Store.** Downloads come from the website and GitHub releases, and from
   Homebrew in Wave 4 (ADR-151).
5. **Windows does not change:** Windows 11, `x64`, as today.

## Consequences

- Linux owners get Plenipo sooner, and Linux proves the shared base before the Mac depends on it.
- A `.deb` owner updates by downloading the new `.deb` until an `apt` list exists.
- Each release builds four downloads: Windows, Mac, Linux `.deb`, and Linux AppImage.
- Ubuntu 26.04's desktop runs only Wayland, so computer use there waits for Wave 4 (ADR-154).

## Alternatives considered

- **Mac first.** Waits on Apple; see ADR-150.
- **Two Mac downloads** (one for Apple's chips, one for Intel). Owners would have to know which Mac
  they have; one download is simpler and costs only build time.
- **Every Linux at once** (`.rpm`, Arch, Flatpak, Snap). More to test than Plenipo can promise; the
  AppImage covers most of them.
- **The Mac App Store.** Its sandbox would stop Plenipo starting AI tools.
