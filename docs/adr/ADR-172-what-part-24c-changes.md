# ADR-172: What part 24C changes after checking today's code

- **Status:** Accepted (2026-10-02). Before coding part 24C, the builder checked today's code and the
  contract against the records and asked the owner five questions. The owner answered: stickers,
  "**Leave out the stickers for now.**"; the other four "**as recommended**".
- **Date:** 2026-10-02
- **Phase:** 24 (Community), part 24C
- **Amends:** [ADR-164 (private messages, sealed)](ADR-164-private-messages-sealed.md) §4 (stickers)
  and §10 (**Delete for me**); [ADR-169 (rewards for taking part)](ADR-169-rewards-for-taking-part.md)
  §4, §5, and §6 (thanks, **Getting started**, and what counts as an invitation); and
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md) §2 (releases).

> **On screen** (ADR-010, plain words and rank names): **Delete for me** ("This deletes it from this
> PC. Your other PCs, and Pat, keep their copies."), **Thanked by 12 people**, **Getting started**,
> **Invite by email**, **Share my profile**.

## In short

Five small changes, each because of what the contract or today's code can do, each answered by the
owner:

1. **No stickers for now.**
2. **Delete for me** deletes a message from **this PC** only.
3. **Share my profile** helps people **find** you. Only **Invite by email** counts as an invitation
   for points and the free month.
4. The **Thanks** button comes with linked organizations and collaborators, because that is where
   someone helps you. **Getting started** shows its first three steps now, and the other two come
   with those parts.
5. **One release** when all of part 24C is done.

## Context

- **Stickers:** ADR-164 §4 has Plenipo's own sticker sets, drawn for Plenipo and built into each
  release. Nobody has drawn them yet.
- **Delete for me:** ADR-164 §10 says it removes a message "from your PCs". The contract has no way
  for one of your PCs to tell your others that you deleted something (contract §6: sealed items go
  to another member).
- **Invitations:** ADR-169 §6 says the share link and picture code each carry your invitation. The
  contract records who invited whom only for an email invitation (contract §13); joining (contract
  §3) carries no inviter, and the share page says nothing about you (ADR-163 §6).
- **Thanks:** ADR-169 §4 puts **Thanks** on a linked organization's answer and on a collaborator, and
  the contract allows thanks only for those (contract §12). Both come in parts 24D and 24E.
  **Getting started** includes **Link with an organization** and **Invite a helper**, and finishing
  it needs a link and a collaboration (contract §12).
- **Releases:** each part of Phase 24 is a release (ADR-161 §2); part 24C is several pull requests.

## Decision

1. **Stickers are left out for now** (the owner: "Leave out the stickers for now."). No **Stickers**
   button. The contract keeps the field, so a message holding a sticker (from a later copy of
   Plenipo) shows "**A sticker**" as plain text. Sticker sets come later, with their own record.
2. **Delete for me deletes from this PC only.** Its question says so: "**This deletes it from this
   PC. Your other PCs, and Pat, keep their copies.**" Deleting from every one of your PCs can come
   with a later version of the contract.
3. **Only Invite by email is an invitation.** **Share my profile** (the link and the picture code)
   helps people find you, and gives no points and no free month. **Invite someone** shows both and
   says which one counts. One less way to cheat for points.
4. **Thanks and Getting started grow with the parts:**
   - part 24C shows "**Thanked by 12 people**" on cards and your profile; the **Thanks** button comes
     with linked organizations (24D, on an answer) and collaborators (24E, on each collaborator);
   - **Getting started** shows **Fill in your profile**, **Find someone**, and **Send a message** in
     part 24C; **Link with an organization** and **Invite a helper** are added by parts 24D and 24E.
     Its 10 points can be earned once both of those parts are open
     ([ADR-171 (switching on part by part)](ADR-171-switching-on-part-by-part.md)).
5. **One release when all of part 24C has merged,** not one for each pull request. With ADR-170, that
   release shows **Coming soon** until the owner switches the people part on.
6. **The safety code's recipe is written in the contract** (contract §7), with a worked example, so
   every copy of Plenipo makes the same 12 digits for the same two people. It covers both people's
   PCs' signing and sealing keys, so a changed key of either kind changes the code.

## Consequences

- Messages hold every letter, number, symbol, and emoji, and reactions; GIFs come after the owner
  chooses the library; stickers come later.
- Someone with two PCs who deletes a message on one still sees it on the other.
- Points for invitations come only from people invited by email.
- Until linked organizations and collaborators open, nobody can press **Thanks**, and nobody earns the
  points for finishing **Getting started**.

## Alternatives considered

- **Stickers drawn now** by the builder, or commissioned. The owner chose to leave them out for now.
- **Delete for me on every PC,** with a contract change and a change in the account service: a new
  kind of sealed note between one member's own PCs. More work for a small gain while most people use
  one PC.
- **Share links as invitations,** with an inviter's name when joining. Anyone could type any name to
  give that person points.
- **A Thanks button on messages.** Points for thanks would then reward chatting, which ADR-169 rules
  out.
