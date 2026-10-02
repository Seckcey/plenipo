# ADR-146: Where the phone's page lives — its own address, never on the relay

- **Status:** Accepted (by the owner, 2026-10-01: "as recommended but can we make the address be
  remote.getplenipo.com instead of phone.getplenipo.com?"). The page's address is
  **`remote.getplenipo.com`**; everything else is as recommended. **Amended by
  [ADR-148](ADR-148-the-phone-page-on-its-own-server.md)** (2026-10-02, the phone's page on its own
  server): the page is served from a small AWS server of its own, not from Coastline (§2 and §6).
- **Date:** 2026-10-01
- **Phase:** 14
- **Part of:** [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md)
- **Keeps:** [ADR-040 (Phase 14 is Plenipo's own web interface for a phone)](ADR-040-phone-web-interface.md)
  §5, "The relay's address and sign-in are kept out of this repository"

> **On screen** (ADR-010, plain words and rank names): the address `remote.getplenipo.com`, shown in
> the picture code and on Settings → Devices.

## In short

Your phone opens Plenipo's page at its own address, **`remote.getplenipo.com`**. The page's files are
built from this repository with each release, and served by **the same machine that serves Plenipo's
website**, updated the same way. **The relay never serves the page**: if it did, a relay that went bad
could change the page and trick you. Plenipo reaches the relay by a name of its own,
**`relay.getplenipo.com`**, which you point at Milepost's relay in Cloudflare, so the relay's real
address never goes in this repository.

## Context

- A web page's code can read everything the page shows and does. **Whoever serves the page is part
  of what you trust**, the same way you trust Plenipo's installer. The sealed line (ADR-143) keeps the
  relay out, but only if the relay cannot change the page.
- A passkey and a notice sign-up belong to one address for good (ADR-142, ADR-144). Changing the
  address later means pairing every phone again.
- Plenipo's website runs on Coastline behind a Cloudflare Tunnel, and updates itself from each
  GitHub release, with no login held by GitHub (ADR-069, ADR-130). Plenipo's domain is
  `getplenipo.com`, with its DNS at Cloudflare (ADR-105). The account service is at
  `account.getplenipo.com`.
- The relay's address and sign-in must never be in this public repository (ADR-040).
- Separate branding work is under way on the website (`codex/branding`); Phase 14 does not touch the
  website's files.

## Decision

1. **The page's address: `remote.getplenipo.com`** (the owner's choice). Its own address, apart from the
   website, so the page shares nothing with it: not its storage, its passkeys, its notice sign-ups,
   or any script.
2. **Its files are built from this repository**, in a new app, `apps/remote` (React, with the design
   system in `packages/ui`), by the release's own checks, and **served next to the website on
   Coastline**, through the same Cloudflare Tunnel, updated from each release the same way as the
   website (recommended). _Amended by ADR-148: served from a small AWS server of its own, with its
   own Tunnel; updated from each release the same way._ A new piece of the website's updater serves them; the website's own files
   do not change.
3. **The page loads nothing from anywhere else.** No outside scripts, fonts, or counters. Its rules
   (a Content Security Policy) allow only its own files, and only the relay for connections, and no
   code made at run time. The PC never sends the page code.
4. **The relay's name: `relay.getplenipo.com`** (recommended). In Cloudflare, the owner points it at
   Milepost's relay. Only this name is in Plenipo. The relay's real address, and anything Milepost
   uses to sign in, stay in the relay's own repository and in Cloudflare.
5. **Test copies** use their own addresses on the PC (`localhost`) and the stand-in relay. Only
   copies built for the tests can point at them, as the license check's stand-in works (ADR-115 §6).
6. **What the owner sets up** (part 14A, before the first release that turns phone access on): the
   two names in Cloudflare, and the page's piece of the website's updater on Coastline. The steps are
   written in the phase's checklist.

## Consequences

- A relay that goes bad cannot change the page, so it cannot trick you into approving something.
- Coastline, which already serves the website, now also serves the phone's page, so its safety
  matters to phone access too.
- The page's address is fixed once phones are paired. Choosing it now matters.

## Alternatives considered

- **The relay serves the page.** One machine fewer, but then the relay could change the code that
  holds your phone's keys and shows you what you approve. Rejected: it breaks ADR-040's promise.
- **`getplenipo.com/phone`, on the website's own address.** The page would share storage and
  passkeys with the website and its scripts. Not recommended.
- **Cloudflare Pages** for the page's files. It works, and needs no machine, but GitHub would hold a
  Cloudflare key to publish, which the website's updater was built to avoid (ADR-069).
- **The account service's server.** It handles payments; it should do nothing else (ADR-103).
- **Putting the relay's real address in Plenipo.** Not allowed (ADR-040).
