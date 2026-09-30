# ADR-022: Subscription pricing and the weekly license check

- **Status:** Accepted
- **Date:** 2026-09-27
- **Phase:** 11A (Free and Pro editions and the license key)
- **Supersedes:** decision 5 of [ADR-021](ADR-021-editions-and-license.md) (Free and Pro editions
  under the Elastic License 2.0). Every other decision in ADR-021 stands.

## Context

ADR-021 settled the Elastic License 2.0 and a Free and Pro split, and decided (item 5) that the
license key would be **verified entirely on the owner's own PC**, with no account, no activation
server, and no network call. That followed directly from ADR-002 (local-first architecture), and it
is the right answer for a one-time purchase.

The owner has since chosen subscription pricing: **$9 a month, or $99 a year**. Twelve months of
the monthly plan is $108, so the yearly plan is exactly one month free — $8.25 a month — and is
advertised that way.

A subscription and a purely offline check cannot both be true. If Plenipo never asks anything, it
can never learn that a subscription ended: one payment would buy Pro forever, and the monthly plan
would be a one-time $9 purchase. The ADR-021 decision has to be replaced rather than stretched.

Three ways out were considered, and are recorded in **Alternatives considered** below.

## Decision

1. **Pro installs check in with 8 West at most once every seven days.** The request carries the
   **key id and the app version, and nothing else**. It never carries project or folder names, file
   paths, objectives, task text, worker output, model choices, approvals, or anything from the
   Ledger. The owner's work never leaves their computer — the promise that changes is "Plenipo never
   talks to the internet about your licence", not "your work stays yours".
2. **A Free install never checks in at all.** Someone who has not paid never contacts 8 West, ever.
   The local-first promise of ADR-002 survives intact for every non-paying owner.
3. **Fail-open, in every failure.** No internet, service down, timeout, DNS failure, garbage
   response: Pro stays on and the check retries later. Pro drops in exactly two cases — the service
   explicitly reports the subscription ended, or **30 days** pass with no successful check. An
   8 West outage must never take Pro from a paying customer, and Plenipo must keep working on a
   plane.
4. **Cancelling drops Pro at the end of the paid period**, never at the moment of cancelling.
5. **The key carries a paid-through date**, replacing the update window ADR-021 proposed. The
   update-window model belongs to a perpetual licence and does not survive the move to a
   subscription.
6. **Lapsing stays non-destructive**, exactly as ADR-021 decided. Dropping to Free never deletes,
   hides, or stops existing departments, projects, or history. Only creating something new past a
   Free limit is blocked.
7. **What the check sends is documented publicly** in `docs/editions.md`, and asserted byte for byte
   in a test, so the claim can be checked rather than believed.

## Consequences

- **8 West now runs a service.** A small one, but one with uptime, a domain, a certificate, and a
  failure mode. Fail-open and the 30-day grace are what keep that service's bad day off the
  customer's desk. It is built separately from Phase 11A, which ships against a written contract and
  a local test double.
- **The "no network at all" line has to come out of the README, `docs/editions.md`, and ADR-021's
  summary.** Saying Plenipo never phones home while it phones home weekly would be a lie, and a
  source-available app is one `grep` away from being caught in it. The honest line is: your work
  never leaves your PC; the licence check is the only thing that does, and only if you are paying.
- **Monthly billing costs more to collect.** Card processing takes roughly $0.56 of each $9 —
  about 6.2% — against roughly $3.17 of each $99, about 3.2%. A yearly customer also cannot churn
  for twelve months, and is charged once instead of twelve times. The yearly plan should be the
  default option shown.
- **Selling worldwide brings sales tax obligations** (EU and UK VAT, US state sales tax, and
  others). A payment processor alone does not carry those; a merchant of record does, for a larger
  cut. This is a business decision for the owner and their accountant, outside this ADR and outside
  the app.
- **A determined person can block the check.** A firewall rule or a hosts entry stops it, and the
  source is public. That is accepted: under the Elastic License 2.0, working around the check is a
  breach of licence. The code marks the boundary; the licence enforces it. Obfuscation is still
  rejected, for the reasons in ADR-021.

## Alternatives considered

- **Yearly plan only, no monthly.** Keeps ADR-021 item 5 exactly as written: a key is emailed once a
  year, pasted in once, and verified offline forever after. Rejected because the monthly plan is
  usually where most signups start, and the owner wants that entry point.
- **A new key pasted in every month.** Fully offline with a monthly plan, at the cost of making
  every customer do manual work every 30 days or lose Pro. Rejected: it would generate support mail
  forever and teach people that Plenipo is annoying.
- **Checking on every launch.** Simpler to reason about, but it makes Plenipo useless offline and
  turns every 8 West hiccup into a customer-visible outage. Rejected in favour of weekly with a
  30-day grace.
- **Hardware binding, so one subscription cannot be shared.** Rejected for the same reasons
  ADR-021 rejected it: it produces support tickets from honest customers who changed a motherboard,
  and stops nobody determined.

## As built (v1.18.0, Phase 11A)

- **The check.** A Pro copy checks at most once every 7 days. After a failed check it tries again
  after 1, 3, 6, and 12 hours, then once a day. The request body is exactly
  `{"key_id":"…","app_version":"…"}`, tested byte for byte in this repository and in the account
  service against `contracts/license-check/v1`.
- **The address.** The check goes through Guard's "weekly license check" purpose, to
  `https://account.getplenipo.com/v1/check` and nowhere else. It follows no redirect, keeps no
  cookie, and reads an answer of at most 4 KB.
- **Fail-open.** Every failure keeps Pro on. Pro drops only when 8 West's signed answer says the
  subscription ended, when a cancelled subscription reaches its end date, or when 30 days pass after
  the last signed answer ([ADR-116](ADR-116-the-weekly-answer-is-signed.md)).
- **Settings → License.** It shows Free or Pro and why, the key's ID (never the key), the plan, the
  paid-through date, the last and next check, and **Enter**, **Replace**, **Check now**, and
  **Remove the key**. Removing the key asks first.
- **Limit: the 30 days are kept on the PC.** Deleting the license record starts a new 30 days for
  that key. Removing the key and entering it again does not, and neither does typing it in again or
  a restart. As this record decided, there is no anti-tamper; the licence covers working around the
  check.
