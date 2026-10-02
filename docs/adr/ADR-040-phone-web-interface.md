# ADR-040: Phase 14 is Plenipo's own web interface for a phone; CrewOS leaves the plan

- **Status:** Accepted (by the owner, 2026-09-27); amends ADR-039
- **Amended by:** [ADR-144 (notices on your phone, sealed for it)](ADR-144-notices-on-your-phone.md):
  the PC sends each sealed notice straight to the phone's notice service, not through the relay
  (§5, "Notices when the page is closed"). Everything else in §5 stands.
- **Date:** 2026-09-27
- **Phase:** 14 (recorded after Phase 13, v1.9.0, before Phase 14 starts)

## Context

The rollout plan's Phase 14 was "CrewOS Remote Visibility and Approved Remote Control": CrewOS, a
separate product, would show Plenipo's state from far away and send it approved objectives. Its
dependency was "Installed desktop product stable", which Phase 13 met.

ADR-039 (the owner's notes, the order of work) then added an iPhone and Android app to Phase 14,
as a "remote" beside CrewOS, paired through the owner's 8 West account (Phase 22).

On 2026-09-27, after Phase 13, the owner decided that CrewOS waits until Plenipo itself is done,
and that it is not part of this plan. The owner still wants to reach Plenipo from a phone or
another device, through a web interface built for Plenipo from scratch, and chose the web
interface only: no phone app.

## Decision

1. **CrewOS is removed from the rollout plan**, and the plan no longer names it. No phase builds
   a CrewOS connection, and no other phase depends on it. If the owner wants CrewOS after
   Plenipo is done, that is a new decision with its own record.
2. **Phase 14 becomes "Plenipo on Your Phone: a Web Interface Built From Scratch".** Plenipo
   gets its own web interface, made for a phone's screen first, that lets the owner see what
   Plenipo is doing, answer approvals, and send objectives from another device. It opens in the
   phone's browser; **there is no phone app** (iPhone or Android) in this plan. Phase 14 keeps its
   place in ADR-039's order of work (ninth, after the 8 West account service).
3. **From ADR-039, Phase 14 keeps** pairing a device from the PC (a one-time code, or the 8 West
   account; the phase's ADR chooses), cutting a lost phone off in one step, the phone confirming
   it is the owner before an approval, and approvals the owner keeps on the PC only.
4. **The rules that protected the CrewOS plan carry over unchanged** (ADR-002, local-first):
   - Plenipo on the owner's PC stays in charge. The web interface only shows and asks; the work
     runs on the PC.
   - A request from the web interface travels: web interface → a signed-in connection to
     Plenipo on the PC → Guard → organization/router → the AI tool on the PC.
   - Nothing reaches a shell, the terminal, files, the screen, the browser, or secrets from the
     web interface, and nothing there widens what workers may do or who may connect
     (permissions, switches, Guard's rules, adding a device, turning phone access on).
   - Using Plenipo from another device is off until the owner turns it on, on the PC, and the
     owner can turn it off there at any time.
5. **The owner's answers (2026-09-27):**
   - **The relay:** a phone reaches the PC through the relay 8 West already runs for Milepost (on
     Linode). Plenipo on the PC connects out to it, so nothing is opened on the PC or the router.
     The relay only passes messages along; the phone and the PC encrypt what they say end to end,
     so the relay cannot read the work, answer an approval, or make up a request. Milepost keeps
     working as before, and the relay change is made and approved in the relay's own repository.
     The relay's address and sign-in are kept out of this repository.
   - **Notices when the page is closed:** yes, by web push through the relay. A notice says in
     one short line what needs the owner, encrypted for the owner's phone alone; a choice on the
     phone shows only "Something needs you" on the lock screen. On an iPhone the page is added to
     the Home Screen first (Apple allows web push only then).
   - **Edition:** Pro only (ADR-021). On Free, nothing connects to the relay.
   - **Approve and allow from the phone, at the very least, and as much else as is safe:** the
     owner approves, refuses, and allows both in the web interface and right from a notice (on
     Android the notice has the buttons; on an iPhone, as far as we know today, one tap opens that
     approval). The phone confirms it is the owner first. Everything else that is safe works from
     the phone too: every page to read, sending objectives, stopping work, Stop all, Allow again,
     and Run again. So that a notice can say what it is asking, its short line is encrypted for
     the owner's phone alone; the relay and the push service cannot read it.
     Phase 14's own record settles the rest (how pairing works, how the phone proves it is the
     owner, and how the relay change is tested).

## Consequences

- Phase 14 no longer depends on CrewOS. It needs Plenipo running on the PC (Phase 13 keeps it
  running in the tray and can start it with Windows), the license key (Phase 11A, since it is
  Pro only), the 8 West account (Phase 22) only if pairing goes through the account, and a small
  change to the relay Milepost uses.
- The relay now carries two products. A fault or an attack there must not reach Plenipo's work:
  the end-to-end encryption, the PC connecting out, and Guard deciding every request are what
  keep that true, and Phase 14 tests each of them.
- The web interface is a second screen for Plenipo, so it reuses the design system
  (`packages/ui`, ADR-030) and its plain words; ADR-004's "when a second UI consumer exists"
  applies to it rather than to CrewOS.
- Plenipo gains a network door it did not have. It must be signed in, limited to named devices,
  protected against replayed requests, and recorded in the Ledger, and it is tested for each of
  those before it ships.
- References to CrewOS in ADR-001, ADR-002, and ADR-004 are examples of "a future remote
  surface"; they stand as written history. The architecture overview drops CrewOS from its list
  of integrations.

## Alternatives considered

- **Keep Phase 14 as CrewOS.** The owner wants Plenipo finished first; CrewOS would add a second
  product to build and keep in step before the app itself is done.
- **Drop remote access entirely until Plenipo is done.** The owner wants to reach Plenipo from a
  phone now; a web interface served by Plenipo keeps the work on the PC and needs no other
  product.
- **A phone app (iPhone and Android), with or instead of the web interface.** The owner chose the
  web interface only: it works in any phone's browser, and a phone app needs store accounts,
  reviews, and two more platforms to build and keep up.
