# ADR-153: Saved keys on Linux live in the system's password store, never in one that forgets them

- **Status:** Accepted (by the owner, 2026-10-02, as recommended)
- **Date:** 2026-10-02
- **Phase:** 23
- **Part of:** [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md)

> **On screen** (ADR-155): on Linux, keys are kept in **your computer's password store (GNOME
> Keyring or KWallet)**. Where there is none: "This computer has no password store, so Plenipo can't
> save keys here. Install GNOME Keyring or KWallet, then try again."

## In short

On Linux today, Plenipo's Vault forgets every saved key each time the computer restarts. Plenipo
will keep keys in the password store Linux desktops already have. If a computer has none, Plenipo
says so and does not save the key. It never keeps keys in a file of its own.

## Context

- The Vault (ADR-013 §12) keeps server sign-ins, Connection sign-ins, paid AI keys, and the license
  key. It uses the `keyring` library: Windows Credential Manager on Windows, the Keychain on a Mac,
  and on Linux the **kernel keyring** (`linux-native`, `crates/capabilities/Cargo.toml`).
- The kernel keyring is kept in memory. The library's own notes say a restart clears it, and its
  "persistent" keyring also expires after a few days. So on Linux, every key would vanish at each
  restart. No customer has met this, because no Linux download has shipped.
- Linux desktops keep passwords in a **Secret Service**: GNOME Keyring on Ubuntu's desktop, KWallet
  on KDE. The `keyring` library can use it.
- Only the Vault's in-memory test store is tested today; the real stores are not.

## Decision

1. **On Linux, the Vault keeps keys in the Secret Service**, so they survive a restart. The library
   may keep a copy in the kernel keyring while the owner is signed in, but the Secret Service is
   where a key lives. The builder picks the `keyring` setup whose tests prove a key survives a
   restart.
2. **No password store, or the owner refuses to unlock it:** Plenipo says so in plain words, does
   not save the key, and anything that needs that key waits. It never falls back to the kernel
   keyring alone, a plain file, or a password-protected file of its own.
3. **Tests use the real store** on each system in CI where it can run (a Secret Service in the
   Linux test session, the Keychain on GitHub's Mac), and the owner's Linux check in Wave 2 restarts
   the computer and finds every key still there.
4. **The Mac** keeps using the Keychain. The signed app keeps access after updates because its
   signing identity never changes (ADR-152).
5. **Windows does not change.**

## Consequences

- Keys on Linux are as safe and as lasting as on Windows and the Mac.
- A Linux server or minimal desktop without a password store cannot save keys; Plenipo says why.
- The Linux test setup grows: it must start a Secret Service for the tests.

## Alternatives considered

- **Keep the kernel keyring.** Every key is lost at restart.
- **A password-protected file of Plenipo's own.** One more password for the owner, and one more
  piece of security code to get right, when the desktop already has a store made for this.
- **A plain file when no store exists.** Never: anyone who can read the file has every key.
