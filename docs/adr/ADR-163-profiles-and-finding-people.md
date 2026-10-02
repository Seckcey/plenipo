# ADR-163: Your profile, and how people find each other

- **Status:** Proposed (2026-10-02), waiting for the owner's answer to question 9 in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Builds on:** [ADR-056 (the owner's tile)](ADR-056-the-owners-tile.md), which said the tile is
  "never sent anywhere" until Phase 24

> **On screen** (ADR-010, plain words and rank names): **Show my profile to people in Community**,
> **Your name in Community: @frank-8west**, **Find someone**, **Invite by email**, **What people
> see**, **Hidden** / **Shown to people in Community**.

## In short

Your tile (picture, name, status, mood, and message) stays on your PC unless you choose
**Show my profile to people in Community**. Then people who are signed in to Community can see it,
and only them: it is never on the open web, and search engines never see it. People find you by
your **Community name** (like `@frank-8west`), typed exactly, or by an **email invitation** you
send. There is no list of everyone to browse.

## Context

- ADR-056 keeps the tile in the Ledger's shared record and says it is never sent anywhere. Phase 24's
  plan: "public profiles (opt-in): the owner's avatar, status, mood, and message from Phase 18,
  through the 8 West account".
- A searchable list of every member, or search by email, tells strangers who uses Plenipo and helps
  spammers. Business owners can still want to be found by people they already know.
- The picture is already safe to share: Plenipo keeps only a 256 × 256 PNG it has checked
  (ADR-056 §3), never a file path.

## Decision

1. **Hidden until you choose.** Settings → Community → **Show my profile to people in Community**,
   off to begin with. While it is off, the account service holds no profile of yours, only what a
   conversation needs (your Community name, and your display name to people you write to).
2. **What is shown, when it is on,** each with its own box, all ticked when you turn it on:
   your **picture**, your **name** (as on your tile), your **status**, your **mood**, your
   **message**, and your **company** (from your account, if you gave one). **What people see**
   shows the card exactly as others will see it.
3. **Your Community name** (`@` and 3 to 30 letters, numbers, or dashes), chosen when you turn
   Community on, unique, and changeable once every 30 days. Names that copy 8 West or Plenipo
   (`8west`, `plenipo`, `support`, `admin`, and similar) are refused.
4. **How people find you** (question 9):
   - **By your exact Community name.** **Find someone** shows a card only for an exact match; a
     near miss shows nothing. The account service limits how many names one account can look up
     in an hour, so nobody can try every name.
   - **By an email invitation.** **Invite by email** asks 8 West to email that address a link to
     join or answer you. Plenipo never says whether that address already has an account.
   - **No list of everyone, and no search by email.** A member list or an "are you here?" email
     search is out, so nobody can collect who uses Plenipo.
5. **Who sees your card:** people signed in to Community who find you as above, people you write to
   or link with, and your collaborators. Never a visitor who is not signed in, and never a search
   engine.
6. **Status and mood follow your tile.** When your profile is shown, a change on your tile is sent
   to the account service; nothing is sent while it is hidden. "**Do not disturb**" is shown to
   others as **Busy**, so they know not to expect an answer.
7. **The picture is checked again** by the account service (a real PNG, at most 256 × 256 and
   256 KB) before it is shown to anyone, and anyone can report a profile (ADR-167). 8 West can hide
   a picture or a message that breaks the rules.
8. **Turning it off** removes your card from the account service at once (ADR-168). People you
   already talk with keep seeing your name in those conversations.
9. **The Ledger records** `community.profile_shown`, `community.profile_hidden`, and
   `community.profile_changed` (which parts, never the picture), as ADR-056 records the tile.
10. **New desktop commands are the main window's alone**, with IPC tests that refuse a second
    window, the sign, and a web page.

## Consequences

- Finding someone needs their Community name or their email, so people mostly find each other
  through people they already know. That is slower, and much safer.
- The account service now holds pictures and messages that other people see, so it needs the
  report path in ADR-167 from the first day.

## Alternatives considered

- **A directory you can browse** (opt-in). Easier to meet new people, but it is a list for
  spammers, and Phase 24's plan keeps out public posting and feeds.
- **Search by email.** Handy, but it tells anyone whether an email address uses Plenipo.
- **A public web page for each profile.** Out: profiles are for people in Community, not the web.
