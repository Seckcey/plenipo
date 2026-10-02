# ADR-162: Your 8 West account in Plenipo — signing in, who may join, and who needs Pro

- **Status:** Accepted, with the owner's change (2026-10-02): question 7 (who needs Pro) "as
  recommended"; question 4 (the age) "**13 and up**", instead of the 18 first recommended; and
  question 14 (the protections for people 13 to 17, §4.3) "as recommended", in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Amends:** [ADR-115 (a Free copy never contacts 8 West)](ADR-115-free-never-contacts-8-west.md),
  only for a Free copy whose owner signs in to Community on purpose (Decision 5)
- **Builds on:** [ADR-118 (customer accounts)](ADR-118-customer-accounts.md),
  [ADR-110 (one person, any of their PCs)](ADR-110-one-person-any-of-their-pcs.md),
  [ADR-101 (the account service's own repository)](ADR-101-account-service-repository-name.md)

> **On screen** (ADR-010, plain words and rank names): **Community** (the switch in Settings →
> Switches), **Sign in to your 8 West account**, **Open the sign-in page**, **Enter this code:
> 4KQ-7TD**, **Signed in as Frank Gonzalez**, **Sign out of your account**, **Your birth month and
> year**, **Community is for people 13 and older**, **Part of Pro**.

## In short

Community is about **people**, so Plenipo needs to know who you are. Today it does not: a Pro copy
holds a license key, and the weekly check sends only the key's ID (ADR-115, ADR-116). This record
says how you sign in to your **8 West account** inside Plenipo, that **everyone in Community must
be 13 or older**, with **extra protections for anyone under 18**, and that **starting** things (a
conversation, a link, an invitation) is part of **Pro**, while **answering** needs only a free
account.

## Context

- The account service (Phase 22) already has accounts: a name, an email, an optional company, a
  password or an emailed sign-in link (ADR-118). People sign in on `account.getplenipo.com`, in a
  web browser. Plenipo on the PC never signs in to it today.
- A Pro license covers one person on any of their own PCs (ADR-110). So one person may run
  Community from more than one PC.
- A Free copy never contacts 8 West (ADR-115). Community cannot work without contacting 8 West.
- Plenipo must never ask for a password in its own window when a safer way exists, and never keeps
  one. Keys and secrets live in the Vault.
- A paid account is the strongest brake on spam and fake accounts that a small company can have.

## Decision

1. **Community is off until you turn it on:** Settings → Switches → **Community**, main window only.
   Turning it on asks you to sign in. Turning it off is **Leave Community** (ADR-167).
2. **Signing in uses a code, never a password in Plenipo** (the way a TV signs in):
   - Plenipo asks the account service for a short code and shows it: "**Enter this code: 4KQ-7TD**",
     with **Open the sign-in page**, which opens `account.getplenipo.com` in your own web browser.
   - You sign in there as usual (password or emailed link), see "**Plenipo on FRANKIE-DESKTOP wants
     to use Community as you**", and press **Allow**.
   - Plenipo then receives a **Community pass** for this PC, kept in the Vault. It is never shown,
     logged, or put in a diagnostics file. The code works once, for 10 minutes.
   - **Settings → Community** shows "**Signed in as Frank Gonzalez**", and **Sign out of your
     account**. The account site lists each PC that holds a pass, with **Remove**, which ends that
     pass at once.
3. **Each PC also makes its own Community key** (Ed25519 for signing, X25519 for sealing), kept in
   the Vault. The account service learns only the public halves, tied to your account. Messages and
   objectives are sealed for these keys (ADR-164). A second PC of yours gets its own key and pass.
4. **Who may join:** a person **13 or older** (question 4, the owner: "13 and up").
   1. **Asked once:** turning Community on asks for **your birth month and year**, never the day,
      and never an ID. Under 13, Plenipo says "**Community is for people 13 and older**", sends
      nothing, and keeps nothing of the answer. Otherwise the account service keeps the month and
      year (ADR-168), so a member who turns 18 becomes an adult member by themselves. Changing it
      later goes through 8 West (by email), so nobody can type their way past the protections. The
      terms of service say the same (draft in `docs/legal/phase-24/`).
   2. **Buying Pro needs 18**, as the terms of sale already say ("for adults"). So a member 13 to 17
      is always on a free account: they can answer, and be a collaborator, but not start a
      conversation, invite, or link (§5).
   3. **Protections for members 13 to 17** (question 14, as recommended), enforced by the account
      service, not only by Plenipo:
      - never listed in the Community directory (ADR-163), and their profile card is shown only to
        people they already talk with, help as a collaborator, or are helped by;
      - a message from an adult always lands in **Requests** first, with "**You don't know this
        person yet. Never share passwords, keys, or where you live.**", and **Block** and **Report**
        right there;
      - adults never see a teen's status or mood;
      - every report about a member under 18, or made by one, is urgent (ADR-167 §8);
      - no money rewards, and never on the leaderboard (ADR-169);
      - GIFs rated for everyone only (ADR-164 §4).
   4. **The attorney checks** whether any state also needs a parent's consent for members under 18,
      and what else the law asks (`docs/legal/phase-24/age-requirement.md`).
5. **Who needs Pro** (question 7):
   - **Starting** needs Pro on the PC that starts it: writing to someone who has never written to
     you, and inviting a collaborator (ADR-166).
   - **Linking organizations** needs Pro on **both** sides (ADR-165): each side runs the other's
     objectives with its own AI tools, and a link joins two businesses.
   - **Answering** needs only a **free 8 West account**: replying to someone who wrote first, and
     accepting an invitation to be a collaborator.
   - A **Free copy** may sign in to Community for this. It then contacts 8 West **only for
     Community, only while you are signed in**, and never for anything else. This is the one change
     to ADR-115. A Free copy that never signs in still never contacts 8 West.
   - One place decides it, as for every limit: a new `Limit::CommunityStart` in
     `Entitlements::check`. When Pro ends, starting pauses; conversations and links already made
     keep working for answering, and nothing is deleted (like Connections, ADR-068).
6. **What a PC sends the account service for Community** (written in `docs/editions.md` and checked
   in a test, as ADR-143 §10 did for the relay): its Community pass, its public Community keys,
   sealed messages and their envelopes (ADR-164), your public profile if you turn it on (ADR-163),
   blocks, reports (ADR-167), and links and invitations (ADR-165, ADR-166). Never a project, a file,
   a folder name, a task, a worker's answer, a key, or anything from the Ledger, unless you send it
   on purpose.
7. **Guard decides Plenipo's own requests to the account service**, as for the weekly check: a new
   purpose, **Community**, allows only `account.getplenipo.com`, only while Community is on and you
   are signed in. Any other address is refused and recorded as `guard.request_refused`.
8. **New desktop commands are the main window's alone**, with IPC tests that refuse a second window,
   the sign, and a web page: the switch, sign in, sign out, and the age box.
9. **The Ledger records** `community.signed_in` and `community.signed_out` (which account, never the
   pass).

## Consequences

- A person needs an 8 West account to use Community, even on Free. That is a new step for Free
  owners, and a new kind of account (one that never bought anything) for the account service.
- The account service now holds more than billing: profiles, envelopes, blocks, reports. ADR-168
  says how long each is kept, and the security review checks the service again.
- Spam has to come from paid accounts, which 8 West can end.
- Members as young as 13 can join. Because messages are sealed, 8 West cannot watch for adults who
  try to harm them; the protections in §4.3, fast reports, and the attorney's review carry that load.
- The account service keeps a birth month and year for every member (ADR-168).

## Alternatives considered

- **Type the password into Plenipo.** Simpler, but Plenipo would handle the password, and a fake
  window could ask for it. The code way never shows Plenipo a password.
- **Community for Pro only, both sides.** Simplest, but a business owner could not invite a helper
  who runs Free, and the acceptance test (a collaborator approves from their own Plenipo) would need
  two paid copies.
- **Community for everyone, Free included, with no Pro step.** Free accounts cost nothing to make,
  so spam and fake accounts would be cheap.
- **18 and older** (the builder's first recommendation). Fewer laws and duties, but the owner chose
  13 and up, so younger people can take part.
- **No age requirement.** Under 13 brings COPPA, the children's privacy law, and its parental
  consent.
- **A box "I am 13 or older" only.** Plenipo could not tell who is under 18, so it could not protect
  them.
- **Check age with an ID, or the full birth date.** More data to keep and protect than a month and
  year.
