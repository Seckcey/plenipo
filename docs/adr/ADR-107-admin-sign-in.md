# ADR-107: 8 West signs in to the admin page with a password and a passkey, behind Cloudflare Access

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)

## In short

The admin page, where 8 West sees customers, subscriptions, keys, and refunds, has two walls. The
first is Cloudflare Access, with 8 West's Microsoft 365 sign-in. The second is the service's own
sign-in: a password plus a passkey (Windows Hello or a USB security key). A passkey cannot be phished;
a six-digit code can.

## Context

- The plan: "admin pages need 8 West's sign-in with a second factor".
- A trick website can capture a code from an authenticator app. A passkey only works on the real
  site.
- 8 West already signs in to Microsoft 365, which has its own second step. The service sits behind
  Cloudflare ([ADR-103](ADR-103-account-service-hosting.md)).

## Decision

1. **Admin accounts are made only from the server's command line.** There is no admin sign-up page.
2. **The service's own sign-in:**
   - a password and a passkey;
   - one-time recovery codes, printed and kept offline, for a lost passkey;
   - a session lasts at most 8 hours, and ends after 30 idle minutes.
3. **Cloudflare Access in front of the admin page,** using 8 West's Microsoft 365 sign-in. The service
   also checks Cloudflare's signed pass on every admin request, so a request that went around
   Cloudflare is refused.
4. **Every admin action is recorded:** who did it, what, and when. That covers refunds, a key
   switched off, and a customer's data looked up.
5. **Customers never reach the admin page.** A customer account cannot become an admin.

## Consequences

- Two sign-ins for 8 West, each hard to steal.
- The owner registers a passkey and prints the recovery codes on the first sign-in.
- Cloudflare Access is free for small teams.

## Alternatives considered

- **Password and authenticator-app code.** Not chosen: codes can be phished.
- **Only Cloudflare Access.** Not chosen: one wall, and the service could not tell which admin did
  what.
