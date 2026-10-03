# ADR-192: One set of paid AI keys for the whole PC

- **Status:** Accepted (the owner, 2026-10-03: answer 2 to
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), "as recommended": one set of keys for
  the whole PC, with spending caps per organization).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 1 (items 1.3 and 1.4)
- **Amends:** [ADR-094 (more than one organization)](ADR-094-more-than-one-organization.md) §9,
  for paid AI keys only; [ADR-085 (paid AI keys with spending caps)](ADR-085-paid-ai-keys-with-spending-caps.md)
  §5 (where a key is kept).

> **On screen** (ADR-010, plain words and rank names): a key saved on any organization's AI tools
> page works in every organization. Its card's light turns green in each one ("API key
> connected").

## In short

A paid AI key you saved worked only in your **first** organization. Every other organization looked
for it in its own Vault, didn't find it, and showed a yellow "Key not in use" light.

**Accepting this record means:** paid AI keys belong to the **PC**. They are kept once, with the
first organization, and every organization uses them. Each organization keeps its **own** paid-keys
switch and its **own** spending caps.

## Context

The AI tools page is the PC's: it was built on the first organization's services (ADR-094 §4), so
a key saved from any window went into the first organization's settings and Vault. But each
organization's workers read keys from their **own** Guard settings and Vault (ADR-094 §9, each
organization's secrets under its own name). So in a second organization:

- the key was never found, and its paid AI tool stayed "not ready";
- the card showed "Key not in use", though the key was saved and working in the first;
- the second organization's record had no way to hide that key, had it reached it.

## Decision

1. **Paid AI keys are kept with the first organization**, in its Guard settings and its Vault, as
   they already were. No key moves.
2. **Every other organization reads them from there.** When an organization is opened, its broker
   is told where the PC's keys are kept (`Broker::keep_paid_keys_in`). Its paid gate reads the key
   from there, and its secret filter hides those keys from its own record, activity, and results.
3. **Each organization's own switch and caps still decide.** "Let workers use paid AI keys" and
   the spending caps are read from the organization doing the work, before a key is read. Saving a
   key follows the switch of the organization whose window the owner is in.
4. **After a key is saved or removed,** every open organization refreshes its secret filter and
   checks that AI tool again, so its light and its workers follow at once.
5. **Other secrets are unchanged.** Servers, connections, and the secrets workers use stay each
   organization's own (ADR-094 §9).

## Consequences

- A key saved once works everywhere, and the light is green everywhere it works.
- Removing a key stops it everywhere.
- Archiving or deleting a second organization never touches the PC's keys. The first
  organization can't be deleted, so the keys always have a home.
- Every organization's record hides the PC's keys, so a key never shows in a second organization's
  activity.

## Alternatives considered

- **Each organization its own keys**, with the card saying which organization a key is in.
  Rejected by the owner (answer 2): one person, one PC, one set of keys.
- **Copy the key into each organization's Vault.** Rejected: more copies of a secret, and removing
  it would have to find them all.
