# ADR-163: Your profile, and how people find each other

- **Status:** Accepted, with the owner's change (2026-10-02). To question 9 the owner answered: "**We
  need a way for people to find each other. We need to encourage community participation. Come up
  with ways to reward users for community participation.**" So this record adds the **Community
  directory** (§4), which waits for the owner's answer to question 16 in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md). Rewards are in
  [ADR-169 (rewards for taking part)](ADR-169-rewards-for-taking-part.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Builds on:** [ADR-056 (the owner's tile)](ADR-056-the-owners-tile.md), which said the tile is
  "never sent anywhere" until Phase 24

> **On screen** (ADR-010, plain words and rank names): **Show my profile to people in Community**,
> **Your name in Community: @frank-8west**, **List me in the Community directory?** (**Yes** /
> **No**), **Directory**, **What your business does**, **Where** (state or country), **New this
> week**, **Find someone**, **Invite by email**, **Share my profile** (a link and a picture code),
> **What people see**, **Hidden** / **Shown to people in Community**.

## In short

Your tile (picture, name, status, mood, and message) stays on your PC unless you choose **Show my
profile to people in Community**. People find each other three ways: the **Community directory**,
which lists people who chose to be in it, searchable by name, company, what their business does,
and where; your **Community name** (like `@frank-8west`) or your **share link and picture code**;
and an **email invitation**. Profiles are only for people signed in to Community: never on the open
web, never seen by search engines. Members under 18 are never in the directory.

## Context

- ADR-056 keeps the tile in the Ledger's shared record and says it is never sent anywhere. Phase 24's
  plan: "public profiles (opt-in): the owner's avatar, status, mood, and message from Phase 18,
  through the 8 West account". Its goal: "Let Plenipo owners find each other".
- The owner (question 9): people need a way to find each other, and Community should encourage
  taking part.
- A list of members is also a list for spammers and scammers, and a search by email tells strangers
  who uses Plenipo. A directory must be something people choose, and hard to copy whole.
- The plan keeps "public posting, feeds, or a marketplace" out of Phase 24. A directory of people is
  none of those: nobody posts in it, and nothing is bought or sold through it.
- Members can be 13 to 17 (ADR-162 §4).
- The picture is already safe to share: Plenipo keeps only a 256 × 256 PNG it has checked
  (ADR-056 §3), never a file path.

## Decision

1. **Hidden until you choose.** Settings → Community → **Show my profile to people in Community**.
   While it is off, the account service holds no profile of yours, only what a conversation needs
   (your Community name, and your display name to people you write to).
2. **What is shown, when it is on,** each with its own box: your **picture**, your **name** (as on
   your tile), your **status**, your **mood**, your **message**, your **company** (from your
   account, if you gave one), **what your business does** (up to 3 kinds from a fixed list, such as
   "Construction" or "Accounting", and one line of your own words, up to 80 characters), and
   **where** (a state or a country, never a town or an address). **What people see** shows the card
   exactly as others will see it.
3. **Your Community name** (`@` and 3 to 30 letters, numbers, or dashes), chosen when you turn
   Community on, unique, and changeable once every 30 days. Names that copy 8 West or Plenipo
   (`8west`, `plenipo`, `support`, `admin`, and similar) are refused.
4. **The Community directory** (question 16):
   - **You choose.** Turning Community on asks "**List me in the Community directory?**" with
     **Yes** and **No**, and neither chosen for you; it explains who will see you. You can change it
     any time. Being listed turns your profile on.
   - **Search and browse** by name, company, what the business does, and where. Each card shows
     your profile and your badges (ADR-169), with **Message** and **Link with this organization**.
   - **New this week** lists people who joined the directory in the last 7 days, so newcomers are
     seen. It is a list of people, not posts.
   - **Members under 18 are never listed**, and never shown in **New this week** (ADR-162 §4).
   - **Hard to copy whole:** results come 20 at a time, and one account can see at most 200 cards a
     day; the account service slows and then stops an account that looks like it is copying.
   - **Only people signed in to Community** can open the directory.
5. **Other ways to find you:**
   - **Your exact Community name.** **Find someone** shows a card for an exact match only, with a
     limit on look-ups an hour. For a member under 18, a look-up by an adult shows only "**Send a
     message request**", no card.
   - **Share my profile:** a link (`getplenipo.com/c/frank-8west`) and a picture code (QR code) you
     can put on a business card or an email. The page says how to find `@frank-8west` in Plenipo
     (**Find someone**) and how to join, and nothing about you. It never checks whether the name
     exists, so it tells nobody who is a member. It is a plain page on Plenipo's website: it opens
     nothing in Plenipo by itself, so a link can never make Plenipo do anything.
   - **Invite by email.** 8 West emails that address a link to join or answer you. Plenipo never
     says whether that address already has an account.
   - **No search by email**, so nobody can learn who uses Plenipo.
6. **Who sees your card:** people signed in to Community who find you as above, people you write to
   or link with, and your collaborators. Never a visitor who is not signed in, and never a search
   engine. For members under 18, only people they already talk with or work with (ADR-162 §4).
7. **Status and mood follow your tile.** When your profile is shown, a change on your tile is sent
   to the account service; nothing is sent while it is hidden. "**Do not disturb**" is shown to
   others as **Busy**. Adults never see a member under 18's status or mood.
8. **The picture is checked again** by the account service (a real PNG, at most 256 × 256 and
   256 KB) before it is shown to anyone, and anyone can report a profile (ADR-167). 8 West can hide
   a picture, a message, or a "what we do" line that breaks the rules.
9. **Turning it off** removes your card and your directory listing from the account service at once
   (ADR-168). People you already talk with keep seeing your name in those conversations.
10. **The Ledger records** `community.profile_shown`, `community.profile_hidden`,
    `community.profile_changed` (which parts, never the picture), and `community.directory_listed`
    and `community.directory_unlisted`.
11. **New desktop commands are the main window's alone**, with IPC tests that refuse a second
    window, the sign, and a web page.

## Consequences

- People can meet new people in Plenipo, which the owner wants, and only people who chose it are
  listed.
- A directory still helps a determined spammer find people. Paid accounts to start conversations
  (ADR-162 §5), **Requests** (ADR-164 §7), limits, and reports are the brakes.
- The account service holds more about listed people (what they do, and where). ADR-168 says it is
  deleted at once when they leave the directory.

## Alternatives considered

- **No directory: only an exact name or an email invitation** (the builder's first recommendation).
  Safer, but the owner wants people to be able to find each other.
- **Everyone listed unless they opt out.** More people to find, but people would be listed without
  choosing it.
- **Search by email.** Handy, but it tells anyone whether an email address uses Plenipo.
- **A public web page for each profile, seen by search engines.** Out: profiles are for people in
  Community. The share link says only that you are on Plenipo Community.
- **Posts or a feed to get people talking.** The plan keeps them out of Phase 24.
