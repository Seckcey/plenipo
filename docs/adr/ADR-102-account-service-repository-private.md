# ADR-102: The account service's repository is private

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)

## In short

Plenipo itself is source-available ([ADR-021](ADR-021-editions-and-license.md), the Elastic License),
so anyone can read what runs on their own PC. The account service is different. It runs on 8 West's
server and handles payments. Nobody needs to read its code to trust Plenipo, and publishing it would
help attackers more than customers. So its repository is private, like Milepost's.

## Context

- The repository was made public, and it was still empty on 2026-09-30.
- Milepost, 8 West IT's other subscription product, keeps its repository private.
- What customers need to know about what Plenipo sends is already public. It is in `docs/editions.md`,
  the check's contract (ADR-101), and a test that checks the request byte for byte.

## Decision

1. **Private.** Only 8 West can see it.
2. **Treat every commit as if the repository were public** (Milepost's rule). No secrets, ever: no
   Stripe keys, no notice-signing secret, no vault access, no passwords.
3. **What customers rely on stays public:** the contract, what the check sends, the privacy notice,
   and the terms of sale.

## Consequences

- On private repositories, GitHub's checks use the account's monthly allowance of minutes. Checks
  that run on Linux keep that small.
- Protection rules for branches on a private repository may need a paid GitHub plan.
- The owner changes the setting on GitHub (**Settings → General → Danger Zone → Change repository
  visibility**). The builder's permissions blocked it.

## Alternatives considered

- **Public, like Plenipo.** Not chosen. It gives customers nothing they need, and it shows attackers
  how the payment side works.
