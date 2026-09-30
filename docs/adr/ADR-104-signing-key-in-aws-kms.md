# ADR-104: The license signing key lives in AWS KMS, and license keys stay Ed25519

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 11A and 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Carries out:** [ADR-039 (the owner's notes)](ADR-039-owners-notes-order-of-work.md) §2.14 (the
  signing key in a cloud key vault; the key format follows what the vault can sign)
- **Amends:** `ROLLOUT_PLAN.md`, Phase 11A: "the signing key belongs to 8 West, stays offline" now
  means "stays in the vault", as ADR-039 §2.14 said

## In short

Since November 2025, AWS's key vault (KMS) can sign with Ed25519. So license keys stay Ed25519, as
Phase 11A was written. The private key never leaves the vault. Only the account service may ask it to
sign, and every signature is logged. Plenipo carries two public keys: the one in use and a spare.

## Context

- ADR-039 §2.14 says: use Ed25519 if the vault can sign with it, otherwise P-256, and decide before
  Phase 11A is built.
- Checked on 2026-09-30:
  - AWS KMS signs Ed25519 (key type `ECC_NIST_EDWARDS25519`, signing with `ED25519_SHA_512`);
  - Google Cloud KMS signs Ed25519 (`EC_SIGN_ED25519`);
  - Azure Key Vault does not (its curves are P-256, P-256K, P-384, and P-521).
- The service runs on AWS ([ADR-103](ADR-103-account-service-hosting.md)).

## Decision

1. **AWS KMS**, in the same AWS account and region as the server.
2. **Ed25519.** Keys are signed with `ED25519_SHA_512` over the plain message. Plenipo checks them
   with a standard Ed25519 library, with no network.
3. **Two production keys from the start: the one in use and a spare.** Plenipo carries both public
   keys. If the key in use is ever at risk:
   - the service switches to the spare;
   - every active customer gets a new key by email and on their account page;
   - a later Plenipo update adds a new spare and stops trusting the old key.
4. **Only the account service may sign.**
   - The server's own AWS identity (an instance role) is the only thing allowed to ask for a
     production signature. No person's AWS sign-in can.
   - Nobody can copy the private key out; KMS does not allow it.
   - AWS CloudTrail logs every signature.
   - Deleting a key means waiting 30 days, KMS's longest wait.
5. **A separate test key for test copies.**
   - Plenipo's test builds trust it; release builds never do.
   - Test copies on Coastline reach it with an AWS user that may only sign with that one test key.
     That user's access keys live only in Coastline's service settings file.
6. **What a license key carries:** a format version, the edition (Pro), the holder, the key ID, the
   plan (monthly or yearly), the paid-through date, the date it was issued, and which signing key
   signed it. The holder is the buyer's name or company as they typed it, never their email. The
   exact layout is in the contract (`contracts/license-check/`, ADR-101).

## Consequences

- KMS costs about $1 a month for each key (two production keys and one test key), plus a few cents a
  month for signatures at Plenipo's size.
- The builder never needs the production vault to build Phase 11A. The contract's examples are signed
  with a throwaway key made only for tests.
- Replacing a key is an ordinary release, not an emergency rebuild.

## Alternatives considered

- **P-256.** Not needed, because the vault signs Ed25519.
- **Azure Key Vault.** No Ed25519.
- **A key on a disconnected machine.** Rejected in ADR-039 §2.14: keys must be issued when a payment
  clears, at any hour.
- **A HashiCorp Vault that 8 West runs itself.** Not chosen: one more service to run and guard.
- **Sending a replacement key inside the weekly answer**, so customers never paste a new one. Not
  chosen for now: it changes the contract, and replacing the key should be rare.
