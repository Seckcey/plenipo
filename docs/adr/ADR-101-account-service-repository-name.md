# ADR-101: The account service's repository is `plenipo-account`, with its own rules

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Carries out:** [ADR-039 (the owner's notes)](ADR-039-owners-notes-order-of-work.md) §2.11 (the
  8 West account service is its own product, in its own repository)

## In short

The 8 West account service (Phase 22) lives in its own repository, `Seckcey/plenipo-account`. It
never lives in Plenipo's repository. Its first name, `plenipo-webapp`, is easy to mix up with
Phase 14's web interface for your phone.

## Context

- The owner made `Seckcey/plenipo-webapp` for Phase 22. The plan says the owner names it.
- Phase 14 (Plenipo on your phone) is "a web interface built from scratch". A repository called
  "webapp" sounds like that phase, not like the account service.
- The owner's direction: "Never put Phase 22's code in the app's repository." The two sides talk only
  through a written contract.

## Decision

1. **The name is `plenipo-account`.** GitHub forwards the old name to the new one.
2. **Its own rules, like Milepost.** It has its own `CLAUDE.md`, its own checks, and its own records
   for how it is built, numbered inside that repository. Decisions that change Plenipo or the
   contract stay in Plenipo's repository (ADR-100 to ADR-129).
3. **One shared thing: the weekly check's contract.** It is written once, in Plenipo's repository
   under `contracts/license-check/`, as a schema and example requests and answers. Both sides test
   against it. The account service pins the contract version it was tested with.
4. **How it is built:** TypeScript on Node.js (the current long-term-support release), Stripe's
   official library (for checking Stripe's signature on its notices, and a fixed Stripe API
   version), and PostgreSQL. Its checks run on Linux, where the service runs.
5. **The website's "Buy Pro" buttons are website content**, in Plenipo's repository
   (`apps/website`). They are links to the account service, not service code.

## Consequences

- Nobody can confuse the account service with the phone's web interface.
- A change to the contract needs a change in both repositories. A test on each side catches a
  mismatch.
- The service's code gets its own review before launch (plan, Phase 22).

## Alternatives considered

- **Keep `plenipo-webapp`.** Not chosen, because of the mix-up with Phase 14.
- **A folder in Plenipo's repository.** Rejected by the owner.
- **Rust, so both sides share code.** Not chosen. Stripe has no official Rust library, and the
  contract is shared as files that both sides test against, which works in any language.
