# ADR-110: One subscription is for one person, on any of their own PCs

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 11A and 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Keeps:** [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md) and
  [ADR-022 (the weekly license check)](ADR-022-subscription-and-license-check.md): no hardware
  binding, and no machine fingerprinting

## In short

One Pro subscription is for one person. They may use it on any of their own PCs, such as a desktop
and a laptop. A team buys one subscription for each person. Plenipo never counts machines. On each
PC, one license covers every organization, and Free's limits count across the whole PC.

## Context

- ADR-021 and ADR-022 rule out hardware binding: it makes support tickets for honest customers and
  stops nobody who is determined.
- The weekly check sends only the key ID and the app version, so the service cannot count PCs, and
  must not try.
- Since Phase 21, one PC can hold many organizations
  ([ADR-094](ADR-094-more-than-one-organization.md)). Each keeps its own secrets in the Vault under
  its own name, and nothing is kept for the whole PC.

## Decision

1. **One person, any of their own PCs.** The terms of sale say so
   ([ADR-111](ADR-111-refunds-and-terms-of-sale.md)). A team buys one per person. `docs/editions.md`
   already offers team licences and invoices on request.
2. **No counting.** Plenipo and the service never count or identify machines.
3. **One license for the whole PC.** It covers every organization on that PC. It is kept in the Vault
   under the first organization's name, because that organization holds the PC's shared record and
   can never be archived or deleted (ADR-094). Uninstalling removes it along with the rest.
4. **Free's limits count across the whole PC.** One organization, one department, one project, and
   three workers at once, counted over every organization together, so a copy cannot step around a
   limit by adding organizations.

## Consequences

- Nothing to set up on a second PC except entering the same key.
- Sharing a key with another person breaks the terms, not the code. The Elastic License and the terms
  of sale are the boundary (ADR-021).
- When Pro ends with several organizations, all of them stay and still open. Only creating something
  new past a Free limit is blocked (ADR-091 §2).

## Alternatives considered

- **One PC per subscription.** It needs machine counting, which ADR-021 and ADR-022 rejected.
- **A license for each organization.** Not chosen: more keys for the same person, for no gain.

## As built (v1.18.0)

- **Where the license is kept.** The key is in the Vault as `plenipo-license-key`, under the first
  organization's name. Its record (`license.json`, not a secret) is in Plenipo's data folder.
  "Delete my Plenipo data" at uninstall removes the key too.
- **How Free's limits count.** Organizations that are not archived; departments and projects that are
  neither archived nor deleted, in every open organization; and the workers on the job (running, or
  waiting for an approval) in every organization.
- **Organizations on Free.** A new organization, a copy of one, and bringing back an archived one
  are all part of Pro. An archived organization waits, kept.
