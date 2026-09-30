# ADR-114: What counts as a business department

- **Status:** Accepted (by the owner, 2026-09-30, as recommended)
- **Date:** 2026-09-30
- **Phase:** 11A
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)
- **Carries out:** [ADR-021 (Free and Pro editions)](ADR-021-editions-and-license.md) §3: the Sales
  department on HubSpot and the business departments after it are Pro

> **On screen** (ADR-010, plain words and rank names): setting up a business department on a Free
> copy says "**Part of Pro**", with what Pro adds.

## In short

Free's one department can be the Development department or a plain department the owner makes. A
department set up from a business template (Sales on HubSpot, and the ones after it) needs Pro. No
business template exists yet, so the lock is built and tested now, and Phase 9 uses it.

## Context

- Free has one department. The Development department is Free.
- Sales on HubSpot is postponed ([ADR-018](ADR-018-sales-on-hubspot-no-paperclip.md), Phase 9). The
  only department setup that exists is the Development department's.
- A plain department is just a name and a head. A business template brings the roles, instructions,
  and connections that make it worth paying for.

## Decision

1. **Free:** one department, either the Development department or a plain one the owner makes.
2. **Pro:** any number of departments, including those set up from a business template.
3. **The lock sits in the template setup,** through `Entitlements::check`. Each template is marked as
   business or not. Development is not.
4. **Tested now with a test-only business template**, since no real one exists. Phase 9's Sales
   template is marked as business and is locked from its first day.
5. **When Pro ends:** business departments stay, open, and keep working. Only setting up a new one is
   blocked, as for every department (`docs/editions.md`). Connections they use pause
   ([ADR-068](ADR-068-connections-are-pro.md)).

## Consequences

- A Free owner can make a plain department named "Sales". What they cannot get on Free is the
  business template's roles and setup.

## Alternatives considered

- **Only Development on Free.** Not chosen: a Free owner whose one real project is not software
  could not use Plenipo at all.
