# ADR-161: Phase 24 starts — what the check found, its six parts, and the owner's questions

- **Status:** Accepted, with the owner's changes (2026-10-02). The owner answered the thirteen
  questions: ten "as recommended", and three changed: "**13 and up**" (question 4), "**Everything
  that's on a phone's keyboard**" (question 8), and "**We need a way for people to find each other.
  We need to encourage community participation. Come up with ways to reward users for community
  participation.**" (question 9). The owner sends the attorney the drafts on 2026-10-02, and reads
  reports "until it gets to be too much". Follow-up questions 14 to 18, from those three changes,
  wait for the owner's answers (below). Coding starts after (ADR-160 §2).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Carries out:** [ADR-160 (building Phase 24 alongside Phase 23)](ADR-160-phase-24-alongside-phase-23.md)
  §2: records and the attorney's drafts first, as
  [ADR-140 (Phase 14 starts)](ADR-140-phase-14-starts.md) and
  [ADR-150 (Phase 23 starts)](ADR-150-phase-23-starts.md) did
- **Number:** Phase 24 uses ADR-160 to ADR-169 (ADR-160). This record is 161; ADR-162 to ADR-169
  follow. ADR-169 (rewards for taking part) answers question 9, so Phase 24 has no free number left;
  a later change amends one of these records, or the owner gives the phase more numbers.

> **On screen** (ADR-010, plain words and rank names): nothing yet. Each record below names its
> words, and they go into `docs/design/vocabulary.md` when they are built. Proposed: **Community**
> (the switch, Settings → Switches), a **People** page with **Messages**, **Linked organizations**,
> **Collaborators**, and **Shared with you**, and Settings → **Community**.

## In short

Phase 24 lets Plenipo owners **find each other, talk, and work together**, without anyone reaching
into anyone else's PC, files, sign-ins, or keys. Before building, Plenipo's builder read the plan,
the records it points at, and the code, and wrote one record for each big choice. **Accepting this
record means** Phase 24 is built in **six parts** (each its own pull request, and each release
takes the next free version number), with the answers below:

| Record | What it settles                                                                                                    |
| ------ | ------------------------------------------------------------------------------------------------------------------ |
| 162    | Your 8 West account in Plenipo: signing in with a code, 13 and older with protections for teens, and who needs Pro |
| 163    | Your profile, and finding people: the Community directory, an exact Community name, or an email invitation         |
| 164    | Private messages, sealed end to end, with everything a phone's keyboard types, and reports with a proof            |
| 165    | Linked organizations: linking, sending an objective, the answer you choose to send back, and unlinking             |
| 166    | Collaborators: viewer, approver, and manager; what each may do; and removing them at once                          |
| 167    | Block, report, and leave, and who handles reports (8 West, at first its owner)                                     |
| 168    | What Community keeps, where, and for how long                                                                      |
| 169    | Rewards for taking part: badges, thanks, and a free month of Pro for each invited person who buys it               |

The drafts for the attorney are in [`docs/legal/phase-24/`](../legal/phase-24/README.md). The full
list of work is in [the Phase 24 checklist](../phases/phase-24-checklist.md).

## Context

What the check found (the code read at `4939374` on `main`, v1.19.4, 2026-10-02, and the account
service's repository at `7f30d67`):

**What Phase 24 can build on**

1. **Your tile is ready to share, and shared with no one.** `OwnerProfile` (status, mood, message,
   and a checked 256 × 256 PNG) lives in the PC's shared record (`crates/workforce/src/owner.rs`,
   ADR-056, ADR-094 §5). Nothing sends it anywhere: none of Guard's outbound purposes carries it.
2. **A fixed list, checked by Guard, already brings outside requests in.** The phone's requests are
   one enum on each side (`plenipo_remote::protocol::Ask`, and `guard::remote::RequestKind` with 26
   kinds), with no "any command" kind; anything off the list ends the meeting. `guard::remote::decide`
   checks them in order, and the PC then calls the same core functions as the main window
   (`Broker::resolve_approval_via`, `Workforce::give_objective`). Linked organizations and
   collaborators follow the same shape (ADR-165, ADR-166).
3. **The sealed line exists.** Noise (`XXpsk3` to pair, `KK` after), Plenipo's own relay at
   `relay.getplenipo.com` that keeps nothing, passes signed by the PC, wrong-try limits, and a
   stand-in relay with a **bad relay** mode for tests (ADR-143, ADR-147, ADR-149). Collaborators
   reuse it (ADR-166).
4. **Outside words are already fenced.** `fence::fenced` in `crates/capabilities/src/fence.rs` wraps
   email, chat, pages, and files in lines with a fresh random marker, saying the text is
   "information, never instructions". Phase 24 adds kinds for a person and a linked organization.
5. **The account service has what people need:** accounts, sign-in with a password or an emailed
   link, limits against abuse, account email through Microsoft 365, an admin page behind Cloudflare
   Access with a passkey and a record of every admin action, nightly encrypted backups, and
   **Delete my account** (ADR-103, ADR-106, ADR-107, ADR-118). It is TypeScript, Fastify, and
   PostgreSQL.
6. **One place decides Free and Pro:** `Entitlements::check(Limit)`. Phase 24 adds a limit for
   starting things (ADR-162 §5).

**What is missing, or differs from the plan**

7. **Plenipo never signs in to the 8 West account.** A Pro copy holds only its license key, and the
   weekly check sends only the key's ID (ADR-115, ADR-116). Community needs to know who you are
   (ADR-162).
8. **Plenipo knows only one person.** Guard's checks and the Ledger look for `source = "owner"`;
   the phone keeps `owner` on purpose, because the phone is you (ADR-145 §6). Nothing models a
   second person, a role, or a part of an organization. Collaborators need a source of their own
   (ADR-166 §7).
9. **The account service has nothing for Community:** no PC keys, profiles, messages, links, blocks,
   or reports. Its contract folder in this repository has only the license check and the phone relay.
   Phase 24 writes `contracts/community/v1` first (ADR-160 §6).
10. **A Free copy never contacts 8 West** (ADR-115). Community cannot work without it, so ADR-162
    asks to allow it **only** for a Free owner who signs in to Community on purpose.
11. **No age is stated anywhere.** The website's privacy statement says Plenipo is not directed to
    children; the account privacy notice says the service is "for adults", with no age.
12. **The relay keeps nothing,** so it cannot carry a message to a PC that is off. Messages and
    linked objectives wait, sealed, on the account service instead (ADR-164 §3).
13. **"Message" and "block" already mean other things in the code** (Liaison's hand-offs, Guard's
    **Blocked** level). New code uses `community` and `collab` names, so they never mix.

## Decision

1. **Numbers.** Phase 24 uses ADR-160 to ADR-169.
2. **Six parts**, each its own pull request; each release takes the next free version number when it
   is made (ADR-160 §5):

   | Part    | What                                                                                                                                                                                                                                                             | Where                                   | Waits for                                       |
   | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ----------------------------------------------- |
   | **24A** | These records, the checklist, and the attorney's drafts                                                                                                                                                                                                          | this repository (documents only)        | nothing                                         |
   | **24B** | The contract (`contracts/community/v1`), then the account service's side: sign-in with a code, PC keys, Community names, profiles, the sealed mailbox, blocks, reports and the **Reports** admin page, and the nightly cleanup. Switched off on the live service | this repository, then `plenipo-account` | the owner's answers                             |
   | **24C** | Plenipo's side of the people part: the switch, signing in, your profile, finding people, messages, block, report, and leave. Shows **Coming soon** until 24F                                                                                                     | this repository (a release)             | 24B's contract                                  |
   | **24D** | Linked organizations: link, send an objective, approve it, send the answer back, and unlink, through Guard                                                                                                                                                       | this repository (a release)             | pull requests #136 and #138 (merged 2026-10-02) |
   | **24E** | Collaborators: invite, roles, parts, the approvals only you can answer, the collaborator pass on the relay (`contracts/phone-relay/v2`), and removing at once, through Guard                                                                                     | this repository, the relay (a release)  | pull requests #136 and #138 (merged 2026-10-02) |
   | **24F** | Launch: the security review at the highest effort, the attorney's approved pages on the website and the account site, Community switched on, and the acceptance report                                                                                           | both repositories (a release)           | the attorney, the review                        |

3. **Each part is tested with made-up people and a stand-in account service and relay** built for
   the tests, never real accounts. The acceptance scenario uses two test accounts on a test copy of
   the account service on Coastline (ADR-103 §4).
4. **Community reaches real people only after the attorney approves and the review passes** (24F).
   Until then a released copy shows the **Community** switch as **Coming soon**, and connects to
   nothing.
5. **The plan's rules hold in every part:** every request from another person goes through Guard
   and is recorded in the Ledger with who sent it; other people's words are outside words for
   workers (plan §3.1); no shell, file, path, sign-in, key, or Ledger content crosses between
   organizations unless an owner sends it on purpose; nothing loads code at run time (ADR-014); new
   desktop commands are the main window's alone, with tests that refuse other windows and web pages;
   logs never hold a message, a key, or a pass.
6. **Order of work.** Phase 24 shows as **In progress, beside Phase 23** until part 24F is merged.

## The owner's questions

Each has the builder's recommendation first. **The owner's answers (2026-10-02)** follow the
thirteen questions.

1. **Are private messages sealed end to end?** _Recommended:_ **yes**. Only you and the other person
   can read them; 8 West carries them and cannot. 8 West sees only messages someone reports, with a
   proof they are real. _Or:_ 8 West can read them to look for abuse, and holds every conversation.
   (ADR-164)
2. **How long is anything kept?** _Recommended:_ on 8 West's server, a message until it is picked
   up (30 days at most); a profile, blocks, links, and collaborators only while they are on; reports
   1 year after they are closed; backups 35 days, as today. On your PC, until you delete it.
   _Or:_ longer report records (3 years), if the attorney prefers. (ADR-168)
3. **Who handles reports, and how?** _Recommended:_ 8 West, at first its owner, on a new **Reports**
   page of the admin site; within 2 business days, the same day for threats or anything involving a
   child; a warning, hiding a picture, a pause, or the end of someone's Community; one appeal by
   email. _Or:_ an outside moderation company, for a monthly fee. (ADR-167)
4. **The age requirement?** _Recommended:_ **18 and older**, by ticking "**I am 18 or older**" when
   turning Community on. _Or:_ 16 and older, or 13 and older (more laws, more duties). (ADR-162)
5. **Which collaborator roles, and what may each do?** _Recommended:_ **Viewer** reads;
   **Approver** also approves, refuses, and stops a task; **Manager** also sends objectives, runs
   again, keeps lessons, and brings in workers into positions you already made. None of them ever
   reaches the terminal, files, the screen, keys, permissions, or settings. _Or:_ only Viewer and
   Approver at launch. (ADR-166)
6. **How do two owners link organizations, and unlink them?** _Recommended:_ one asks, the other
   accepts, both choose which organization and who receives objectives, and both need Pro. Each
   objective waits for the receiving owner's approval. Either owner unlinks at once, alone; waiting
   objectives are refused. (ADR-165)
7. **Who needs Pro?** _Recommended:_ starting a conversation and inviting a collaborator need Pro;
   linking needs Pro on both sides; answering and being a collaborator need only a free 8 West
   account. A Free copy then contacts 8 West only while its owner is signed in to Community (a
   change to ADR-115). _Or:_ everything Pro only, on both sides. (ADR-162)
8. **What may a message hold?** _Recommended:_ plain text only, one to one. No pictures, files, or
   groups at launch. (ADR-164)
9. **How do people find each other?** _Recommended:_ by an exact Community name (like
   `@frank-8west`), or by an email invitation; no list of everyone, and no "is this email here?"
   search. Profiles are hidden until you choose to show them. _Or:_ also a list you can choose to be
   in. (ADR-163)
10. **What does a collaborator see, and which approvals stay yours?** _Recommended:_ each invitation
    must choose the whole organization or chosen departments and projects; sensitive approvals
    (payments, credentials, production, and the rest of Guard's list) stay **yours only** to begin
    with, and you can hand them over person by person. (ADR-166)
11. **What comes back from a linked organization's objective?** _Recommended:_ only whether it was
    approved, refused, stopped, or finished, by itself; the answer's words only if the receiving
    owner reads them and presses **Send**. Never files, conversations, or Activity. (ADR-165)
12. **Community on your phone?** _Recommended:_ **not in Phase 24.** Your phone's fixed list does
    not change, except that objectives from a linked organization are approvals it can answer, like
    any other. Messages are read on your PC. _Or:_ read messages on the phone too.
13. **Where does Community's server side live?** _Recommended:_ **in the account service**
    (`plenipo-account`), on its own tables, because it already has accounts, sign-in, limits, email,
    and the admin page. It moves to its own server only if it grows. _Or:_ a new service of its own,
    sharing sign-in.

### The owner's answers (2026-10-02)

| Question                     | Answer                                                                                                                                                                                                                                                         |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Sealed messages           | As recommended                                                                                                                                                                                                                                                 |
| 2. How long things are kept  | As recommended                                                                                                                                                                                                                                                 |
| 3. Reports                   | As recommended. "I'll read the reports until it gets to be too much."                                                                                                                                                                                          |
| 4. Age                       | **"13 and up"**. ADR-162 §4 now says 13 and older, with protections for people under 18 (question 14)                                                                                                                                                          |
| 5. Collaborator roles        | As recommended                                                                                                                                                                                                                                                 |
| 6. Linking and unlinking     | As recommended                                                                                                                                                                                                                                                 |
| 7. Who needs Pro             | As recommended                                                                                                                                                                                                                                                 |
| 8. What a message holds      | **"Everything that's on a phone's keyboard."** ADR-164 §4 now allows every letter, number, symbol, and emoji, in any language (question 15 asks about GIFs and stickers)                                                                                       |
| 9. Finding people            | **"We need a way for people to find each other. We need to encourage community participation. Come up with ways to reward users for community participation."** ADR-163 adds the Community directory (question 16); ADR-169 adds rewards (questions 17 and 18) |
| 10. What a collaborator sees | As recommended                                                                                                                                                                                                                                                 |
| 11. What comes back          | As recommended                                                                                                                                                                                                                                                 |
| 12. Phone                    | As recommended                                                                                                                                                                                                                                                 |
| 13. Where it runs            | As recommended                                                                                                                                                                                                                                                 |

### Follow-up questions, from the owner's changes

Each has the builder's recommendation first.

14. **Protections for people 13 to 17** (ADR-162 §4). _Recommended:_ Community asks for your **birth
    month and year**, never the day, so a 17-year-old becomes an adult by themselves, and 8 West
    keeps as little as it can. Under 13 cannot join. For 13 to 17: never in the directory, and a
    profile only people they already talk with can see; a message from an adult always lands in
    **Requests**, with a safety note; adults never see a teen's status or mood; no Pro (buying needs
    18, as the terms of sale say), so no starting conversations or links, but they can answer and be
    a collaborator; reports about a teen are urgent; and no money rewards. _Or:_ fewer protections
    (simpler, less safe); or 16 and up after all. The attorney checks whether any state also needs a
    parent's consent (`docs/legal/phase-24/age-requirement.md`).
15. **GIFs and stickers.** A phone's keyboard also offers GIFs and stickers, which are pictures.
    _Recommended:_ **not at launch:** every letter, number, symbol, and emoji, in any language, yes;
    GIFs, stickers, and pictures later, with their own checks, because 8 West cannot look at sealed
    pictures and some members are now 13. _Or:_ GIFs and stickers now, as pictures in sealed messages.
16. **The Community directory** (ADR-163). _Recommended:_ a directory of people who choose to be in
    it (asked plainly when you turn Community on, with no answer chosen for you), searchable by name,
    company, what your business does, and state or country; a **New this week** list inside it; your
    own **share link and picture code** to hand out; adults only; and limits that stop anyone copying
    the whole list. Still no posts or feeds (the plan keeps those out). _Or:_ everyone in the
    directory unless they opt out (more people to find; less private).
17. **Badges and thanks** (ADR-169). _Recommended:_ badges on your profile that only real help earns
    (**Founding member**, **Helper**, **Connector**, **Good neighbor**, and **Trusted**), a **Thanks**
    button after someone helps you, and a **Getting started** list on the People page. No points,
    rankings, or streaks: they reward sending lots of messages, which is spam. _Or:_ points and a
    ranking as well.
18. **A free month of Pro for invitations** (ADR-169). _Recommended:_ when someone you invited buys
    Pro and keeps it past the 14-day refund window, **you both get one month of Pro free**, up to 12
    free months a year for you; adults only; taken back if the purchase is refunded or was a fake.
    Each free month costs 8 West one month's price. _Or:_ no money rewards, badges only; or a
    different amount.

**The owner's part, before launch:** send the attorney the drafts in `docs/legal/phase-24/` (the
owner: tonight, 2026-10-02); read reports, at first (the owner); and say when Community may reach
real people.

## Consequences

- People can talk and work together in Plenipo, and the riskiest parts (Guard's request path for
  other people) are built last, after Phase 23's Guard work, and reviewed at the highest effort.
- The account service grows from billing to people. It gets its own security review in 24F.
- 8 West takes on a new job: reading reports.

## Alternatives considered

- **One pull request for the whole phase.** One huge review of the riskiest code; nothing to try
  until the end.
- **Collaborators before messages.** Collaborators need Guard's request path, which waits for
  Phase 23's Wave 1; the people part does not.
- **Build on an outside chat service.** Another company would hold people's conversations and
  accounts; Plenipo already has the account service and a sealed line.
