# Phase 24 — Implementation Checklist

**Status: part 24A in progress** (started 2026-10-02), beside Phase 23. Builds on v1.19.4. Below,
"[x]" is done. Plenipo is made by 8 West Ventures, LLC.

Source: `ROLLOUT_PLAN.md`, Phase 24 — Community, and the records written for it:

- [ADR-160 (building Phase 24 alongside Phase 23)](../adr/ADR-160-phase-24-alongside-phase-23.md)
- [ADR-161 (Phase 24 starts: what the check found, six parts, the owner's questions)](../adr/ADR-161-phase-24-starts.md)
- [ADR-162 (your 8 West account in Plenipo: signing in, 18 and older, who needs Pro)](../adr/ADR-162-your-account-in-plenipo.md)
- [ADR-163 (your profile, and how people find each other)](../adr/ADR-163-profiles-and-finding-people.md)
- [ADR-164 (private messages, sealed end to end)](../adr/ADR-164-private-messages-sealed.md)
- [ADR-165 (linked organizations)](../adr/ADR-165-linked-organizations.md)
- [ADR-166 (collaborators: viewer, approver, and manager)](../adr/ADR-166-collaborators.md)
- [ADR-167 (block, report, and leave, and who handles reports)](../adr/ADR-167-block-report-leave.md)
- [ADR-168 (what Community keeps, where, and for how long)](../adr/ADR-168-what-is-kept-and-for-how-long.md)

**Numbers:** ADR-160 to ADR-169. Dates are Pacific time. The app uses the plain words in
[`docs/design/vocabulary.md`](../design/vocabulary.md).

**Goal (plan):** "Let Plenipo owners find each other, talk, and work together — without anyone
reaching into anyone else's PC, files, sign-ins, or keys."

## In short, for the owner

- **First, your answers.** ADR-161 has thirteen questions, each with a recommendation. Nothing is
  coded before you answer.
- **Then the attorney.** The drafts in [`docs/legal/phase-24/`](../legal/phase-24/README.md) go to
  an attorney. Community reaches real people only after the attorney approves them and a security
  review passes.
- **Six parts:** records and drafts → the account service's side → people (profiles, messages,
  block, report, leave) → linked organizations → collaborators → launch.
- **Linked organizations and collaborators wait** for Phase 23's Guard work (pull requests #136 and
  #138) to merge, so Guard never takes two sets of changes at once.

## The owner's part

- [ ] Answer the thirteen questions in ADR-161 (or "as recommended").
- [ ] Hire an attorney, and send them the drafts in `docs/legal/phase-24/`.
- [ ] Decide who reads reports at launch, and plan the time for it (ADR-167).
- [ ] After the review and the attorney: say when Community may reach real people (24F).

## Part 24A — records and drafts (documents only)

- [x] ADR-161 to ADR-168, proposed, with the check of today's code
- [x] This checklist
- [x] Drafts for the attorney in `docs/legal/phase-24/`: terms of service for Community, changes to
      the privacy statement and the account privacy notice, the age requirement, and the moderation
      and report process
- [x] The ADR index and the plan's order-of-work row
- [ ] The owner's answers written into each record (status **Accepted**, with the owner's words)

## Part 24B — the contract, then the account service's side

In this repository first:

- [ ] `contracts/community/v1`: sign-in with a code; PC keys; Community names; profiles; the sealed
      mailbox (envelopes, picking up, deleting); the report stamp and proof (ADR-164 §6); blocks;
      reports; links and invitations; limits; example requests and answers; HPKE test answers
- [ ] What a PC sends for Community, written in `docs/editions.md` (ADR-162 §6)

In `plenipo-account`, under its own rules (ADR-101, ADR-102):

- [ ] Sign-in with a code, **Allow** on the account site, the Community pass, and **Remove** for each
      PC (ADR-162 §2)
- [ ] "I am 18 or older", recorded with its time (ADR-162 §4)
- [ ] PC keys, Community names, profiles, and the picture check (ADR-163)
- [ ] The sealed mailbox, the stamping key, and the limits (ADR-164 §3, §6, §8)
- [ ] Blocks, enforced by the service (ADR-167 §4)
- [ ] Reports, the proof check, and the **Reports** admin page with its emails (ADR-167 §5 to §11)
- [ ] The nightly cleanup for every time in ADR-168, each with a test
- [ ] **Delete my account** removes Community too (ADR-168 §5)
- [ ] Switched off on the live service until 24F

## Part 24C — people: profile, messages, block, report, leave

- [ ] `Limit::CommunityStart` in `Entitlements::check` (ADR-162 §5)
- [ ] Guard's outbound purpose **Community**: only `account.getplenipo.com`, only while signed in
      (ADR-162 §7)
- [ ] The **Community** switch, signing in with a code, the age box, signing out (ADR-162)
- [ ] Each PC's Community key in the Vault (ADR-162 §3)
- [ ] Your profile: hidden to begin with, each part's box, **What people see** (ADR-163)
- [ ] **Find someone** and **Invite by email** (ADR-163 §4)
- [ ] Messages: HPKE sealing with test answers, signing, **Requests**, the safety code, "**Pat's
      computers changed**", **Delete for me** (ADR-164)
- [ ] **Give to a worker**, fenced as outside words (a new `fence::Source` kind) (ADR-164 §9)
- [ ] Block, report (with the proof), and leave, everywhere (ADR-167)
- [ ] **Delete my Community data from this PC** (ADR-168 §3)
- [ ] Ledger events `community.*`, never a message's words
- [ ] New desktop commands, main window only, with IPC tests
- [ ] **Coming soon** until 24F
- [ ] Words added to `docs/design/vocabulary.md`

## Part 24D — linked organizations (after pull requests #136 and #138)

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

## Part 24E — collaborators (after pull requests #136 and #138)

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

- [ ] The security review at the highest effort, of this repository and the account service
      (ADR-160 §7)
- [ ] The attorney's approved texts go into `apps/website/legal/` and the account site's legal pages
- [ ] Community switched on, on the live service and in a release
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
- [ ] A made-up report (words nobody sent) is refused (ADR-164 §6)
- [ ] An approval only the owner can answer refuses a collaborator's answer (ADR-166 §3)
- [ ] Each time in ADR-168 is enforced by the cleanup
- [ ] A Free copy that never signs in to Community still never contacts 8 West (ADR-162 §5)

## Acceptance (plan)

Two owners link their organizations. One sends the other an objective, which runs only after the
receiving owner approves it and under their own permissions. One owner invites a collaborator as an
approver, who approves a task from their own Plenipo. Every step is in both Ledgers, and nothing
else crossed.
