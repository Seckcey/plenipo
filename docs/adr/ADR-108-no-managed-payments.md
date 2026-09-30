# ADR-108: No Stripe Managed Payments at launch; Stripe Tax as planned

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Answers:** the plan's "Stripe's Managed Payments ... is an option to weigh in this phase's ADR"

## In short

Stripe's Managed Payments would make Stripe the seller of record. Stripe would then handle sales tax
in more than 80 countries, fraud, disputes, and payment questions, for an extra fee on every sale.
Plenipo does not use it at launch. 8 West sells Plenipo itself, and Stripe Tax works out and collects
the tax, as the plan says.

## Context

From Stripe's documentation, checked 2026-09-30:

- It adds a fee to every sale, on top of Stripe's normal fee. Stripe's pricing page gives the rate.
- Customers' card statements would say `LINK.COM* 8WEST PLENIPO PRO`.
- Stripe sends the receipts and invoices, and may refund a customer if 8 West does not answer a
  question within 48 hours.
- It only works through Checkout or Payment Links. It does not cover invoices 8 West makes by hand
  for businesses.
- It applies only to new subscriptions. Existing subscribers cannot be moved to it.

Most Plenipo buyers are expected to be businesses. For sales to businesses in other countries, the
buyer usually handles the tax (reverse charge). For sales to individuals in the EU and UK, VAT is due
from the first sale.

## Decision

1. **No Managed Payments at launch.** 8 West is the seller, with its own name on statements and
   receipts ([ADR-106](ADR-106-account-email.md)).
2. **Stripe Tax** works out and collects sales tax on Checkout and on invoices. It watches the
   thresholds until 8 West registers anywhere.
3. **Before launch, the owner asks 8 West's accountant** about EU and UK VAT on sales to individuals.
   If registering there is too much work, turn on Managed Payments before launch, while there are no
   subscribers to leave behind.

## Consequences

- Lower fees, and 8 West's own name on everything.
- 8 West handles disputes and tax registrations itself.
- Stripe Tax needs 8 West's head-office address in Plenipo's Stripe account before it can work, even
  in test mode.

## Alternatives considered

- **Managed Payments from the start.** Simplest for taxes abroad, at a cost on every sale, with Link's
  name on statements and no hand-made invoices. Kept as the fallback in decision 3.
