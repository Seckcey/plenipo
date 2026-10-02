# Age requirement for Community — draft

> **Draft for attorney review. Not in force.** From
> [ADR-162 (your 8 West account in Plenipo)](../../adr/ADR-162-your-account-in-plenipo.md) §4.
> **[Owner's answer: question 4]**

## The rule

**Community is only for people 18 and older.**

## Why 18

- Plenipo is a business tool: it runs AI workers that can spend money, change files, and act on
  servers and online accounts. It is sold to adults buying for themselves or their business (the
  account privacy notice already says the account service "is for adults").
- Community lets people talk privately and work in each other's organizations. Keeping it to adults
  avoids the extra duties that apply to children and teenagers online: COPPA for children under 13,
  and a growing number of state laws for minors on online services.
- Private messages are sealed, so 8 West cannot watch conversations for grooming or other harm to
  minors. Keeping minors out is the safer design.

## How it is asked

1. When someone turns **Community** on in Plenipo, Plenipo shows the Community terms and a box:
   "**I am 18 or older**". Community cannot be turned on without ticking it.
2. The account service records that the person ticked it, and when, for as long as the account
   lasts.
3. Plenipo does not ask for a birth date or an identity document.
4. The Community terms and the privacy notice both state the rule.

## If someone under 18 is found

- Anyone can report a person with the reason **Someone under 18**. That report emails 8 West's
  owner at once, and is looked at the same day where possible
  ([ADR-167](../../adr/ADR-167-block-report-leave.md) §8).
- If 8 West reasonably believes a person is under 18, it ends their use of Community and deletes
  their Community data, as for any ended account (ADR-168), except what the law requires it to keep.
- The person can ask once for another look, for example if they are in fact 18 or older.

## Questions for the attorney

1. Is a tick box enough, for a business tool with no feed and no public posts?
2. Do any state laws require more (for example age checks, or parental consent) for a service like
   this, even when it is limited to adults?
3. If Community is open outside the United States, does any country require a different age or
   wording?
4. Should the Pro terms of sale also state an age? (Today the account privacy notice says the
   service is for adults, with no age.)
