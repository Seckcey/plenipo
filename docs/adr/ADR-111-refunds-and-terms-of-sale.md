# ADR-111: Refunds, and where the terms of sale and the account privacy notice live

- **Status:** Accepted (by the owner, 2026-09-30, as recommended). The wording is drafted for an
  attorney's review before launch.
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)

## In short

Customers can cancel any time and keep Pro to the end of the period they paid for. There are no
refunds for part of a period. Anyone who asks within 14 days of their first payment gets a full
refund. The terms of sale and the account's privacy notice are drafted in the account service's
repository. Plenipo's website privacy page gains a paragraph about the weekly check.

## Context

- Stripe's customer portal is already set to cancel at the end of the paid period, with no prorated
  refund.
- The terms and privacy statements added on 2026-09-30 (`apps/website/legal`) describe the app and the
  website. They say the website has no account or payment form, and they do not mention the weekly
  license check.
- The plan: "privacy policy and terms of sale, drafted for an attorney's review".

## Decision

1. **Cancelling:** any time, in the customer portal. Pro stays on to the end of the paid period, then
   Plenipo drops to Free with nothing lost ([ADR-022](ADR-022-subscription-and-license-check.md)).
2. **Refunds:**
   - no refunds for part of a period;
   - a full refund of a subscription's first payment if the customer asks within 14 days of it;
   - a refunded first payment ends the subscription, and the weekly check then reports it ended;
   - 8 West may refund more on its own judgment, from the admin page
     ([ADR-107](ADR-107-admin-sign-in.md)).
3. **The terms of sale** (price, renewal, cancelling, refunds, one person on any of their own PCs
   ([ADR-110](ADR-110-one-person-any-of-their-pcs.md)), and a link to the Elastic License for
   software rights) are drafted in the account service's repository, and shown on the account site.
4. **The account privacy notice** (what the account service keeps, why, for how long, and who
   processes it: Stripe, AWS, Microsoft 365, and Cloudflare) is drafted there too.
5. **Plenipo's website privacy page** (`apps/website/legal/privacy.md`) gains a short paragraph: a Pro
   copy checks in once a week, sending only its key ID and app version, with a link to the account
   privacy notice.
6. **Stripe links to both:** Checkout and the customer portal get the terms and privacy addresses
   before launch.
7. **Everything is marked "draft for attorney review"** until an attorney has read it. Nothing invents
   a governing law, an arbitration clause, or a certification; that follows the rule in the website
   legal statements record ([website legal statements](website-legal-statements.md)).

## Consequences

- One refund rule, easy to explain, and generous enough to remove the risk of trying Pro.
- The owner has an attorney review the terms of sale and the privacy notice before live mode.

## Alternatives considered

- **No refunds at all.** Not chosen: the Free edition is the trial, but a first-payment refund builds
  trust at almost no cost.
- **Refunds for part of a period.** Not chosen: more work for small sums.
