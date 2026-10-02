# ADR-169: Rewards for taking part — points, a leaderboard, badges, thanks, and a free month for invitations

- **Status:** Accepted, with the owner's change (2026-10-02). From the owner's answer to question 9:
  "**We need to encourage community participation. Come up with ways to reward users for community
  participation.**" Then question 17 (badges and thanks): "**Definitely need points and a
  leaderboard**", so points (§1) and a leaderboard (§2) are added beside badges and thanks; and
  question 18 (a free month of Pro): "as recommended". The questions are in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Number:** the last of Phase 24's numbers (ADR-160).
- **Builds on:** [ADR-163 (your profile, and finding people)](ADR-163-profiles-and-finding-people.md),
  where points and badges show; [ADR-111 (refunds and the terms of sale)](ADR-111-refunds-and-terms-of-sale.md),
  whose 14-day refund window a free month waits for; [ADR-119 (editions and prices)](ADR-119-editions-and-prices-revised.md)

> **On screen** (ADR-010, plain words and rank names): **Points**, **Leaderboard**, **This week**,
> **All time**, **How to earn points**, **Top helper this week**, **Badges**, **Founding member**,
> **Helper**, **Connector**, **Good neighbor**, **Trusted**, **Thanks**, **Thanked by 12 people**,
> **Getting started**, **Invite someone**, **You both get a month of Pro free**, **Your free months:
> 2 of 12 this year**.

## In short

Plenipo rewards people for **helping each other**. You earn **points** when other people agree you
helped: they accept your message, link with you, take you on as a helper, thank you, or join because
you invited them. The **leaderboard** shows who earned the most points this week and of all time.
You also earn **badges**, anyone you helped can press **Thanks**, and a **Getting started** list
shows new members what to try. When someone you invited **buys Pro and keeps it**, **you both get a
month of Pro free**. Sending lots of messages earns nothing, so spamming can never win.

## Context

- The owner wants people to take part in Community, rewarded with points and a leaderboard.
- A reward that counts messages, posts, or contacts teaches people to spam, and a leaderboard makes
  that worse: people compete for it. So every point must need **someone else's agreement** (they
  accepted, they thanked you, they paid) or **time**, and no one person can give another more than a
  few.
- The plan keeps public posting, feeds, and a marketplace out of Phase 24. A leaderboard is a list of
  people and their points; nobody posts in it.
- Pro costs $19 a month (ADR-119); buying it needs 18 (the terms of sale). Refunds of a first payment
  are allowed within 14 days (ADR-111).
- The account service already knows what points need: accepted requests, links and collaborators,
  thanks, and who paid (ADR-168).
- Members can be 13 to 17 (ADR-162 §4); they are never in the directory.

## Decision

1. **Points** (question 17), worked out by the account service from its own records, never sent by a
   PC:

   | You earn                                                        | Points |
   | --------------------------------------------------------------- | ------ |
   | Someone accepts your message request (once for each person)     | 2      |
   | Someone thanks you (§4)                                         | 3      |
   | Someone you invited joins Community                             | 5      |
   | A link with another organization lasts 7 days                   | 10     |
   | You help someone as a collaborator for 7 days, or they help you | 10     |
   | You finish **Getting started** (§5) (once)                      | 10     |
   | Each month in Community with no report against you upheld       | 5      |
   | Someone you invited buys Pro and keeps it past 14 days          | 25     |
   - **Nothing for** messages sent, requests sent, invitations sent, links asked for, or time spent
     in Plenipo.
   - **Limits:** at most 100 points a week, and at most 20 points a month from any one other person,
     so two friends cannot feed each other.
   - Points have **no money value**, cannot be moved, and are taken away when what earned them is
     undone (a link ended within 7 days, a refund) or when 8 West finds cheating (ADR-167 §9).
   - **How to earn points** on the People page lists the table above.
   - The numbers are a starting point; 8 West can change them from the admin page. Points already
     earned stay.

2. **The leaderboard** (question 17), on the People page:
   - **This week** (from Monday, Pacific time) and **All time**, the top 50 each, with each person's
     picture, name, Community name, badges, and points, and **Message**.
   - Your own place shows even when you are not in the top 50.
   - **Left out:** members under 18 (they see their own points only), and anyone who chooses **Appear
     offline** (ADR-163 §5).
   - Only people signed in to Community can see it.
   - The top member each week gets the **Top helper this week** badge for the next week.
3. **Badges,** shown on your profile, your directory card, and the leaderboard, each with a one-line
   reason when someone points at it, worked out by the account service once a day:

   | Badge                    | How you earn it                                                            |
   | ------------------------ | -------------------------------------------------------------------------- |
   | **Founding member**      | Joined Community in its first 90 days after launch                         |
   | **Helper**               | Has been someone's collaborator for 30 days or more                        |
   | **Connector**            | Has 3 links with other organizations that have each lasted 30 days or more |
   | **Good neighbor**        | Thanked by 10 or more different people                                     |
   | **Trusted**              | In Community for 6 months, with no report against them upheld              |
   | **Top helper this week** | Most points last week (§2)                                                 |

   A badge is lost when what earned it is broken: **Trusted** when a report is upheld, and every
   badge when 8 West ends someone's Community (ADR-167 §9).

4. **Thanks:**
   - A **Thanks** button appears where someone really helped you: on a linked organization's answer
     (ADR-165 §5), and on each collaborator in your list (ADR-166).
   - One thanks for each person each 7 days. Only members who have been in Community for 7 days can
     thank. Your profile shows "**Thanked by 12 people**" (different people, not presses).
5. **Getting started**, on the People page, for you only: **Fill in your profile**, **Find
   someone**, **Send a message**, **Link with an organization**, **Invite a helper**. Each is ticked
   when done. Finishing it earns 10 points once (§1). It hides when all are done, or when you close
   it.
6. **A free month of Pro for invitations** (question 18, as recommended):
   - **Invite someone** gives you your share link and picture code (ADR-163 §6) and **Invite by
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
7. **Never:** points for sending things, streaks, money for anyone under 18, or members under 18 on
   the leaderboard.
8. **The Ledger records** `community.points_earned` (how many, and for what), `community.badge_earned`,
   `community.badge_lost`, `community.thanked`, `community.thanks_received`, and
   `community.reward_earned` (the month, never payment details).
9. **New desktop commands are the main window's alone**, with IPC tests that refuse a second window,
   the sign, and a web page: **Thanks**, the leaderboard, points, badges, and **Getting started**.

## Consequences

- People who help each other rise to the top, and newcomers can see who has been around and helped.
- A leaderboard makes people try to game it. The limits in §1, points that need someone else's
  agreement, the **Cheating for points** report reason (ADR-167), and 8 West's power to take points
  away are the brakes. 8 West watches the top of the board for cheating, as part of reading reports.
- Invitations that turn into paying members are paid back in Pro months: 8 West spends about $38 to
  win a member who pays $19 a month.
- The account service keeps a little more (points, who thanked whom, who invited whom). ADR-168 says
  for how long.

## Alternatives considered

- **Points for every message or action.** Easy to make, and the most active spammers would top the
  board.
- **No points or leaderboard, badges and thanks only** (the builder's first recommendation). Less
  to game, but the owner wants points and a leaderboard.
- **Points that buy things** (Pro months, extras). That makes points worth money, which invites far
  more cheating and brings tax and promotion rules; the free month for a paying invitation (§6) is
  the money reward.
- **Rewards for every invitation sent, or every new member.** Rewards made-up accounts.
- **Members under 18 on the leaderboard.** It would show them to every adult, which the protections
  in ADR-162 §4 rule out.
