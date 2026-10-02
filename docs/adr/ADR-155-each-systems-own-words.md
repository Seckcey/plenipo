# ADR-155: Each system's own words on screen

- **Status:** Accepted (by the owner, 2026-10-02, as recommended)
- **Date:** 2026-10-02
- **Phase:** 23
- **Part of:** [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)
- **Amends:** `docs/design/vocabulary.md` (ADR-010, plain words and rank names): the rows that
  require Windows words now point to a table for each system

> **On screen:** this record is about what is on screen. The table is in
> `docs/design/vocabulary.md`, "Words that change with the system".

## In short

Plenipo says "this PC" and "Windows Credential Manager" because it was built for Windows. On a Mac
it will say "this Mac" and "your Mac's Keychain"; on Linux, "this computer" and "your computer's
password store". Each system gets the words its owners already know.

## Context

- About 46 lines of screen text name Windows, 37 say "this PC", and 10 talk about the tray.
- Keyboard labels say Ctrl and Alt only. A Mac's keys are Cmd (⌘), Option (⌥), and Control (⌃).
  The editor's Save already uses Cmd on a Mac; only its label is wrong.
- `docs/design/vocabulary.md` **requires** Windows words in nine places, for example "Windows
  Credential Manager (where it's kept)" and "Start Plenipo with Windows". Every screen follows that
  file, so it changes first.
- Some words come from the Rust side (the password store's name, Start at sign-in), and some from
  the screens (keyboard labels).

## Decision

1. **A table of words for each system** goes into `docs/design/vocabulary.md` in Wave 0, and a
   note above "Say this, not that" sends Mac and Linux readers of the nine Windows-only rows to it. Windows words stay exactly as they are on Windows.
2. **One place decides.** Words that depend on the system come from the Rust side, which knows which
   system it is on; the screens never guess from the browser. Keyboard labels come from one helper
   in the screens, which asks the Rust side once.
3. **Names in the code and in the Ledger stay.** A Ledger event type such as `windowsRestart` keeps
   its name, so old records still read; only the words shown change (ADR-010).
4. **Mac and Linux words for Windows' own pages:** where Plenipo sends the owner to a system setting
   ("Windows Settings → System → Notifications"), each system names its own page.

## Consequences

- Owners read their own system's words; screenshots and help differ a little by system.
- Tests for screen text run for each system's words.

## Alternatives considered

- **"This computer" everywhere.** Simpler, but Windows and Mac owners say "PC" and "Mac".
- **Keep the Windows words.** Wrong on a Mac or Linux: there is no Credential Manager there.
