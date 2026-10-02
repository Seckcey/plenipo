# ADR-167: Block, report, and leave — and who handles reports, and how

- **Status:** Proposed (2026-10-02), waiting for the owner's answer to question 3 in
  [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md).
- **Date:** 2026-10-02
- **Phase:** 24 (Community)
- **Part of:** [ADR-161 (Phase 24 starts)](ADR-161-phase-24-starts.md)
- **Builds on:** [ADR-164 (private messages, sealed)](ADR-164-private-messages-sealed.md) §6, the
  proof that makes a report trustworthy; [ADR-107 (admin sign-in)](ADR-107-admin-sign-in.md), the
  page where 8 West works

> **On screen** (ADR-010, plain words and rank names): **Block**, **Unblock**, **Report**, **What is
> wrong?**, **Spam**, **Harassment or threats**, **A scam**, **Hate**, **Sexual content**, **Someone
> under 18**, **Pretending to be someone else**, **Something else**, **Thanks. 8 West will look at
> this.**, **Leave this conversation**, **Leave**, **Leave Community**, **Not delivered**.

## In short

**Block** and **Report** are on every person, message, profile, link, and invitation. **Leave** is
on every conversation, link, and shared organization, and **Leave Community** turns it all off.
Reports go to **8 West**, and at first that means **the owner of 8 West, Frank**, on 8 West's own
admin page. Because messages are sealed, 8 West sees only what the person reporting chooses to
send, with a proof that it is real. 8 West looks within **2 business days**, sooner for threats or
anything involving a child, and can warn, hide a picture or message, pause someone's Community, or
end it.

## Context

- Phase 24's plan: "block, report, and leave, everywhere", and "this phase's ADR decides ... how
  reports are handled and by whom". Test: "a blocked person cannot message or send objectives".
- 8 West is a small company. Whoever handles reports must be someone who exists today.
- Messages are sealed end to end (ADR-164), so 8 West cannot look at a conversation by itself.
- The account service already has an admin page behind Cloudflare Access, a password, and a
  passkey, and records every admin action (`admin_audit`) (ADR-107).
- In the United States, a service that learns of apparent child sexual abuse material must report
  it to the National Center for Missing & Exploited Children (NCMEC). The attorney confirms the
  exact duties.

## Decision

### Block

1. **Block** works on a person, everywhere at once: they cannot message you, send a link request or
   an invitation, send objectives, or find your profile. Any link with them ends, and they stop being
   your collaborator, or you theirs. What is already on your PC stays until you delete it.
2. **They are not told.** Their messages show **Not delivered**; their requests show a plain "Can't
   send this". This keeps blocking safe for the person who blocks.
3. **Unblock** at any time, in Settings → Community → **Blocked**. Links and collaborations that the
   block ended do not come back by themselves.
4. The account service enforces blocks, so a blocked person's PC cannot get around them, and your
   PC refuses them too (two locks).

### Report

5. **Report** is on a person, a message, a profile, a link request, an invitation, and an objective
   from a linked organization. It asks **What is wrong?** (the list above) and lets you add up to
   1,000 characters. It offers to **Block** at the same time.
6. **What a report carries:** what you report, and nothing else. For messages and objectives, you
   tick which ones (up to 20); your PC sends their words with the proof from ADR-164 §6, so 8 West
   can tell they are real. For a profile, the account service already has it. Never your files, your
   Ledger, or other conversations.
7. **Who handles it** (question 3): **8 West Ventures, LLC**, and to begin with **the owner of
   8 West**, on a new **Reports** page in the account service's admin page. Each report shows what
   was reported and its proof check, the reporter's and the reported person's Community names, and
   earlier reports about them. 8 West can add trained helpers later; each has their own admin
   sign-in, and every action is in `admin_audit`.
8. **How fast:** 8 West looks within **2 business days**. A report of **Harassment or threats** or
   **Someone under 18** emails the owner at once, and is looked at the same day where possible.
9. **What 8 West can do**, with a plain reason each time:
   - nothing, if nothing broke the rules;
   - a **warning** by email;
   - **hide** a profile's picture, message, or name;
   - **pause** the person's Community for 7 or 30 days (they can still use Plenipo and Pro);
   - **end** the person's Community for good. Their profile and waiting messages are deleted, their
     links and collaborations end, and they cannot make a new Community account with the same email.
     Pro itself is not ended by this; the terms say what happens to it (draft for the attorney).
10. **The person reporting** is told when it is done ("**We looked at your report and took
    action.**" or "**...and found nothing against our rules.**"), never what action.
11. **The person reported** is told what rule and what action, by email, and can ask once for
    another look, by replying within 14 days. If 8 West has more than one person by then, someone
    else looks.
12. **The law:** apparent child sexual abuse material is reported to NCMEC as the law requires, and
    kept only as the law says (the attorney confirms). A threat to someone's life is passed to the
    police where 8 West believes it is real. Requests from the police follow the attorney's process
    (draft in `docs/legal/phase-24/moderation-and-reports.md`), and 8 West can only give what it
    has: never a sealed message's words, which it cannot read.
13. **Reports are not shown to workers**, and never go through Plenipo's AI workers or any AI
    company. People read them.

### Leave

14. **Leave this conversation** deletes it from your PCs and refuses new messages in it (the other
    person sees **Not delivered**). **Unlink** leaves a link (ADR-165 §6). **Leave** leaves an
    organization you help with (ADR-166 §9).
15. **Leave Community** (turning the switch off, ADR-162 §1) hides your profile, ends every link
    and collaboration, deletes your messages still waiting on 8 West's server, and signs this PC
    out. What is on your PC stays until you delete it, with **Delete my Community data from this
    PC**. Your 8 West account stays; **Delete my account** on the account site removes Community with
    it (ADR-168).

### Recorded

16. Your Ledger records `community.blocked`, `community.unblocked`, `community.reported` (what kind,
    and the reason chosen; never the words), and each leave. The account service records reports and
    every admin action.
17. **New desktop commands are the main window's alone**, with IPC tests that refuse a second
    window, the sign, and a web page.

## Consequences

- At first, the owner of 8 West reads reports. That is real work, and it grows with Community.
  The checklist asks the owner to plan for it before launch.
- 8 West cannot find abuse nobody reports. The terms and the privacy notice say so.
- A banned person could make a new account with a new email. Paid accounts (ADR-162 §5) make that
  cost money each time.

## Alternatives considered

- **Tell a blocked person they are blocked.** Clearer for them, less safe for the person blocking.
- **An outside moderation company.** Costs money every month, and sees reports that 8 West could
  read itself while Community is small. Possible later.
- **AI to sort reports.** Sending reports to an AI company shares other people's words with one
  more party; people read them at first.
- **Reports without the reported words.** 8 West could not tell what happened.
