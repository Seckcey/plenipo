# ADR-168: What Community keeps, where, and for how long

- **Status:** Accepted (2026-10-02): the owner answered question 2 in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md) "as recommended". The rows for the birth
  month and year, the directory, and rewards follow the owner's answers to questions 4 and 9.
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Builds on:** [ADR-118 (customer accounts)](ADR-118-customer-accounts.md), whose **Delete my
  account** already removes personal data at once, and the account privacy notice's "How long we
  keep it"

> **On screen** (ADR-010, plain words and rank names): **Delete my Community data from this PC**,
> and the account site's **Delete my account**. The times below go into the privacy notice in plain
> words.

## In short

**On 8 West's server, Community keeps as little as it can, for as short a time as it can.** A
sealed message is deleted as soon as it is picked up, and after 30 days at the most. Your profile,
blocks, links, and collaborators last only while they are on. Reports are kept 1 year after they are
closed, and a ban as long as it lasts. **On your PC**, everything stays until you delete it, like the
rest of your Plenipo records. Backups on 8 West's server still expire after 35 days.

## Context

- Phase 24's plan: "this phase's ADR decides ... how long anything is kept".
- The account service already keeps nightly encrypted backups for 35 days that cannot be deleted
  early (ADR-103), removes personal data at once when an account is deleted (ADR-118), and keeps
  invoices 7 years for tax.
- Plenipo keeps your records on your PC until you delete them (the privacy statement).
- The website legal statements record forbids promising a fixed time we cannot keep. Every time
  below must be something the service enforces in code, with a test.

## Decision

1. **On 8 West's server (the account service)** (question 2):

   | What                                                                | Kept                                                                                     |
   | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
   | A sealed message, link objective, answer, or invitation             | Until every PC it is for has picked it up, then deleted; **30 days** at the most         |
   | Its envelope (who, who to, when, size, the report tag)              | Deleted with the message. The stamp travels with the message, so the server keeps none   |
   | Your public profile and picture                                     | While it is shown; deleted at once when you hide it or leave Community                   |
   | Your Community name                                                 | While you are in Community; then free for others after **90 days** (so nobody steals it) |
   | Each PC's public Community keys and Community pass                  | While that PC is signed in; deleted at once on sign out or **Remove**                    |
   | Blocks                                                              | While they are on                                                                        |
   | Links, invitations, and collaborators (who, which role, which part) | While they are on; invitations that lapse, at once                                       |
   | Your birth month and year (ADR-162 §4)                              | As long as your account; an answer under 13 is never kept                                |

| Your directory listing (what your business does, where) | While you are listed; deleted at once when you leave the directory |
| Badges and thanks (ADR-169) | While you are in Community |
| Who invited whom, for a free month (ADR-169) | Until the reward is given or refused, then 60 days; the free month itself is an invoice record (7 years, for tax) |
| A report, what it carried, and what 8 West did | **1 year** after it is closed, then deleted |
| A ban (that an email may not rejoin Community) | As long as the ban, kept as a scrambled code of the email, not the email itself |
| Limits' counters (lookups, messages a minute) | Cleared within a day, like the account service's other counters |
| Backups of all the above | **35 days**, encrypted, as today |

2. **Apparent child sexual abuse material** reported to NCMEC is kept only as the law requires and
   then deleted; the attorney confirms the time (ADR-167 §12).
3. **On your PC:** messages, conversations, links, collaborators, and the record of what they did
   stay until you delete them, in your Ledger's backups and exports, like everything else Plenipo
   keeps. **Delete my Community data from this PC** removes them in one step (it asks first). The
   Ledger's events about Community stay with your other Activity, and never hold a message's words.
4. **On a collaborator's PC:** only their own record of what they did. Nothing of your organization
   is saved there (ADR-166 §11).
5. **Delete my account** on the account site also removes everything in item 1 at once, except a
   report's record and a ban, which follow their own times, and backups, which expire after 35 days.
6. **Each time is enforced by the service** (a nightly cleanup, like today's), each with a test that
   a thing past its time is gone. The privacy notice states these times in plain words (draft in
   `docs/legal/phase-24/privacy-changes.md`).

## Consequences

- If the police ask 8 West for someone's messages, 8 West has none it can read, and few envelopes.
- A report can still be proven after the message is gone from the server, because the stamp is
  signed and travels with the message (ADR-164 §6).
- A person who comes back after a ban with the same email is refused, without 8 West keeping their
  email.

## Alternatives considered

- **Keep sealed messages until the person deletes them** (a server mailbox). Handy for a new PC, but
  8 West would hold everyone's sealed conversations for years.
- **Keep reports for 3 years or more.** Helps with repeat offenders, but holds other people's words
  longer. The attorney may ask for longer; that is a small change.
- **No limit on envelopes.** They show who talks to whom; that is personal data, so they go with the
  message.
