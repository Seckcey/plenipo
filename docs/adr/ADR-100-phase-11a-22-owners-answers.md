# ADR-100: Phase 11A and 22 — what the check found, and the owner's answers

- **Status:** Accepted (by the owner, 2026-09-30: "Everything else is as recommended"). Three
  answers are still open, and their records say **Proposed**: the server's size
  ([ADR-103](ADR-103-account-service-hosting.md) §2), Plenipo's own domain and web address
  ([ADR-105](ADR-105-domain-and-web-address.md)), and the account service's own email through Microsoft 365
  ([ADR-106](ADR-106-account-email.md)).
- **Date:** 2026-09-30
- **Phase:** 11A (Free and Pro editions and the license key) and 22 (the 8 West account service)
- **Number:** Phase 11A and 22 use ADR-100 to ADR-129. This is the first record. ADR-101 to
  ADR-118 each hold one of the owner's answers.
- **Carries out:** [ADR-021 (Free and Pro editions under the Elastic License)](ADR-021-editions-and-license.md),
  [ADR-022 (subscription pricing and the weekly license check)](ADR-022-subscription-and-license-check.md),
  [ADR-039 (the owner's notes)](ADR-039-owners-notes-order-of-work.md) §2.11, §2.13, and §2.14
  (the account service in its own repository; Stripe the standard way; the signing key in a cloud
  key vault), and [ADR-068 (Connections and add-on tools are part of Pro)](ADR-068-connections-are-pro.md)

## In short

Before building, Plenipo's builder compared the plan, [`docs/editions.md`](../editions.md), and the
code, listed where they disagree, and asked the owner 17 questions. The owner accepted every
recommendation and asked three new questions: how big a server the service needs, whether to buy
a domain first, and whether email can go through Microsoft 365 like 8 West's other apps. **Accepting
this record means** building Phase 11A in Plenipo and Phase 22 in a separate private repository,
`plenipo-account`, as the records below say.

## What the check found

The code was read at `6776e0c` (v1.16.0 on `main`, 2026-09-30).

**Pro in `docs/editions.md`, but missing from Phase 11A's list**

1. **One organization on Free** ([ADR-091](ADR-091-phase-21-owners-answers.md) §2). No limit exists
   in the code.
2. **Workers that learn from their work (lessons)**, Pro in ADR-021 and
   [ADR-024](ADR-024-workers-learn-from-work.md). Phase 11A has no deliverable or test for it.
   Learning is on for every copy today.
3. **Plenipo on your phone** (Phase 14). Not built, so there is nothing to lock yet.
4. **Priority support.** Not code.

**The code differs from the plan**

5. **Workers at the same time.** Every copy runs up to 4 AI tool turns at once in each organization
   (`AgentConfig.max_active_turns`). Free's 3 is not enforced, and Pro's "Unlimited" is not true.
   A worker that cannot start waits with no note: Liaison treats "busy" as "try again later".
6. **Business departments.** No business department template exists. Only the Development
   department's setup does. Sales on HubSpot is postponed (Phase 9).
7. **Tools are handed out per step, not per task.** When Pro ends, a task with several steps would
   lose its Connection tools at its next step, not "keep its tools and finish" (ADR-068 §4).
8. **The Vault belongs to organizations.** Every saved secret belongs to one organization (ADR-094).
   Nothing is kept for the whole PC.
9. **A Free copy already makes requests.** It checks GitHub for updates once a day
   ([ADR-038](ADR-038-updates.md)), and npm and GitHub for AI tool versions.

**Words that are out of date**

10. Phase 11A says "all four AI tools" (there are seven), "runs before Phase 11" (Phase 11 shipped in
    v1.6.0), "is restyled in Phase 12A" (delivered), and "the signing key stays offline" (ADR-039
    §2.14 moved it to a cloud key vault).
11. `docs/editions.md` says "Your own sign-ins, never API keys". Phase 16's third wave adds paid AI
    keys ([ADR-085](ADR-085-paid-ai-keys-with-spending-caps.md)), and nothing says whether they are
    Free or Pro.
12. The terms and privacy statements added on 2026-09-30 say the website "has no account, payment,
    or contact form". They also do not mention the weekly license check.

**Stripe and the repository**

13. Stripe was set up in the "8 West IT sandbox", which Coastmark also uses. ADR-039 §2.13 asks for a
    separate Stripe account for Plenipo.
14. Stripe Tax is not on yet. It needs 8 West's head-office address.
15. Stripe's customer portal returns to `plenipo.8westit.com/account`. That address is the static
    website, which has no account pages.
16. The Stripe product's description ("Connections, and everything in the free edition") undersells
    Pro.
17. The new repository was public and named `plenipo-webapp`.

## Decision

The owner's answers, 2026-09-30:

| #   | Question                                  | Answer                                                                         | Record                                               |
| --- | ----------------------------------------- | ------------------------------------------------------------------------------ | ---------------------------------------------------- |
| 1   | The new repository's name                 | `plenipo-account`                                                              | [101](ADR-101-account-service-repository-name.md)    |
| 2   | Public or private                         | Private                                                                        | [102](ADR-102-account-service-repository-private.md) |
| 3   | Hosting, backups, monitoring              | AWS, a server of its own; nightly copies; an outside checker. Size proposed    | [103](ADR-103-account-service-hosting.md)            |
| 4   | The cloud key vault                       | AWS KMS; keys stay Ed25519                                                     | [104](ADR-104-signing-key-in-aws-kms.md)             |
| 5   | The service's web address                 | Proposed: `account.<Plenipo's own domain>`                                     | [105](ADR-105-domain-and-web-address.md)             |
| 6   | Email                                     | Stripe sends every billing email; account email proposed through Microsoft 365 | [106](ADR-106-account-email.md)                      |
| 7   | How 8 West signs in to the admin page     | A password and a passkey, behind Cloudflare Access                             | [107](ADR-107-admin-sign-in.md)                      |
| 8   | Stripe's Managed Payments                 | Not at launch                                                                  | [108](ADR-108-no-managed-payments.md)                |
| 9   | A Stripe account of Plenipo's own         | Yes: "Plenipo by 8 West Ventures, LLC", already made by the owner              | [109](ADR-109-plenipos-own-stripe-account.md)        |
| 10  | Computers per subscription                | One person, on any of their own PCs                                            | [110](ADR-110-one-person-any-of-their-pcs.md)        |
| 11  | Refunds                                   | No partial refunds; a full refund within 14 days of the first payment          | [111](ADR-111-refunds-and-terms-of-sale.md)          |
| 12  | Lessons on Free                           | They pause, like Connections                                                   | [112](ADR-112-lessons-pause-on-free.md)              |
| 13  | Workers at the same time                  | Free: the 4th waits with a note. Pro: 4 at a time in each organization         | [113](ADR-113-workers-at-the-same-time.md)           |
| 14  | What counts as a business department      | One made from a business template                                              | [114](ADR-114-business-departments.md)               |
| 15  | "No outbound request at all" on Free      | A Free copy never contacts 8 West; updates and AI tool versions keep working   | [115](ADR-115-free-never-contacts-8-west.md)         |
| 16  | Sign the weekly answer                    | Yes                                                                            | [116](ADR-116-the-weekly-answer-is-signed.md)        |
| 17  | Paid AI keys                              | Free                                                                           | [117](ADR-117-paid-ai-keys-are-free.md)              |
| —   | Customer sign-in, and deleting an account | A password or an emailed sign-in link; tax records kept 7 years                | [118](ADR-118-customer-accounts.md)                  |

The builder's other defaults, accepted with the rest, are in the record they belong to: the
service's language and checks (ADR-101), the license key's contents (ADR-104), one license for
every organization on a PC (ADR-110), and where the terms of sale live (ADR-111).

## Consequences

- Phase 11A is built against the written contract and a local test double, as the plan says. Only
  the check's address depends on the domain, and it is kept in one place, so building starts now.
  The domain has to be settled before Phase 11A's pull request merges (ADR-105).
- Phase 22's code lives only in `plenipo-account`. The one thing both sides share is the weekly
  check's contract, written once in this repository and tested on both sides (ADR-101).
- These records keep out anything an attacker could use, such as server addresses and the admin
  page's address. Those live in the private repository.
- Some jobs are the owner's: renaming the repository and making it private (the builder's
  permissions blocked both), connecting Stripe's connector to Plenipo's own Stripe account, and the
  AWS, DNS, and Microsoft 365 setup the other records list.

## Alternatives considered

- **One record with every answer, as ADR-091 did for Phase 21.** Not chosen: the owner asked for one
  record per decision.
