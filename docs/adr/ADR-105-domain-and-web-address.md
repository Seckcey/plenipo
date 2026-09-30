# ADR-105: Plenipo's own domain is `getplenipo.com`, and the account service answers at `account.getplenipo.com`

- **Status:** Accepted (by the owner, 2026-09-30: "I bought the domain getplenipo.com on
  godaddy.com")
- **Date:** 2026-09-30
- **Phase:** 11A and 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)

## In short

Plenipo's own domain is `getplenipo.com`. It is registered at GoDaddy, and its DNS is at Cloudflare.
The account service answers at `account.getplenipo.com`. That includes the weekly check at
`https://account.getplenipo.com/v1/check`, which is built into every copy of Plenipo and never
changes. Email about accounts comes from the same domain.

## Context

- The owner, 2026-09-30: "I'm going to buy a top level domain like plenipo.com. Should I get that
  first before we start this phase?"
- `plenipo.com` has been registered since 2016 by someone else. It is parked at GoDaddy.
- The builder recommended `plenipo.app`. The owner bought `getplenipo.com` at GoDaddy instead. It was
  registered on 2026-09-30 and is paid through 2027-09-30.
- Why the address matters:
  - The check's address is built into every copy. If it changes later, old copies keep calling the
    old one. If the old one ever stops answering, those customers lose Pro 30 days later
    ([ADR-022](ADR-022-subscription-and-license-check.md)).
  - An email domain's good name with mail providers builds up over time
    ([ADR-106](ADR-106-account-email.md)).
  - Stripe's customer portal and Checkout link back to it.

## Decision

1. **`getplenipo.com`**, registered at GoDaddy, set to renew by itself every year, and locked against
   transfer.
2. **Its DNS is at Cloudflare**, like `8westit.com`. GoDaddy's nameservers are switched to
   Cloudflare's, and DNSSEC is turned on at Cloudflare, with its record added at GoDaddy.
3. **Addresses:**
   - `account.getplenipo.com`: sign up, sign in, the account page, buying, Stripe's notices, and the
     admin page (behind Cloudflare Access, [ADR-107](ADR-107-admin-sign-in.md));
   - `https://account.getplenipo.com/v1/check`: the weekly check, fixed forever once released;
   - account email from `getplenipo.com`, through 8 West's Microsoft 365, and Stripe's billing email
     from the same domain (ADR-106);
   - Stripe's customer portal returns to `https://account.getplenipo.com/account`;
   - test copies on Coastline answer at their own test address, never at the production one.
4. **The website** stays at `plenipo.8westit.com` for now. Its "Buy Pro" buttons link to
   `account.getplenipo.com`. Moving the website to `getplenipo.com` is a separate change.

## Consequences

- Plenipo has its own name, apart from 8 West IT's.
- Phase 11A builds the check's address as `https://account.getplenipo.com/v1/check`.
- The domain must be renewed every year, forever. Automatic renewal is on.

## Alternatives considered

- **`plenipo.app`** (the builder's recommendation). Not chosen by the owner.
- **Buying `plenipo.com` from its owner.** Unknown price, and it could take weeks.
- **Everything at `plenipo.8westit.com`.** It works, and 8 West already owns it. Not chosen: the owner
  wants Plenipo's own name.
