# Phase 24 (Community) — drafts for the attorney

> **Draft for attorney review. Not in force.** Nothing in this folder is published. The website's
> legal pages (`apps/website/legal/`) and the account site's legal pages (in the private repository
> `plenipo-account`) change only after the attorney approves these texts
> ([ADR-160 (building Phase 24 alongside Phase 23)](../../adr/ADR-160-phase-24-alongside-phase-23.md) §2).

Plenipo is made by 8 West Ventures, LLC. Community lets Plenipo owners show a profile, send each
other private messages, link their organizations, and invite collaborators. The product decisions
behind these drafts are in
[ADR-161 (Phase 24 starts)](../../adr/ADR-161-phase-24-starts.md) and ADR-162 to ADR-168. They are
**proposed**: the owner has not answered every question yet, so each draft marks the places that
follow a recommendation with **[Owner's answer: question N]**.

**The owner's answers (2026-10-02):** questions 1 to 13 are answered. Three changed the drafts:
members may be **13 and up**, with protections for members under 18; a message may hold
**everything a phone's keyboard types** (every letter, number, symbol, and emoji); and Community
gets a **directory** and **rewards** for taking part (badges, thanks, and a free month of Pro for
invitations). Follow-up questions 14 to 18 are still open, and the drafts mark them.

## What is here

| Draft                                               | What it is                                                                                   |
| --------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| [Community terms](community-terms.md)               | Terms of service for Community, to add to the website's terms of service                     |
| [Privacy changes](privacy-changes.md)               | Changes to the website's privacy statement and to the account privacy notice                 |
| [Age requirement](age-requirement.md)               | Who may join Community, and how it is asked                                                  |
| [Moderation and reports](moderation-and-reports.md) | How reports are handled, by whom, what 8 West may do, appeals, and requests from authorities |

## Questions for the attorney

1. **Where the Community terms live.** Recommended: a new section of the website's terms of service
   (`apps/website/legal/terms.md`), accepted when someone turns Community on. Or: a separate page.
2. **Acceptance.** Turning Community on shows the terms and asks for a birth month and year. Is a
   link to the terms with an "I agree" button enough? For members 13 to 17, does a parent need to
   agree?
3. **Age.** The owner chose 13 and up. Is a typed birth month and year enough, which state laws
   for minors could apply, and are the protections for members 13 to 17 right?
   (age-requirement.md)
4. **Sealed messages and the law.** 8 West cannot read private messages. Which duties apply to it
   anyway (for example 18 U.S.C. § 2258A reporting to NCMEC when it learns of apparent child sexual
   abuse material through a report), and how long must it keep what it reports?
5. **Requests from authorities.** What process should 8 West follow, and what notice may it give the
   person concerned? (moderation-and-reports.md)
6. **Retention.** Are the times in ADR-168 acceptable (reports kept 1 year after they are closed; a
   ban kept as a scrambled code of the email while it lasts)?
7. **Ending someone's Community and their Pro subscription.** Ending Community does not end Pro.
   Is that right, and should any refund apply?
8. **Linked organizations.** When one business sends another an objective, and the other's AI
   workers do the work, who is responsible for the result? The draft says each owner is responsible
   for what they ask and what they approve; the attorney decides the wording.
9. **Collaborators.** An owner invites a person to view, approve, or manage work in their
   organization. Does the owner need any statement from the collaborator (for example about
   confidentiality), or is that between them?
10. **Rewards.** A free month of Pro for the person who invites and the person invited, when the
    invited person buys Pro and keeps it past 14 days (ADR-169). Does this program need written
    rules, a tax note, or anything for states with promotion laws?
11. **The directory.** People choose to be listed, adults only. Anything the terms or the privacy
    notice must say about it?
12. **The laws of other countries** (for example the EU's Digital Services Act for messaging and
    reports), if Community is open outside the United States.

These drafts follow the website's rule: they do not invent a governing law, an arbitration clause,
a fixed time that the service does not enforce, or a certification
([website legal statements](../../adr/website-legal-statements.md)). Where one is needed, the draft
says **[Attorney: ...]**.
