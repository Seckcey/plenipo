# ADR-162: Your 8 West account in Plenipo — signing in, who may join, and who needs Pro

- **Status:** Proposed (2026-10-02), waiting for the owner's answers to questions 4 and 7 in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Would amend:** [ADR-115 (a Free copy never contacts 8 West)](ADR-115-free-never-contacts-8-west.md),
  only for a Free copy whose owner signs in to Community on purpose (Decision 5)
- **Builds on:** [ADR-118 (customer accounts)](ADR-118-customer-accounts.md),
  [ADR-110 (one person, any of their PCs)](ADR-110-one-person-any-of-their-pcs.md),
  [ADR-101 (the account service's own repository)](ADR-101-account-service-repository-name.md)

> **On screen** (ADR-010, plain words and rank names): **Community** (the switch in Settings →
> Switches), **Sign in to your 8 West account**, **Open the sign-in page**, **Enter this code:
> 4KQ-7TD**, **Signed in as Frank Gonzalez**, **Sign out of your account**, **I am 18 or older**,
> **Part of Pro**.

## In short

Community is about **people**, so Plenipo needs to know who you are. Today it does not: a Pro copy
holds a license key, and the weekly check sends only the key's ID (ADR-115, ADR-116). This record
says how you sign in to your **8 West account** inside Plenipo, that **everyone in Community must
be 18 or older**, and that **starting** things (a conversation, a link, an invitation) is part of
**Pro**, while **answering** needs only a free account.

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
4. **Who may join:** a person **18 or older** (question 4). Turning Community on asks once, with a
   box "**I am 18 or older**" that must be ticked, and the account service records that you said so,
   and when. The terms of service say the same (draft in `docs/legal/phase-24/`). Plenipo does not
   ask for a birth date or an ID.
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

## Alternatives considered

- **Type the password into Plenipo.** Simpler, but Plenipo would handle the password, and a fake
  window could ask for it. The code way never shows Plenipo a password.
- **Community for Pro only, both sides.** Simplest, but a business owner could not invite a helper
  who runs Free, and the acceptance test (a collaborator approves from their own Plenipo) would need
  two paid copies.
- **Community for everyone, Free included, with no Pro step.** Free accounts cost nothing to make,
  so spam and fake accounts would be cheap.
- **No age requirement, or 13 and older.** Younger members bring children's privacy laws (COPPA
  under 13, and several state laws for teenagers) and a much heavier moderation duty. Plenipo is a
  business tool; 18 fits it.
- **Check age with an ID or a birth date.** More data to keep and protect, for a business tool that
  already takes a payment card on Pro.
