# ADR-170: Community follows 8 West's switch — no new release to turn it on

- **Status:** Accepted (2026-10-02). The owner does not want features built and then left switched
  off in a release, and asked that Plenipo follow the account service's own switch. The builder
  recommended this record with three safety catches (§2, §3, §5), and the owner answered "**as
  recommended**".
- **Date:** 2026-10-02
- **Phase:** 24 (Community), part 24C
- **Number:** Phase 24 had no free number left (ADR-161). The owner gave Phase 24 **ADR-170 to
  ADR-179** (2026-10-02, "as recommended"); this is the first of them. Phase 23 keeps ADR-150 to
  ADR-159.
- **Amends:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md) §4 ("Until then a released
  copy shows the Community switch as Coming soon, and connects to nothing") and its part **24F**
  ("Community switched on ... in a release"). Everything else in ADR-161 stands.
- **Builds on:** [ADR-115 (a Free copy never contacts 8 West)](ADR-115-free-never-contacts-8-west.md)
  as amended by [ADR-162 (your account in Plenipo)](ADR-162-your-account-in-plenipo.md) §5

> **On screen** (ADR-010, plain words and rank names): **Coming soon**, **Check again**,
> **Community is closed for now**, **Update Plenipo to use Community**, **Community can't be reached
> right now. Nothing was changed.**

## In short

When Phase 14 put Plenipo on your phone, each release had **Coming soon** built in, and a new
release was needed to switch it on. Community works the other way. **8 West's account service has
its own on/off switch for Community. Plenipo follows it.** While the switch is off, Plenipo shows
**Coming soon**. The moment the owner switches it on, the copies people already have start working,
with no new release.

Plenipo asks only when **you press the Community switch** (or **Check again**), so a copy whose owner
never presses it still never contacts 8 West. And 8 West can turn away an old version of Plenipo, so
a mistake found by the security review can be fixed before Community works for anyone.

## Context

- Phase 14 built **Coming soon** into each release (`PLENIPO_RELAY_LIVE` at build time), and a new
  release switched phone access on (v1.19.4). ADR-161 §4 planned the same for Community.
- **The owner's preference (2026-10-02):** no features built and then left switched off. When the
  owner switches Community on at the account service, the release people already have should work.
- **The account service already has the switch.** Until the owner sets `COMMUNITY_ENABLED`, every
  `/v1/community/*` request answers `503 unavailable`, "Community isn't open yet." (part 24B).
- What today's check of the code and the service found:
  - The contract used `unavailable` for both "not open yet" and "something went wrong". Plenipo could
    not tell them apart, so it might show **Coming soon** when the service is only busy.
  - The service cannot turn away an old version of Plenipo. If following the switch were all there
    is, then a copy with a mistake that the security review finds would start working too, the moment
    Community is switched on.
  - A Free copy never contacts 8 West (ADR-115), except for Community while its owner signs in on
    purpose (ADR-162 §5). A Plenipo that asked "is Community open?" by itself, every day, would break
    that.

## Decision

1. **No switch built into the release.** A release with Community in it is ready to work. The
   account service's switch decides whether it does.
2. **Plenipo asks only when you ask it to** (safety catch 1). Pressing **Community** in Settings →
   Switches, or **Check again**, asks the service "is Community open?" (`GET /v1/community/open`,
   contract §1). That question carries no pass, no body, and nothing about you or this PC: only
   Plenipo's version, as every request does. Plenipo never asks by itself while you are not signed
   in. **A Free copy whose owner never presses the switch never contacts 8 West**, as before (a test
   checks it).
3. **A clear answer** (safety catch 2), new in the contract:
   - **Open:** signing in continues (ADR-162 §2).
   - **`not_open`** (Community isn't open yet): the switch stays off and says **Coming soon**, with
     **Check again**. This PC remembers the answer, so the switch keeps saying **Coming soon** until a
     check finds Community open. Nothing is sent and nothing is kept at 8 West.
   - **`update_needed`:** "**Update Plenipo to use Community**", with the button that installs the
     newest version.
   - **`unavailable`, or no answer at all:** "**Community can't be reached right now. Nothing was
     changed.**" Never **Coming soon**.
4. **If 8 West closes Community again** after you signed in (for example, to fix something), every
   request answers `not_open`. Plenipo stops picking up messages and shows "**Community is closed for
   now**". It keeps your pass, your keys, and everything on your PC, and while your switch is on it
   asks again about once an hour. When Community opens again, it carries on by itself.
5. **8 West can turn away an old version** (safety catch 3). The account service can set the lowest
   version of Plenipo that may use Community. Every request from an older copy answers
   `update_needed`, so a copy with a known mistake cannot use Community, before or after the switch
   is on. Every request already says which version sent it (`User-Agent: Plenipo/<version>`).
6. **One part at a time** ([ADR-171 (switching Community on part by part)](ADR-171-switching-on-part-by-part.md)):
   the answer to "is Community open?" also says whether linked organizations and collaborators are
   open. Their buttons say **Coming soon** until their part opens.
7. **The contract first** (ADR-160 §6): `GET /v1/community/open`, `not_open`, `update_needed`, and
   closed parts are written in `contracts/community/v1` in this repository, as their own reviewed
   change. The account service then copies the new contract and makes its changes, in its own
   session: [the list of changes for the account service](../phases/phase-24-account-service-changes.md).
8. **What still needs a release:**
   - **The attorney's words.** Plenipo's own screens use the words of the drafts in
     `docs/legal/phase-24/`. If the attorney changes a sentence that Plenipo shows (for example, the
     age question or "You'll be listed in the Community directory"), a release with the new words
     comes before the switch is turned on. The terms themselves are not built into Plenipo: it opens
     the published terms page in your web browser and accepts the version the service names (`Me`,
     contract §3), so the attorney's changes to the terms need no release.
   - **GIFs.** Guard must have the GIF library's picture address built in (ADR-164 §4), so GIFs come
     in a release after the owner chooses the library. Until then the **GIF** button is hidden.
   - **A fix the security review asks for**, with the lowest allowed version raised to it (§5).
9. **Tests,** with a stand-in Community service built from the contract's examples (never the live
   service): **Coming soon** while the stand-in is closed, working the moment it opens with nothing
   else changed, `update_needed`, a part that is closed, `unavailable` never shown as **Coming
   soon**, and a Free copy that never presses the switch contacting nobody.

## Consequences

- Launch day is the owner turning a switch on at the account service, not a release.
- Every release with Community in it must be safe to switch on at any time. The lowest allowed
  version (§5) is the brake: before the switch goes on, 8 West sets it to the release the security
  review passed.
- Someone who presses the switch before launch sees **Coming soon**. That one question reaches
  8 West, as it would for any website.
- The account service changes a little (§7). Until it has, the live service answers `unavailable`,
  which Plenipo shows as "can't be reached right now": nobody can use Community either way.

## Alternatives considered

- **Coming soon built into each release, as for the phone** (ADR-161 as written). Simple, but a
  release is needed to switch Community on, and features sit built and switched off, which the owner
  does not want.
- **Plenipo asks by itself, every day.** Coming soon would turn into the switch by itself, but every
  Free copy would contact 8 West, which ADR-115 forbids.
- **Reading the words of the `unavailable` answer** ("isn't open yet" or "went wrong"). No contract
  change, but Plenipo would depend on a sentence that 8 West may reword.
- **Follow the switch with no lowest version.** Simpler, but a copy with a mistake found by the
  review would start working the moment Community is switched on.
