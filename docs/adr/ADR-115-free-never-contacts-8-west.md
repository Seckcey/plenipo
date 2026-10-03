# ADR-115: A Free copy never contacts 8 West

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Amended by:** [ADR-162 (your 8 West account in Plenipo)](ADR-162-your-account-in-plenipo.md) §5
  (2026-10-02): a Free copy also contacts 8 West, for Community only, while its owner is signed in
  to Community on purpose. A Free copy that never signs in still never contacts 8 West.
- **Date:** 2026-09-30
- **Phase:** 11A
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Keeps:** [ADR-022 (the weekly license check)](ADR-022-subscription-and-license-check.md) §2 ("a
  Free install never checks in at all") and [ADR-038 (updates)](ADR-038-updates.md)
- **Amends:** `ROLLOUT_PLAN.md`, Phase 11A's test "a Free install makes no outbound request at all",
  which now reads as the promise below

## In short

The promise is that a Free copy never contacts 8 West. Plenipo's license code makes no request of
any kind unless a key has been entered. The things every copy already does keep working: the daily
check for a new Plenipo on GitHub, the check for new AI tool versions, the Connections the owner sets
up, and the AI tools' own traffic.

## Context

- Phase 11A's test list says "a Free install makes no outbound request at all".
- Every copy already asks GitHub once a day whether a newer Plenipo is out (ADR-038), and asks npm and
  GitHub for the newest AI tool versions. `docs/editions.md` says the update check is separate from
  the license and the same for Free and Pro.
- Every request Plenipo makes by itself already goes through Guard's list of allowed addresses, each
  for a named purpose.

## Decision

1. **The promise:** a Free copy never contacts 8 West. It is written that way in `docs/editions.md`,
   the plan, and the privacy page.
2. **No key, no check-in.** The license code does not even build its network client until a key has
   been entered.
3. **Guard gains one purpose, "the weekly license check".** It allows only the check's address, and
   only when a key is present.
4. **Nothing else changes.** Updates (ADR-038), AI tool versions, Connections, and the AI tools' own
   traffic are unaffected.
5. **Tested three ways:**
   - with no key, the license code makes no request;
   - Guard refuses the license purpose for any other address, and for any request without a key;
   - a real-app test runs a Free copy, with the check pointed at a local stand-in service, through a
     whole Development objective, and the stand-in receives nothing.
6. **Only test builds can point the check at a stand-in service.** Release builds always use the
   address built into them ([ADR-105](ADR-105-domain-and-web-address.md)).

## Consequences

- The promise is exact, and a test proves it.
- An owner who blocks GitHub still gets no update notices, as before.

## Alternatives considered

- **No outbound requests at all on Free.** Not chosen: it would switch off update notices and the AI
  tool version check for every Free owner, against ADR-038.

## As built (v1.18.0)

- **No key, no request.** With no key, Plenipo never builds a check. Its regular look and **Check
  now** send nothing.
- **Guard.** The weekly license check's purpose allows only the check's one address.
- **Test copies.** Only a copy built with the test key trusted can point the check at a stand-in on
  the PC (`PLENIPO_LICENSE_STAND_IN` at build time).
- **Tested three ways.**
  - A unit test runs a Free copy for weeks of clock time, and its stand-in receives nothing.
  - Guard's test refuses every other address.
  - Two real-app tests: the Development flow runs start to finish on Free while a stand-in listens,
    and the license test checks Free, then a key, then Pro. In both, a Free copy sends nothing.
