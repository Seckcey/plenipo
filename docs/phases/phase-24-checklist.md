# Phase 24 — Implementation Checklist

**Status: part 24C in progress** (started 2026-10-02), beside Phase 23. Parts 24A and 24B are done
(24B's last items wait for the owner). The owner answered the thirteen questions and the five
follow-up questions on 2026-10-02, and part 24C's five questions the same day (ADR-170 to ADR-172).
Builds on v1.19.4. Below, "[x]" is done. Plenipo is made by 8 West Ventures, LLC.

Source: `ROLLOUT_PLAN.md`, Phase 24 — Community, and the records written for it:

- [ADR-160 (building Phase 24 alongside Phase 23)](../adr/ADR-160-phase-24-alongside-phase-23.md)
- [ADR-161 (Phase 24 starts: what the check found, six parts, the owner's questions)](../adr/ADR-161-phase-24-starts.md)
- [ADR-162 (your 8 West account in Plenipo: signing in, 13 and older with protections for teens, who needs Pro)](../adr/ADR-162-your-account-in-plenipo.md)
- [ADR-163 (your profile, and how people find each other: the directory, a name, or an email)](../adr/ADR-163-profiles-and-finding-people.md)
- [ADR-164 (private messages, sealed end to end)](../adr/ADR-164-private-messages-sealed.md)
- [ADR-165 (linked organizations)](../adr/ADR-165-linked-organizations.md)
- [ADR-166 (collaborators: viewer, approver, and manager)](../adr/ADR-166-collaborators.md)
- [ADR-167 (block, report, and leave, and who handles reports)](../adr/ADR-167-block-report-leave.md)
- [ADR-168 (what Community keeps, where, and for how long)](../adr/ADR-168-what-is-kept-and-for-how-long.md)
- [ADR-169 (rewards for taking part: points, a leaderboard, badges, thanks, and a free month for invitations)](../adr/ADR-169-rewards-for-taking-part.md)
- [ADR-170 (Community follows 8 West's switch: Coming soon until it opens, no new release to turn it on)](../adr/ADR-170-community-follows-8-wests-switch.md)
- [ADR-171 (switching Community on part by part)](../adr/ADR-171-switching-on-part-by-part.md)
- [ADR-172 (what part 24C changes: no stickers yet, Delete for me on this PC, email invitations, one release)](../adr/ADR-172-what-part-24c-changes.md)

**Numbers:** ADR-160 to ADR-179 (the owner gave Phase 24 ADR-170 to ADR-179 on 2026-10-02). Dates are Pacific time. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md).

**Goal (plan):** "Let Plenipo owners find each other, talk, and work together — without anyone
reaching into anyone else's PC, files, sign-ins, or keys."

## In short, for the owner

- **Your answers are in** (2026-10-02): ten as recommended, and three changes: **13 and up**,
  **everything on a phone's keyboard** in messages, and a **directory** and **rewards** so people
  find each other and take part. Then five follow-up answers: protections for teens, **GIFs and
  stickers**, everyone **listed in the directory by default** (with **Appear offline** to hide),
  **points and a leaderboard**, and a free month of Pro for invitations.
- **Coming soon until 8 West opens it** (ADR-170): Plenipo follows the account service's own
  switch, so the release people already have starts working the moment the owner switches Community
  on. Each part is switched on when it is ready: people first, then linked organizations, then
  collaborators (ADR-171).
- **Then the attorney.** The drafts in [`docs/legal/phase-24/`](../legal/phase-24/README.md) go to
  an attorney. Community reaches real people only after the attorney approves them and a security
  review passes.
- **Six parts:** records and drafts → the account service's side → people (profiles, messages,
  block, report, leave) → linked organizations → collaborators → launch.
- **Linked organizations and collaborators waited** for Phase 23's Guard work (pull requests #136
  and #138), so Guard never took two sets of changes at once. Both merged on 2026-10-02.

## The owner's part

- [x] Answer the thirteen questions in ADR-161 (2026-10-02)
- [x] Answer follow-up questions 14 to 18 in ADR-161 (2026-10-02)
- [ ] Choose the GIF library (for example GIPHY) and accept its terms for 8 West, so the account
      service can hold its key (ADR-164 §4)
- [ ] Send the attorney the drafts in `docs/legal/phase-24/` (the owner: the night of 2026-10-02)
- [x] Decide who reads reports at launch: the owner, "until it gets to be too much" (ADR-167)
- [x] Answer part 24C's five questions (2026-10-02): follow 8 West's switch, switch on part by
      part, ADR-170 to ADR-179 for Phase 24, no stickers for now, and the rest as recommended
      (ADR-170 to ADR-172)
- [ ] After the attorney and each part's review: switch that part on at the account service
      (ADR-171), and set the lowest version of Plenipo allowed (ADR-170 §5)

## Part 24A — records and drafts (documents only)

- [x] ADR-161 to ADR-169, with the check of today's code
- [x] This checklist
- [x] Drafts for the attorney in `docs/legal/phase-24/`: terms of service for Community, changes to
      the privacy statement and the account privacy notice, the age requirement, and the moderation
      and report process
- [x] The ADR index and the plan's order-of-work row
- [x] The owner's answers written into each record (status **Accepted**, with the owner's words),
      and the drafts changed to match
- [x] The answers to follow-up questions 14 to 18 written in (ADR-162, ADR-163, ADR-164, ADR-169),
      and the drafts changed to match

## Part 24B — the contract, then the account service's side

In this repository first:

- [x] `contracts/community/v1`: sign-in with a code; PC keys; Community names; profiles; the sealed
      mailbox (envelopes, picking up, deleting); the report stamp and proof (ADR-164 §6); blocks;
      reports; links and invitations; limits; example requests and answers; a worked item with its
      stamp and report proof (the published HPKE test answers come with Plenipo's side, part 24C)
- [x] What a PC sends for Community, written in `docs/editions.md` (ADR-162 §6)

In `plenipo-account`, under its own rules (ADR-101, ADR-102). Written in two pull requests,
Seckcey/plenipo-account#20 (the base) and Seckcey/plenipo-account#21 (reports, points, and
invitations), each read by two security reviews with every finding fixed and tested. Ticked items
are written and tested; they reach the live service, still switched off, when the owner merges.

- [x] Sign-in with a code, **Allow** on the account site, the Community pass, and **Remove** for each
      PC (ADR-162 §2)
- [x] The birth month and year, refused under 13 with nothing kept, and changed only by 8 West
      (ADR-162 §4)
- [x] The protections for members 13 to 17, enforced by the service (ADR-162 §4)
- [x] PC keys, Community names, profiles, and the picture check (ADR-163)
- [x] The directory: every adult listed unless they appear offline, search, **New this week**, and
      the limits against copying (ADR-163 §4, §5)
- [ ] GIF search through the service, with the library's key and the ratings by age, keeping no
      search words (ADR-164 §4). Waits for the owner's choice of library; until then the service
      answers `gifs_unavailable`
- [x] Points with their limits, the leaderboard (this week and all time, adults who do not appear
      offline), and taking points away (ADR-169 §1, §2)
- [x] Badges worked out each day, thanks, and its limits (ADR-169 §3, §4)
- [ ] Rewards for invitations: who invited whom, the 14-day wait, Stripe credit, 12 a year, the
      card and email checks, taking back, and the admin switch (ADR-169 §6). Who invited whom, the
      check that the account uses the invited address, and the 150-day limit are written; the free
      month itself (Stripe credit) comes next
- [x] The sealed mailbox, the stamping key, and the limits (ADR-164 §3, §6, §8)
- [x] Blocks, enforced by the service (ADR-167 §4)
- [x] Reports, the proof check, and the **Reports** admin page with its emails (ADR-167 §5 to §11)
- [x] The nightly cleanup for every time in ADR-168, each with a test
- [x] **Delete my account** removes Community too (ADR-168 §5)
- [ ] Switched off on the live service until 24F (off by default: `COMMUNITY_ENABLED` unset; ticked
      once the merged service is deployed and checked off)
- [ ] The changes for ADR-170 and ADR-171, in a session of its own: "not open" as its own answer,
      **Is Community open?**, the lowest version of Plenipo, and a switch for linked organizations and
      one for collaborators ([the list](phase-24-account-service-changes.md))

## Part 24C — people: profile, messages, block, report, leave

- [x] ADR-170 to ADR-172, and [the list of changes for the account service](phase-24-account-service-changes.md)
- [x] The contract's change, as its own reviewed change: **Is Community open?**, `not_open`,
      `update_needed`, closed parts, the safety code's recipe, and a worked seal (ADR-170 §7,
      ADR-172 §6)
- [x] `Limit::CommunityStart` in `Entitlements::check` (ADR-162 §5)
- [x] Guard's outbound purpose **Community**: only `account.getplenipo.com`, only while signed in
      (ADR-162 §7)
- [x] The **Community** switch, signing in with a code, the age box, signing out (ADR-162)
- [x] Each PC's Community key in the Vault (ADR-162 §3)
- [x] Your profile: each part's box, all ticked to begin with (the owner's answer to question 16),
      and **What people see** (ADR-163)
- [x] Turning Community on says "**You'll be listed in the Community directory**", and **Appear
      offline** sits beside your status (ADR-163 §1, §5)
- [x] The directory, **Find someone**, **Invite by email**, and **Share my profile** (ADR-163 §4,
      §6), with the share page on the website (the same page for every name)
- [x] Messages: HPKE sealing with test answers, signing, **Requests**, the safety code, "**Pat's
      computers changed**", **Delete for me** (ADR-164)
- [x] Every letter, number, symbol, and emoji, emoji reactions, and hidden control characters shown
      as visible marks (ADR-164 §4)
- [x] A pasted or dropped photo is refused (ADR-164 §4). The **GIF** button stays hidden until the
      owner chooses the library (then a release adds Guard's **GIFs** purpose, ADR-170 §8); stickers
      are left out for now (ADR-172 §1)
- [x] Points, the leaderboard, badges, "**Thanked by**", **Getting started** (its first three
      steps), and **Invite someone** (ADR-169, ADR-172 §3, §4). The **Thanks** button comes with
      parts 24D and 24E
- [x] **Give to a worker**, fenced as outside words (a new `fence::Source` kind) (ADR-164 §9)
- [x] Block, report (with the proof), and leave, everywhere (ADR-167)
- [x] **Delete my Community data from this PC** (ADR-168 §3)
- [x] Ledger events `community.*`, never a message's words
- [x] New desktop commands, main window only, with IPC tests
- [x] **Coming soon** while the account service says Community is not open, **Check again**,
      **Update Plenipo to use Community**, and **Community is closed for now** (ADR-170)
- [x] Words added to `docs/design/vocabulary.md`

## Part 24D — linked organizations (pull requests #136 and #138 merged)

- [ ] Link requests, accepting, the receiving position, the safety code, and **What crosses this
      link** (ADR-165 §1)
- [ ] Sending an objective: words only, sealed, with limits (ADR-165 §2)
- [ ] Receiving: an approval, Guard's checks in order, `Workforce::give_objective`, outside words
      (ADR-165 §3, §4)
- [ ] The **Keep these approvals on my PC only** box for linked objectives (ADR-165 §3)
- [ ] The state that goes back by itself, and **Send the answer back** (ADR-165 §5)
- [ ] Pausing, unlinking, blocking, and Pro ending (ADR-165 §6)
- [ ] Ledger events `link.*` in both Ledgers (ADR-165 §7)
- [ ] New desktop commands, main window only, with IPC tests

## Part 24E — collaborators (pull requests #136 and #138 merged)

- [ ] Invitations: role, part, and the approvals only you can answer, all ticked (ADR-166 §3 to §5)
- [ ] Each role's fixed list in code, and Guard's checks with **who is asking** (ADR-166 §1, §6)
- [ ] The collaborator pass, `contracts/phone-relay/v2`, the relay, and the stand-in relay's **bad
      relay** tests (ADR-166 §5)
- [ ] `source = collaborator:<ID>`, and a test for every owner-only check that a collaborator is
      refused (ADR-166 §7)
- [ ] **Shared with you** on the collaborator's side (ADR-166 §11)
- [ ] Remove, leave, pause, and Pro ending (ADR-166 §9)
- [ ] New desktop commands, main window only, on both sides, with IPC tests

## Part 24F — launch

- [ ] The security review at the highest effort, of this repository and the account service, before
      **each** part is switched on (ADR-160 §7, ADR-171)
- [ ] The attorney's approved texts go into `apps/website/legal/` and the account site's legal pages
- [ ] Each part switched on at the live service when it is ready, with no release needed for it
      (ADR-170, ADR-171): the people part, then linked organizations, then collaborators
- [ ] The acceptance scenario on two test accounts, then the acceptance report
- [ ] The order of work, the roadmap, and `docs/editions.md` updated

## Tests (from the plan, and the records)

- [ ] A linked organization's objective waits for the receiving owner's approval and runs under
      their Guard
- [ ] A collaborator's permissions are enforced: a viewer cannot approve; an approver cannot hire
- [ ] A blocked person cannot message or send objectives
- [ ] No file, path, sign-in, key, or Ledger content crosses between organizations unless an owner
      sends it on purpose
- [ ] Removing a collaborator ends their access at once
- [x] A made-up report (words nobody sent) is refused (ADR-164 §6)
- [ ] An approval only the owner can answer refuses a collaborator's answer (ADR-166 §3)
- [ ] Each time in ADR-168 is enforced by the cleanup
- [x] A Free copy that never signs in to Community still never contacts 8 West (ADR-162 §5)
- [x] Under 13 cannot join, and nothing of the answer is kept (ADR-162 §4)
- [x] A member under 18 is never in the directory, an adult never sees their status or mood, and an
      adult's message to them lands in **Requests** (ADR-162 §4)
- [x] The directory cannot be copied whole, and **Appear offline** removes you from it and the
      leaderboard at once (ADR-163 §4, §5)
- [x] A message can never make Plenipo fetch anything but the GIF library's pictures (ADR-164 §4)
- [ ] Sending messages earns no points, and one person can give another at most 20 points a month
      (ADR-169 §1)
- [ ] A reward cannot be earned by inviting yourself, and is taken back on a refund (ADR-169 §6)

## Acceptance (plan)

Two owners link their organizations. One sends the other an objective, which runs only after the
receiving owner approves it and under their own permissions. One owner invites a collaborator as an
approver, who approves a task from their own Plenipo. Every step is in both Ledgers, and nothing
else crossed.
