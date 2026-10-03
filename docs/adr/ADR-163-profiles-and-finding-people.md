# ADR-163: Your profile, and how people find each other

- **Status:** Accepted, with the owner's changes (2026-10-02). To question 9 the owner answered:
  "**We need a way for people to find each other. We need to encourage community participation.
  Come up with ways to reward users for community participation.**" That added the **Community
  directory** (§4). To question 16 (the directory) the owner answered: "**Default is on and shows
  users listed. They have to appear offline to not be shown.**" So every adult member is listed and
  shown unless they choose **Appear offline** (§1, §4). Rewards are in
  [ADR-169 (rewards for taking part)](ADR-169-rewards-for-taking-part.md). The questions are in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Builds on:** [ADR-056 (the owner's tile)](ADR-056-the-owners-tile.md), which said the tile is
  "never sent anywhere" until Phase 24

> **On screen** (ADR-010, plain words and rank names): **You'll be listed in the Community
> directory**, **Appear offline**, **Offline**, **Your name in Community: @frank-8west**,
> **Directory**, **What your business does**, **Where** (state or country), **New this week**,
> **Find someone**, **Invite by email**, **Share my profile** (a link and a picture code), **What
> people see**.

## In short

When you turn Community on, your **profile is shown and you are listed in the Community
directory**, so people can find you. Your profile is your tile (picture, name, status, mood, and
message), plus your company, what your business does, and your state or country; you choose which
parts. To **not be shown**, choose **Appear offline**: you leave the directory and the leaderboard,
and people you already talk with see you as **Offline**. People also find each other by a
**Community name** (like `@frank-8west`), a **share link and picture code**, or an **email
invitation**. Profiles are only for people signed in to Community: never on the open web, never seen
by search engines. Members under 18 are never in the directory.

## Context

- ADR-056 keeps the tile in the Ledger's shared record and says it is never sent anywhere. Phase 24's
  plan: "public profiles (opt-in): the owner's avatar, status, mood, and message from Phase 18,
  through the 8 West account". Its goal: "Let Plenipo owners find each other".
- The owner (questions 9 and 16): people need a way to find each other, Community should encourage
  taking part, and the directory is **on by default**; **Appear offline** is how someone is not
  shown. Turning Community on is itself the choice the plan's "opt-in" asks for: nothing is shown
  until you turn Community on, and Plenipo says plainly, at that moment, that you will be listed.
- A list of members is also a list for spammers and scammers, and a search by email tells strangers
  who uses Plenipo. The directory must be hard to copy whole.
- The plan keeps "public posting, feeds, or a marketplace" out of Phase 24. A directory of people is
  none of those: nobody posts in it, and nothing is bought or sold through it.
- Members can be 13 to 17 (ADR-162 §4), and they are never in the directory (question 14).
- The picture is already safe to share: Plenipo keeps only a 256 × 256 PNG it has checked
  (ADR-056 §3), never a file path.

## Decision

1. **Shown when you turn Community on** (question 16). Turning Community on says, before you
   finish: "**You'll be listed in the Community directory, so people can find you. Choose Appear
   offline at any time to not be shown.**" It shows your card as others will see it (**What people
   see**), with the boxes in §2. Nothing is sent before you finish turning Community on.
2. **What is shown,** each with its own box, **all ticked to begin with**: your **picture**, your
   **name** (as on your tile), your **status**, your **mood**, your **message**, your **company**
   (from your account, if you gave one), **what your business does** (up to 3 kinds from a fixed
   list, such as "Construction" or "Accounting", and one line of your own words, up to 80
   characters, both empty until you fill them), and **where** (a state or a country, never a town or
   an address, empty until you choose it). Untick any part, at any time.
3. **Your Community name** (`@` and 3 to 30 letters, numbers, or dashes), chosen when you turn
   Community on, unique, and changeable once every 30 days. Names that copy 8 West or Plenipo
   (`8west`, `plenipo`, `support`, `admin`, and similar) are refused.
4. **The Community directory:**
   - **Every adult member is listed** unless they choose **Appear offline** (the owner's answer to
     question 16).
   - **Search and browse** by name, company, what the business does, and where. Each card shows the
     profile, badges, and points (ADR-169), with **Message** and **Link with this organization**.
   - **New this week** lists people who joined in the last 7 days, so newcomers are seen. It is a
     list of people, not posts.
   - **Members under 18 are never listed**, and never in **New this week** (ADR-162 §4).
   - **Hard to copy whole:** results come 20 at a time, and one account can see at most 200 cards a
     day; the account service slows and then stops an account that looks like it is copying.
   - **Only people signed in to Community** can open the directory.
5. **Appear offline** (Settings → Community, and the top bar's panel beside your status):
   - you leave the directory, **New this week**, and the leaderboard (ADR-169) at once;
   - people you already talk with, link with, or help see you as **Offline**, and nothing of your
     status or mood;
   - nobody can find your card by your Community name; they see only "**Send a message
     request**";
   - your messages, links, and collaborations keep working;
   - turning it off lists you again.
6. **Other ways to find you:**
   - **Your exact Community name.** **Find someone** shows a card for an exact match only, with a
     limit on look-ups an hour. For a member under 18, or a member who appears offline, it shows
     only "**Send a message request**", no card.
   - **Share my profile:** a link (`getplenipo.com/c/frank-8west`) and a picture code (QR code) you
     can put on a business card or an email. The page says how to find `@frank-8west` in Plenipo
     (**Find someone**) and how to join, and nothing about you. It never checks whether the name
     exists, so it tells nobody who is a member. It is a plain page on Plenipo's website: it opens
     nothing in Plenipo by itself, so a link can never make Plenipo do anything.
   - **Invite by email.** 8 West emails that address a link to join or answer you. Plenipo never
     says whether that address already has an account.
   - **No search by email**, so nobody can learn who uses Plenipo.
7. **Who sees your card:** people signed in to Community, through the directory and the ways above,
   people you write to or link with, and your collaborators. Never a visitor who is not signed in,
   and never a search engine. For members under 18, only people they already talk with or work with
   (ADR-162 §4).
8. **Status and mood follow your tile.** A change on your tile is sent to the account service while
   you are shown; nothing is sent while you appear offline. "**Do not disturb**" is shown to others
   as **Busy**. Adults never see a member under 18's status or mood.
9. **The picture is checked again** by the account service (a real PNG, at most 256 × 256 and
   256 KB) before it is shown to anyone, and anyone can report a profile (ADR-167). 8 West can hide
   a picture, a message, or a "what we do" line that breaks the rules.
10. **Leaving Community** removes your card and your listing from the account service at once
    (ADR-168). People you already talk with keep seeing your name in those conversations.
11. **The Ledger records** `community.profile_changed` (which parts, never the picture),
    `community.appeared_offline`, and `community.appeared_online`.
12. **New desktop commands are the main window's alone**, with IPC tests that refuse a second
    window, the sign, and a web page.

## Consequences

- People can meet new people in Plenipo from the first day, as the owner wants: everyone who turns
  Community on is easy to find.
- Being listed is the default, so Plenipo must say so plainly when Community is turned on, and
  **Appear offline** must be one click away. The attorney checks whether any privacy law asks for
  more (for example, people in the European Union).
- A directory still helps a determined spammer find people. Paid accounts to start conversations
  (ADR-162 §5), **Requests** (ADR-164 §7), limits, and reports are the brakes.
- The account service holds more about listed people (what they do, and where). ADR-168 says it is
  deleted at once when they leave Community.

## Alternatives considered

- **No directory: only an exact name or an email invitation** (the builder's first recommendation).
  Safer, but the owner wants people to be able to find each other.
- **A directory people choose to join, off by default** (the builder's second recommendation).
  More private, but fewer people to find; the owner chose on by default.
- **Hiding from the directory and appearing offline as two separate switches.** More choice, but
  the owner wants one simple switch.
- **Search by email.** Handy, but it tells anyone whether an email address uses Plenipo.
- **A public web page for each profile, seen by search engines.** Out: profiles are for people in
  Community. The share link says only how to find you.
- **Posts or a feed to get people talking.** The plan keeps them out of Phase 24.
