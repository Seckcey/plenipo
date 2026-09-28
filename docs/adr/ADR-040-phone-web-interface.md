# ADR-040: Phase 14 is Plenipo's own web interface for a phone; CrewOS leaves the plan

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 14 (recorded after Phase 13, v1.9.0, before Phase 14 starts)

## Context

The rollout plan's Phase 14 was "CrewOS Remote Visibility and Approved Remote Control": CrewOS, a
separate product, would show Plenipo's state from far away and send it approved objectives. Its
dependency was "Installed desktop product stable", which Phase 13 met.

On 2026-09-27, after Phase 13, the owner decided that CrewOS waits until Plenipo itself is done,
and that it is not part of this plan. The owner still wants to reach Plenipo from a phone or
another device, through a web interface built for Plenipo from scratch.

## Decision

1. **CrewOS is removed from the rollout plan**, and the plan no longer names it. No phase builds
   a CrewOS connection, and no other phase depends on it. If the owner wants CrewOS after
   Plenipo is done, that is a new decision with its own record.
2. **Phase 14 becomes "Plenipo on Your Phone: a Web Interface Built From Scratch".** Plenipo
   gets its own web interface, made for a phone's screen first, that lets the owner see what
   Plenipo is doing, answer approvals, and send objectives from another device. It opens in the
   phone's browser; **there is no phone app** (iPhone or Android) in this plan.
3. **The rules that protected the CrewOS plan carry over unchanged** (ADR-002, local-first):
   - Plenipo on the owner's PC stays in charge. The web interface only shows and asks; the work
     runs on the PC.
   - A request from the web interface travels: web interface → a signed-in connection to
     Plenipo on the PC → Guard → organization/router → the AI tool on the PC.
   - Nothing reaches a shell, the terminal, files, the screen, the browser, secrets, or
     Settings from the web interface.
   - Using Plenipo from another device is off until the owner turns it on, on the PC, and the
     owner can turn it off there at any time.
4. **How a phone reaches the PC is decided at the start of Phase 14**, in that phase's own
   record, with the owner. The choices to weigh include: the same Wi-Fi only; a private network
   the owner already uses (for example Tailscale); or a relay that 8 West runs. The same record
   decides whether a phone can be notified while the page is closed (that needs an outside push
   service), and which edition (Free or Pro) includes it.

## Consequences

- Phase 14 no longer depends on another product being ready. It needs only Plenipo, running on
  the PC (Phase 13 keeps it running in the tray and can start it with Windows).
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
