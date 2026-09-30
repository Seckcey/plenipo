# ADR-109: Plenipo has its own Stripe account

- **Status:** Accepted (by the owner, 2026-09-30: "I already created the Stripe property under
  8 West IT. It's called Plenipo by 8 West Ventures, LLC. It has already been set up with the Pro
  version product.")
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Carries out:** [ADR-039 (the owner's notes)](ADR-039-owners-notes-order-of-work.md) §2.13 (a
  separate Stripe account for Plenipo keeps its books apart from 8 West's other businesses)
- **Amends:** `ROLLOUT_PLAN.md`, Phase 22, "Stripe, set up so far": it was set up in the 8 West IT
  sandbox; it now lives in Plenipo's own account

## In short

Plenipo sells through its own Stripe account, "Plenipo by 8 West Ventures, LLC", under 8 West IT's
Stripe organization. Building and testing use that account's test mode. The copy in the 8 West IT
sandbox, which Coastmark shares, is no longer used.

## Context

- The product, prices, and customer portal were first set up in the "8 West IT sandbox". Coastmark
  also sends its payment notices there.
- The owner has made Plenipo's own account, with the Pro product.
- After the owner reconnected Stripe's connector (2026-09-30), the builder could see two Plenipo
  accounts in test mode: "Plenipo by 8 West Ventures, LLC" and "Plenipo by 8 West Ventures, LLC
  sandbox". Neither had a product, a price, or a customer portal in test mode yet, and Stripe Tax was
  waiting for the head-office address. The owner's Pro product is most likely in live mode, which
  the connector does not show.

## Decision

1. **Plenipo's own account, "Plenipo by 8 West Ventures, LLC"**, for live mode at launch. **Its
   sandbox** is where the service is built and tested.
2. **The code finds prices by their lookup keys,** `plenipo_pro_monthly` and `plenipo_pro_yearly`,
   never by Stripe's IDs, so the same code works in test and live mode.
3. **The builder makes the same setup in the sandbox,** as the plan's "Stripe, set up so far" lists,
   and checks it:
   - both lookup keys, with tax added on top;
   - the product's tax category (`txcd_10202003`, for the accountant to confirm);
   - the card statement name `8WEST PLENIPO PRO`;
   - the customer portal: switching between monthly and yearly, cancelling at the end of the paid
     period, and its return address ([ADR-105](ADR-105-domain-and-web-address.md));
   - Stripe Tax.
4. **The service uses a restricted Stripe key** that can do only what the service needs. That key
   and the notice-signing secret go in the server's settings, never in a repository and never in the
   chat.
5. **The service accepts only Plenipo's notices.** A notice about any other product is recorded and
   ignored.

## Consequences

- Plenipo's books are apart from 8 West IT's other businesses, and Coastmark's notices never reach
  Plenipo's service.
- The owner enters 8 West's head-office address in the Plenipo account's tax settings, in the sandbox
  and in live mode, so Stripe Tax can work.
- Before launch, the owner checks that the live product matches the sandbox: the lookup keys, the tax
  category, the statement name, and the customer portal.
- The product's description should list everything Pro adds, not only Connections.

## Alternatives considered

- **Keep building in the 8 West IT sandbox.** Not chosen: Plenipo's own account now exists, and
  testing in it matches what goes live.
