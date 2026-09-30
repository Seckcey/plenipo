# ADR-116: The weekly check's answer is signed, and its time decides the grace

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 11A and 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Adds to:** [ADR-022 (the weekly license check)](ADR-022-subscription-and-license-check.md). What
  Plenipo sends does not change: the key ID and the app version, and nothing else.

## In short

The account service signs every weekly answer with the vault's key, the same way it signs license
keys. Plenipo trusts an answer only if the signature is right. A fake server could otherwise answer
"still paid" forever. The signed answer also carries the service's time, and the 30 days of grace
count from that time, so neither winding the PC's clock back nor replaying an old answer can stretch
them.

## Context

- The plan's contract: the key ID and the app version in, the subscription's state out.
- Blocking the check keeps Pro for 30 days, then it drops. An unsigned answer would let a fake server
  keep Pro on for good, with nothing more than a hosts-file entry.
- ADR-021 rejects obfuscation and anti-tamper tricks. A signature is not a trick: it is the same kind
  of check Plenipo already makes on the key itself.
- The plan's clock rule: grace is measured against the later of the PC's clock and the most recent
  time the service reported.

## Decision

1. **Every answer is signed** by the vault's key ([ADR-104](ADR-104-signing-key-in-aws-kms.md)). It
   carries:
   - the key ID;
   - the state: active; cancelled, ending on a date; ended; or unknown key;
   - the paid-through date;
   - the service's time ("as of");
   - which signing key signed it.
2. **Plenipo checks the signature** against its two public keys, and checks that the key ID is its own.
   An answer that fails is a failed check. Pro stays on and the check tries again later; a bad answer
   never switches Pro off by itself.
3. **Time:**
   - grace ends 30 days after the newest signed "as of" time;
   - "now" is the latest of the PC's clock, the newest signed "as of" time, and the latest clock time
     Plenipo has ever seen;
   - winding the clock back does not extend grace, and replaying an old answer does not reset it;
   - a clock set forward is the owner's problem to explain, as the plan says.
4. **The request does not change.** It is still exactly the key ID and the app version, checked byte
   for byte.
5. **The service may reuse one signed answer per key per day,** to keep signing costs small.
6. **Written in the contract** (`contracts/license-check/`) with signed example answers made with a
   test key.

## Consequences

- Pro can be kept without paying only by changing Plenipo's code, which the Elastic License forbids.
  The code marks the boundary; the license enforces it (ADR-021).
- Signing costs a few cents a month at Plenipo's size.

## Alternatives considered

- **An unsigned answer over HTTPS.** Not chosen: a fake server with its own certificate installed on
  the PC could answer anything, forever.
- **A new key in every answer.** Not chosen for now; see ADR-104.

## As built (v1.18.0)

- **Signing.** Answers are signed with Ed25519 over `plenipo-license-answer.v1.` and the answer's
  contents.
- **What Plenipo keeps.** It keeps the newest signed answer, and checks it again at every start. A
  record changed by hand keeps nothing it cannot prove.
- **Plenipo's "now".** The latest of: the PC's clock, the latest clock time Plenipo has seen, and
  the answer's own time. So winding the clock back never extends the 30 days.
- **What does not count.** An "unknown" answer counts as a failed check, and an older answer than
  the one kept is ignored.
