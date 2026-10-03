# ADR-197: Templates for organizations, departments, and projects

- **Status:** Accepted (the owner's first list, kept in Phase 25: "Add templates for
  organizations, projects, businesses and enterprises"; item 2.8 of
  [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), accepted 2026-10-03).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 2 (item 2.8)
- **Carries out part of:** parked Phase 15 (department templates). Phase 15 itself stays parked:
  only the owner can schedule it.
- **Builds on:** [ADR-091 (the owner's answers for Phase 21)](ADR-091-phase-21-owners-answers.md) §5 ("Use a template" in the
  new-organization dialog), [ADR-094 (more than one organization)](ADR-094-more-than-one-organization.md)
  §15 (copying an organization's setup), and
  [ADR-196 (use the team you hired first)](ADR-196-use-the-team-you-hired-first.md).

> **On screen** (ADR-010, plain words and rank names): "Use a template" in New organization;
> "Start from a template" in New department; "Use the Software project template" in New project;
> Settings → Organization → Templates: "Add a template's departments" and "Save this organization
> as a template".

## In short

"Use a template" was greyed out ("Templates are coming later"). The only template was the
Development team behind "Set up a Development project".

**Accepting this record means:**

1. **Five organization templates**, each a set of departments, every department with its manager
   and an on-call team of built-in roles:
   - **Software project:** Development (Senior Developer, Code Reviewer, QA Engineer,
     Documentation Writer).
   - **Small business:** Operations (Operations Engineer, Security Auditor) and Marketing
     (Designer, Writer, Researcher).
   - **Agency:** Development, Design (Designer, Researcher), and Marketing.
   - **IT services:** Operations and Documentation (Documentation Writer, Researcher).
   - **Enterprise:** Development, Operations, Marketing, Design, and Documentation.
2. **Where you pick one:** a new organization ("Use a template"); this organization (Settings →
   Organization → Templates: only the departments it doesn't have are added); a new department
   (one department's template); a new project ("Use the Software project template", which reuses
   the department's workers first, ADR-196).
3. **Free keeps one department:** a template that would add more than one is part of Pro, and is
   refused before anything is made. One department works on Free like any other.
4. **Save this organization as a template:** its departments and positions, roles, permissions,
   switches, AI model choices, and titles, the same copy as "Copy from one of your organizations",
   kept in a small file of its own in Plenipo's data folder (`templates/`). Never its work, keys,
   connections, or files. A new organization can start from it.

## Decision

- Each role's starting model choices and permissions come with the built-in role, so a template
  needs no settings of its own; rank names stay the organization's.
- A department's team reports to its manager. A project and its supervisor are added later (New
  project, or the Software project template).

## Consequences

- A new owner can have a whole organization in one click.
- Saved templates are kept per PC, beside the list of organizations.

## Alternatives considered

- **Templates with their own models and permissions.** Rejected for now: the built-in roles
  already carry sensible starting choices, and fewer settings is the point of Wave 2.
- **A project and supervisor in every department.** Rejected: a project needs its own folder and
  name, which a template can't know.
