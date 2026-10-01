# ADR-119: Editions and prices, revised — Pro at $19, Partner plans by organizations

- **Status:** Accepted (by the owner, 2026-09-30)
- **Date:** 2026-09-30
- **Phase:** 11A and 22, before the first sale
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Amends:** [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md) and
  [ADR-022 (subscription pricing and the weekly check)](ADR-022-subscription-and-license-check.md):
  the prices, and the number of organizations Pro includes; and
  [ADR-110 (one person, any of their PCs)](ADR-110-one-person-any-of-their-pcs.md): a license now
  says how many organizations it covers

## In short

Pro costs $19 a month and covers up to 3 organizations. Companies that run Plenipo for clients
(MSPs, agencies, IT firms) buy a Partner plan, priced by how many organizations it covers. The
number of organizations is written inside the signed license key and counted on the PC, so nothing
new is sent to 8 West. Free does not change.

## Context

- Research on pricing (kept in 8 West's private records, 2026-09-30) found that $9 a month sits at
  the bottom of the market. Close rivals charge $15–$60 per person, and business tiers for similar
  tools charge $19–$40 per person.
- One $9 license covered unlimited organizations. So an IT company running 20 client organizations
  paid the same as a hobbyist, and the customers who get the most value paid the least.
- Organizations are what grows with an MSP's or agency's business: one per client. Plenipo already
  counts them on the PC, and Free already stops at one.
- Nothing has been sold yet, so the long-term price can be set now. Raising a price by more than
  half later is hard.

## Decision

| Edition               | Who it is for                            | Price                        | Organizations  |
| --------------------- | ---------------------------------------- | ---------------------------- | -------------- |
| **Free**              | Anyone, one business                     | $0                           | 1              |
| **Pro**               | One owner running their own business     | $19 a month, or $190 a year  | Up to 3        |
| **Partner 10**        | MSPs, agencies, IT firms, per technician | $79 a month                  | Up to 10       |
| **Partner 25**        | Same                                     | $149 a month                 | Up to 25       |
| **Partner Unlimited** | Same                                     | $299 a month                 | No limit       |
| **Business** (later)  | Companies with many staff                | About $39 per person a month | Set when built |

1. **Free does not change:** 1 organization, 1 department, 1 project, and 3 workers at a time, with
   every safety feature and every AI tool.
2. **Pro and every Partner plan include all of Pro:**
   - unlimited departments and projects;
   - 4 workers at a time in each organization;
   - Connections, add-on tools, and lessons.

   Partner plans differ from Pro only in how many organizations they cover.

3. **The license key says how many organizations it covers.** Plenipo counts organizations that are
   not archived, on the PC, against that number. An organization past the number waits, kept, as on
   Free. Nothing new is sent to 8 West: the weekly check still sends only the key's ID and the
   version (ADR-022, ADR-115).
4. **One license is still for one person, on any of their PCs** (ADR-110). A Partner company buys one
   Partner plan per technician.
5. **Yearly prices give 2 months free.**
6. **Partners also get:**
   - a free copy for their own internal use;
   - 20% of revenue for referrals, for one year.

   Both are set up by hand at first.

7. **Business comes later.** It is sold only once its fleet tools exist (a silent installer, locked
   company settings, central license management, invoices, faster support). When it does, the terms
   of sale say "Pro and Partner are for businesses up to 25 people", kept by honesty, not tracking.
8. **Not chosen for now:**
   - reselling AI usage (8 West pays for no AI use);
   - charging per result;
   - in-app marketplaces;
   - in-app affiliate links (they would need tracking).

## Consequences

- **Before 1.18.0 ships:**
  - the license key gains the edition and the number of organizations it covers;
  - the account service sells the new plans by their lookup keys;
  - `docs/editions.md` and the release notes show the new prices.
- **Stripe:** prices are found by lookup keys, never Stripe's IDs (ADR-109), so the new prices are new
  lookup keys:
  - `plenipo_pro_monthly` and `plenipo_pro_yearly`, at the new amounts;
  - one per Partner plan.
- **Plan switches:** a customer who moves between Pro and a Partner plan gets a new key with the new
  allowance.
- **Before selling widely, after the first buyers:** the risks in the AI providers' terms that the
  research listed. The owner chose to deal with them once Plenipo has its first buyers.

## Alternatives considered

- **Keep $9 with unlimited organizations.** Not chosen: the biggest users would pay the least, and a
  later change would mean new keys for every customer.
- **A price per client organization, reported to 8 West.** Not chosen: it would send counts to 8 West,
  against ADR-115 (a Free copy never contacts 8 West) and the privacy promise.
- **A rule by company size only** (Docker's). Kept for when Business exists.
