# ADR-105: Plenipo's own domain, and where the account service answers

- **Status:** **Proposed** (2026-09-30). The owner plans to buy a domain for Plenipo. This record
  holds the recommendation until the owner has one.
- **Date:** 2026-09-30
- **Phase:** 11A and 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)

## In short

`plenipo.com` belongs to someone else. Buy another short name: `plenipo.app` is recommended. Do it
before Phase 11A merges. The account service then answers at `account.<domain>`. That includes the
weekly check at `https://account.<domain>/v1/check`, which is built into every copy of Plenipo and
never changes.

## Context

- The owner, 2026-09-30: "I'm going to buy a top level domain like plenipo.com. Should I get that
  first before we start this phase?"
- `plenipo.com` has been registered since 2016-09-11, at GoDaddy, and is paid through 2027-09-11. It
  shows a parked page (checked 2026-09-30 in the public registry). Its owner might sell it, at an
  unknown price.
- These names had no DNS on 2026-09-30, so they are probably free (the registrar confirms):
  `plenipo.app`, `plenipo.ai`, `plenipo.io`, `plenipo.dev`, `getplenipo.com`, `useplenipo.com`.
- Why the address matters:
  - The check's address is built into every copy. If it changes later, old copies keep calling the
    old one. If the old one ever stops answering, those customers lose Pro 30 days later
    ([ADR-022](ADR-022-subscription-and-license-check.md)).
  - An email domain's good name with mail providers builds up over time
    ([ADR-106](ADR-106-account-email.md)).
  - Stripe's customer portal and Checkout link back to it.

## Decision (proposed)

1. **Buy `plenipo.app`.** It is short, says what Plenipo is, and costs about $15 a year. Browsers
   always use HTTPS for `.app` names. `plenipo.ai` is the pricier choice. `plenipo.com` only if its
   owner sells it cheaply.
2. **Keep its DNS at Cloudflare**, like `8westit.com`.
3. **Addresses:**
   - `account.<domain>`: sign up, sign in, the account page, buying, Stripe's notices, and the admin
     page (behind Cloudflare Access, [ADR-107](ADR-107-admin-sign-in.md));
   - `https://account.<domain>/v1/check`: the weekly check, fixed forever once released;
   - email from the same domain (ADR-106);
   - Stripe's customer portal returns to `https://account.<domain>/account`, changed in Plenipo's
     Stripe account;
   - the website stays at `plenipo.8westit.com` for now. Its "Buy Pro" buttons link to
     `account.<domain>`. Moving the website is a separate change.
4. **When:** building starts now. Plenipo keeps the check's address in one place. The domain must be
   settled before Phase 11A's pull request merges.

## Consequences

- Plenipo gets its own name, apart from 8 West IT's.
- The domain must be renewed every year, forever. Turn on automatic renewal.

## Alternatives considered

- **Everything at `plenipo.8westit.com`.** It works, and 8 West already owns it. Not chosen: the owner
  wants Plenipo's own name.
- **Only the check at an `8westit.com` address.** A fallback if buying a domain is delayed.
