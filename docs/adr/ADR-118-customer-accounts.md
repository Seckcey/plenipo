# ADR-118: Customer accounts — signing in, and deleting an account

- **Status:** Accepted (by the owner, 2026-09-30, as recommended). How long tax records are kept is for
  8 West's accountant and attorney to confirm.
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)

## In short

Customers sign in with a password or with a sign-in link sent by email. The service keeps the least
it can: name, email, an optional company, the plan, the key ID, and dates. Deleting an account removes
the personal data at once and keeps only what the law needs for tax, which is 7 years unless the
accountant says otherwise.

## Context

- The plan asks for sign up, sign in, reset password, and delete my account, plus sign-in links by
  email, with sign-in abuse slowed down.
- The plan's rule: collect the least. Card numbers never touch 8 West; Stripe holds them.
- The weekly check must still answer correctly for a key after its account is deleted.

## Decision

1. **Sign up:** name, email, optional company, and a password. The email is confirmed by a link
   before the account can buy.
2. **Sign in:** a password, or "email me a sign-in link". The link works once, for 15 minutes.
3. **Passwords:**
   - at least 12 characters;
   - checked against known leaked passwords with a range check that never sends the password itself
     (Have I Been Pwned);
   - stored only as an Argon2id hash.
4. **Reset:** a link by email that works once, for 30 minutes. A reset signs out every other session.
5. **Slowing abuse:** limits on sign-in, sign-up, reset, and sign-in links, by email and by internet
   address. It slows attackers down without letting them lock a real customer out.
6. **Delete my account:**
   - stops renewal. Pro stays on to the end of the paid period, which was already paid for;
   - removes the name, email, company, password, sessions, and sign-in history at once;
   - asks Stripe to remove the customer's personal details where Stripe allows. Stripe keeps its own
     payment records as the law requires;
   - keeps, with no personal data: the key ID and its state, so Plenipo still gets a correct weekly
     answer, and invoice records (amount, tax, date, country) for 7 years, for tax;
   - sends a last email to say it is done.

## Consequences

- Little personal data to protect, and little to lose.
- The privacy notice ([ADR-111](ADR-111-refunds-and-terms-of-sale.md)) says what is kept, why, and
  for how long.

## Alternatives considered

- **Sign-in links only, no passwords.** Not chosen: the plan asks for passwords and resets, and some
  customers' email is slow.
- **Delete everything, including invoices.** Not allowed: tax law requires keeping sales records.
