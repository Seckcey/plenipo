# ADR-169: Rewards for taking part — badges, thanks, and a free month for invitations

- **Status:** Proposed (2026-10-02), from the owner's answer to question 9: "**We need to encourage
  community participation. Come up with ways to reward users for community participation.**" Waits
  for the owner's answers to questions 17 (badges and thanks) and 18 (a free month of Pro) in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Number:** the last of Phase 24's numbers (ADR-160).
- **Builds on:** [ADR-163 (your profile, and finding people)](ADR-163-profiles-and-finding-people.md),
  where badges show; [ADR-111 (refunds and the terms of sale)](ADR-111-refunds-and-terms-of-sale.md),
  whose 14-day refund window a free month waits for; [ADR-119 (editions and prices)](ADR-119-editions-and-prices-revised.md)

> **On screen** (ADR-010, plain words and rank names): **Badges**, **Founding member**, **Helper**,
> **Connector**, **Good neighbor**, **Trusted**, **Thanks**, **Thanked by 12 people**, **Getting
> started**, **Invite someone**, **You both get a month of Pro free**, **Your free months: 2 of 12
> this year**.

## In short

Plenipo rewards people for **helping each other**, never for making noise. You earn **badges** on
your profile for real help: being a collaborator, keeping links, being thanked by many people, and
being a good member for a long time. Anyone you helped can press **Thanks**. A **Getting started**
list shows new members what to try. And when someone you invited **buys Pro and keeps it**, **you
both get a month of Pro free**. There are no points, rankings, or streaks, because those reward
sending lots of messages, which is spam.

## Context

- The owner wants people to take part in Community, and wants them rewarded for it.
- A reward that counts messages, posts, or contacts teaches people to spam. A reward has to need
  **someone else's agreement** (they accepted, they thanked you, they paid) or **time**, so one
  person cannot earn it alone.
- The plan keeps public posting, feeds, and a marketplace out of Phase 24. Rewards must not need
  any of them.
- Pro costs $19 a month (ADR-119); buying it needs 18 (the terms of sale). Refunds of a first payment
  are allowed within 14 days (ADR-111).
- The account service already knows the things a reward needs: links and collaborators exist, and
  Stripe knows who paid (ADR-168).
- Members can be 13 to 17 (ADR-162 §4).

## Decision

1. **Badges** (question 17), shown on your profile and your directory card (ADR-163), each with a
   one-line reason when someone points at it. The account service works them out from its own
   records, once a day:

   | Badge               | How you earn it                                                            |
   | ------------------- | -------------------------------------------------------------------------- |
   | **Founding member** | Joined Community in its first 90 days after launch                         |
   | **Helper**          | Has been someone's collaborator for 30 days or more                        |
   | **Connector**       | Has 3 links with other organizations that have each lasted 30 days or more |
   | **Good neighbor**   | Thanked by 10 or more different people                                     |
   | **Trusted**         | In Community for 6 months, with no report against them upheld              |
   - A badge is lost when what earned it is broken: **Trusted** when a report is upheld, and every
     badge when 8 West ends someone's Community (ADR-167 §9).
   - You can hide your badges (Settings → Community → **Show my badges**).

2. **Thanks** (question 17):
   - A **Thanks** button appears where someone really helped you: on a linked organization's answer
     (ADR-165 §5), and on each collaborator in your list (ADR-166).
   - One thanks for each person each 7 days. Only members who have been in Community for 7 days can
     thank. Your profile shows "**Thanked by 12 people**" (different people, not presses).
   - The account service keeps who thanked whom, and when (ADR-168).
3. **Getting started**, on the People page, for you only: **Show your profile**, **Choose whether
   to be in the directory**, **Find someone**, **Send a message**, **Link with an organization**,
   **Invite a helper**. Each is ticked when done. It hides when all are done, or when you close it.
   It earns nothing by itself, so nobody does them just to tick them.
4. **A free month of Pro for invitations** (question 18):
   - **Invite someone** gives you your share link and picture code (ADR-163 §5) and **Invite by
     email**; each carries your invitation, so the account service knows who invited a new member.
   - When a person you invited **buys Pro and keeps it past the 14-day refund window**, **you both
     get one month of Pro free**: one month's price as a credit on your next bill, through Stripe.
   - **At most 12 free months a year** for one person. **Your free months: 2 of 12 this year** shows
     in Settings → Community and on the account site.
   - **Adults only** (buying Pro needs 18). **Not for yourself:** the account service refuses a
     reward when the two accounts share a payment card (Stripe's card fingerprint) or an email
     address.
   - **Taken back** if the purchase is refunded or charged back before the credit is used, or if
     8 West finds the invitation was fake.
   - Each free month costs 8 West one month's price, so each invited person who buys costs about
     **$38** in free months (two months at $19). The owner can change the amount, or stop new
     rewards, from the admin page; rewards already earned stay.
   - The rules are in the Community terms (draft for the attorney: promotions and their taxes).
5. **Never:** points, a ranking of members, streaks, rewards for the number of messages, contacts,
   or link requests sent, or money for anyone under 18.
6. **The Ledger records** `community.badge_earned`, `community.badge_lost`, `community.thanked`,
   `community.thanks_received`, and `community.reward_earned` (the month, never payment details).
7. **New desktop commands are the main window's alone**, with IPC tests that refuse a second window,
   the sign, and a web page: **Thanks**, **Show my badges**, and **Getting started**.

## Consequences

- People who help each other show it, and newcomers can see who has been around and helped.
- Invitations that turn into paying members are paid back in Pro months: 8 West spends about $38 to
  win a member who pays $19 a month.
- The account service keeps a little more (who thanked whom; who invited whom). ADR-168 says for how
  long.

## Alternatives considered

- **Points and a ranking of the most active members.** Easy to make, and they reward volume: the
  people who send the most messages win. That is spam, and a ranking is close to a feed.
- **Rewards for every invitation sent, or every new member.** Rewards made-up accounts.
- **Badges only, no money.** Cheaper; the owner can choose it (question 18).
- **A discount instead of a free month.** Harder to explain than "a month free".
