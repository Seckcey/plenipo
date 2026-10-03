# ADR-173: Leave this conversation deletes it from this PC

- **Status:** Accepted (2026-10-03), by the builder, applying the owner's answer to question 5
  ("**as recommended**": **Delete for me** on this PC only,
  [ADR-172 (what part 24C changes)](ADR-172-what-part-24c-changes.md) §2) to the same limit. The
  owner can overrule it.
- **Date:** 2026-10-03
- **Phase:** 24 (Community), part 24C
- **Amends:** [ADR-167 (block, report, and leave)](ADR-167-block-report-leave.md) §14

> **On screen** (ADR-010, plain words and rank names): **Leave this conversation** ("Leave this
> conversation? It is deleted from this PC, and Pat's new messages won't be delivered. Your other
> PCs keep their copies.").

## In short

**Leave this conversation** deletes the conversation from **this PC**, and 8 West stops delivering
the other person's new messages to all your PCs. Your other PCs keep the messages they already
have, as with **Delete for me**.

## Context

- ADR-167 §14 says **Leave this conversation** "deletes it from your PCs and refuses new messages".
- Refusing new messages works for every PC: 8 West does it (contract §5).
- Deleting what is already on your other PCs does not: the contract has no way for one of your PCs
  to tell your others (contract §6), which is why **Delete for me** deletes from this PC only
  (ADR-172 §2).

## Decision

1. **Leave this conversation** tells 8 West (contract §5), then deletes the conversation's messages
   from this PC. Its question says so.
2. This PC still knows you left, so the conversation says "**You left this conversation. Writing
   again opens it on your side.**"
3. Deleting from every one of your PCs can come with the same later version of the contract as
   **Delete for me** on every PC (ADR-172, alternatives).

## Consequences

- Someone with two PCs who leaves a conversation on one still sees its old messages on the other,
  and no new ones arrive on either.
- A message deleted by leaving can no longer be reported from this PC. **Report** is offered before
  leaving.

## Alternatives considered

- **Keep the messages on this PC too** until **Delete my Community data from this PC**. Simpler, but
  ADR-167 §14 asks for the conversation to go.
- **A new kind of sealed note between one member's own PCs,** with a contract and account-service
  change. The same work ADR-172 put off for **Delete for me**.
